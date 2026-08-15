//! 现货钱包与杠杆钱包之间的资金划转适配器。
//!
//! 承载双向搬运资金、解析划转资产、写入划转幂等记录，以及从历史流水重建重放响应。
//! 核心不变量是锁序：无论资金流向哪边，都固定先锁现货钱包再锁杠杆钱包，
//! 靠这个稳定顺序而不是加锁时机来防止两个方向的并发划转交叉等待形成死锁。
//! 资金只在 available 桶之间搬运，frozen 与 locked 原样带进流水快照，划转不涉及冻结。
//! 每次划转写两条配对流水，共用同一个 `transfer_id` 作为引用，幂等重放正是靠回读这两条流水的
//! after 快照来还原首次响应，因此返回的是当时余额而非当前余额。

use super::ledger::{
    insert_margin_wallet_ledger, insert_spot_wallet_ledger, lock_margin_wallet_row,
    lock_spot_wallet_row,
};
use crate::{
    error::{AppError, AppResult},
    modules::margin::presentation::MarginWalletAccountSnapshot,
};
use bigdecimal::BigDecimal;
use sqlx::{MySql, Pool, Transaction};

#[derive(Debug, sqlx::FromRow)]
/// 保证金划转使用的启用资产标识、符号与金额精度规则。
pub(crate) struct MarginTransferAssetRule {
    /// 解析后的资产主键，无论调用方按 id 还是 symbol 指定都归一到这里。
    pub(crate) id: u64,
    /// 该资产允许的最大小数位，划转金额超出这个精度会在动账前被拒绝。
    pub(crate) precision_scale: i32,
    /// 是否允许从现货账户发起新的转入；关闭后既有杠杆余额仍可转回现货。
    pub(crate) margin_transfer_enabled: bool,
}

#[derive(Debug, sqlx::FromRow)]
/// 已持久化划转请求快照，用于同键同参重放及异参冲突判断。
pub(crate) struct MarginTransferRecord {
    /// 首次划转生成的 UUIDv7 业务编号，同时是两条配对流水的引用值。
    pub(crate) transfer_id: String,
    /// 首次划转实际使用的资产主键，重放时与本次请求解析结果比对。
    pub(crate) asset_id: u64,
    /// 归一化后的来源账户，只会是 spot 或 margin。
    pub(crate) from_account: String,
    /// 归一化后的目标账户，与来源账户不同。
    pub(crate) to_account: String,
    /// 首次划转的金额，重放时必须完全相等才认定为同一笔请求。
    pub(crate) amount: BigDecimal,
}
/// 按现货后保证金的稳定顺序锁定两侧钱包，将同额资金从现货转入保证金并各写流水。
/// 两侧余额、两笔流水与划转记录同事务提交；余额不足或任一步失败整体回滚。
///
/// 余额检查紧跟在锁现货之后、锁杠杆之前，因此资金不足时可以少持有一把锁就提前退出。
/// 杠杆侧用 `lock_margin_wallet_row`，账户不存在会先补一行零余额再加锁，首次转入无需预建账户。
/// 现货流水记为 `margin_transfer_out` 且金额取负，杠杆流水记为 `margin_transfer_in` 金额取正，
/// 两条共用同一个 `transfer_id`，构成可对账的配对记录。
/// 返回值固定是「现货快照在前、杠杆快照在后」，与资金流向无关，调用方按位置解构即可。
pub(crate) async fn transfer_spot_to_margin_wallets(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    transfer_id: &str,
) -> AppResult<(MarginWalletAccountSnapshot, MarginWalletAccountSnapshot)> {
    // 双向划转统一先锁现货、再锁杠杆钱包，避免反向请求形成交叉等待。
    let spot_wallet = lock_spot_wallet_row(tx, user_id, asset_id).await?;
    if spot_wallet.available < *amount {
        return Err(AppError::Validation(format!(
            "insufficient available balance for margin transfer: requested {}, available {}, locked {}",
            amount, spot_wallet.available, spot_wallet.locked
        )));
    }
    let margin_wallet = lock_margin_wallet_row(tx, user_id, asset_id).await?;
    let spot_available_after = spot_wallet.available.clone() - amount.clone();
    let margin_available_after = margin_wallet.available.clone() + amount.clone();
    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&spot_available_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query(
        "UPDATE margin_wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?",
    )
    .bind(&margin_available_after)
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    insert_spot_wallet_ledger(
        tx,
        user_id,
        asset_id,
        "margin_transfer_out",
        &(-amount.clone()),
        &spot_available_after,
        &spot_available_after,
        &spot_wallet.frozen,
        &spot_wallet.locked,
        "margin_transfer",
        transfer_id,
    )
    .await?;
    insert_margin_wallet_ledger(
        tx,
        user_id,
        asset_id,
        "margin_transfer_in",
        amount,
        &margin_available_after,
        &margin_available_after,
        &margin_wallet.frozen,
        &margin_wallet.locked,
        "margin_transfer",
        transfer_id,
    )
    .await?;
    Ok((
        MarginWalletAccountSnapshot {
            asset_id,
            available: spot_available_after,
            frozen: spot_wallet.frozen,
            locked: spot_wallet.locked,
        },
        MarginWalletAccountSnapshot {
            asset_id,
            available: margin_available_after,
            frozen: margin_wallet.frozen,
            locked: margin_wallet.locked,
        },
    ))
}

/// 在调用方事务内完成杠杆到现货划转；金额精度和资产有效性应已由应用层校验。
/// 即使资金方向相反也固定先锁现货钱包、再锁杠杆钱包，随后校验杠杆侧可用余额。
/// 杠杆可用余额扣减、现货可用余额增加及两条配对流水必须同事务提交，余额快照与流水一致。
/// 本函数不提交事务也不独立处理重放；调用方以划转幂等记录阻止重复动账。
pub(crate) async fn transfer_margin_to_spot_wallets(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    transfer_id: &str,
) -> AppResult<(MarginWalletAccountSnapshot, MarginWalletAccountSnapshot)> {
    // 与 spot -> margin 保持相同锁序。
    let spot_wallet = lock_spot_wallet_row(tx, user_id, asset_id).await?;
    let margin_wallet = lock_margin_wallet_row(tx, user_id, asset_id).await?;
    if margin_wallet.available < *amount {
        return Err(AppError::Validation(format!(
            "insufficient margin available balance for transfer: requested {}, available {}, locked {}",
            amount, margin_wallet.available, margin_wallet.locked
        )));
    }
    let margin_available_after = margin_wallet.available.clone() - amount.clone();
    let spot_available_after = spot_wallet.available.clone() + amount.clone();
    sqlx::query(
        "UPDATE margin_wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?",
    )
    .bind(&margin_available_after)
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&spot_available_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
    insert_margin_wallet_ledger(
        tx,
        user_id,
        asset_id,
        "margin_transfer_out",
        &(-amount.clone()),
        &margin_available_after,
        &margin_available_after,
        &margin_wallet.frozen,
        &margin_wallet.locked,
        "margin_transfer",
        transfer_id,
    )
    .await?;
    insert_spot_wallet_ledger(
        tx,
        user_id,
        asset_id,
        "margin_transfer_in",
        amount,
        &spot_available_after,
        &spot_available_after,
        &spot_wallet.frozen,
        &spot_wallet.locked,
        "margin_transfer",
        transfer_id,
    )
    .await?;
    Ok((
        MarginWalletAccountSnapshot {
            asset_id,
            available: spot_available_after,
            frozen: spot_wallet.frozen,
            locked: spot_wallet.locked,
        },
        MarginWalletAccountSnapshot {
            asset_id,
            available: margin_available_after,
            frozen: margin_wallet.frozen,
            locked: margin_wallet.locked,
        },
    ))
}

/// 在划转事务内解析出要搬运的资产，并取回它的精度规则供后续金额校验。
/// `asset_id` 优先：给了主键就只按主键查，此时完全不看 `asset_symbol`，两者不一致不会被察觉。
/// 只有主键缺省时才回退到符号查询，符号比较忽略大小写，空白符号视同未提供并报必填。
/// 两条分支都要求资产处于 active，停用资产查不到即返回 NotFound，禁止对已下架币种发起新划转。
/// 虽然在事务内执行，但查询不带 FOR UPDATE，只是读取配置，不锁定资产行。
pub(crate) async fn resolve_active_transfer_asset(
    tx: &mut Transaction<'_, MySql>,
    asset_id: Option<u64>,
    asset_symbol: Option<&str>,
) -> AppResult<MarginTransferAssetRule> {
    if let Some(asset_id) = asset_id {
        return sqlx::query_as::<_, MarginTransferAssetRule>(
            "SELECT id, precision_scale, margin_transfer_enabled FROM assets WHERE id = ? AND status = 'active' LIMIT 1",
        )
        .bind(asset_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound);
    }
    let Some(symbol) = asset_symbol
        .map(str::trim)
        .filter(|symbol| !symbol.is_empty())
    else {
        return Err(AppError::Validation(
            "margin transfer asset_id or asset_symbol is required".to_owned(),
        ));
    };
    sqlx::query_as::<_, MarginTransferAssetRule>(
        r#"SELECT id, precision_scale, margin_transfer_enabled
           FROM assets
           WHERE UPPER(symbol) = UPPER(?) AND status = 'active'
           LIMIT 1"#,
    )
    .bind(symbol)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)
}

/// 为幂等重放解析资产主键，与新划转路径的关键差别是这里不要求资产仍处于 active。
/// 因为首次划转成功后资产可能被下架，此时用户重试同一个幂等键仍应拿回原结果而不是报 NotFound。
/// 优先级规则与新划转一致：给了主键只查主键，缺省才按符号忽略大小写查找，空白符号报必填。
/// 只返回主键供逐字段比对，不取精度规则，因为重放不会重新校验金额也不会真正动账。
/// 走连接池而非事务，重放核对是纯只读操作，无需与后续写入共享事务视图。
pub(crate) async fn resolve_transfer_asset_id_for_replay(
    pool: &Pool<MySql>,
    asset_id: Option<u64>,
    asset_symbol: Option<&str>,
) -> AppResult<u64> {
    if let Some(asset_id) = asset_id {
        return sqlx::query_scalar::<_, u64>("SELECT id FROM assets WHERE id = ? LIMIT 1")
            .bind(asset_id)
            .fetch_optional(pool)
            .await?
            .ok_or(AppError::NotFound);
    }
    let Some(symbol) = asset_symbol
        .map(str::trim)
        .filter(|symbol| !symbol.is_empty())
    else {
        return Err(AppError::Validation(
            "margin transfer asset_id or asset_symbol is required".to_owned(),
        ));
    };
    sqlx::query_scalar::<_, u64>("SELECT id FROM assets WHERE UPPER(symbol) = UPPER(?) LIMIT 1")
        .bind(symbol)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

#[allow(clippy::too_many_arguments)]
/// 在资金变更前写入划转请求快照并占用用户幂等键，唯一键阻止并发重复动账。
/// 返回类型刻意保留原始 `sqlx::Error` 而不包装成 `AppError`，因为调用方必须据错误码区分
/// 「唯一键冲突要转入重放」和「真实数据库故障要上抛」，包装后会丢掉这个判定依据。
/// 落库内容就是这笔请求的完整语义：资产、双向账户和金额，重放核对全部依赖这几列。
/// 必须先于任何余额更新执行，先占键后动钱是整条划转链路防重复扣款的前提。
pub(crate) async fn insert_margin_transfer(
    tx: &mut Transaction<'_, MySql>,
    transfer_id: &str,
    user_id: u64,
    asset_id: u64,
    from_account: &str,
    to_account: &str,
    amount: &BigDecimal,
    idempotency_key: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO margin_transfers
           (transfer_id, user_id, asset_id, from_account, to_account, amount, idempotency_key)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(transfer_id)
    .bind(user_id)
    .bind(asset_id)
    .bind(from_account)
    .bind(to_account)
    .bind(amount)
    .bind(idempotency_key)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 按用户和幂等键读取既有划转请求快照，返回资产、双向账户和金额供逐字段比对。
/// 幂等键的作用域是单个用户，不同用户可以使用相同的键而互不干扰。
/// 未命中返回 None 表示这是一次全新请求，调用方继续走正常划转流程。
/// 走连接池只读且不加锁，因此并发的首次划转若尚未提交，这里会读不到从而放行到插入分支，
/// 真正的并发保护由插入时的唯一键冲突兜底。
pub(crate) async fn load_margin_transfer_by_idempotency_key(
    pool: &Pool<MySql>,
    user_id: u64,
    idempotency_key: &str,
) -> AppResult<Option<MarginTransferRecord>> {
    sqlx::query_as::<_, MarginTransferRecord>(
        r#"SELECT transfer_id, asset_id, from_account, to_account, amount
           FROM margin_transfers
           WHERE user_id = ? AND idempotency_key = ?
           LIMIT 1"#,
    )
    .bind(user_id)
    .bind(idempotency_key)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

/// 从原划转写下的两条配对流水中回读余额 after 快照，据此重建首次响应而不再次移动资金。
/// 分别查现货流水表和杠杆流水表，都以用户、资产、引用类型 `margin_transfer` 和 `transfer_id` 定位，
/// 按流水主键升序取第一条，确保同一笔划转即使被误写多条也稳定取到最早那条。
/// 两条流水缺任意一条都返回内部错误，因为划转成功时必然成对写入，缺失说明数据已不一致。
/// 之所以不直接读钱包当前余额，是为了不把后续交易造成的变化泄漏进这次重放响应，
/// 保证同一个幂等键无论重试多少次，返回的余额都与首次完全一致。
pub(crate) async fn load_margin_transfer_wallet_snapshots(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    transfer_id: &str,
) -> AppResult<(MarginWalletAccountSnapshot, MarginWalletAccountSnapshot)> {
    // 幂等重放必须使用原划转流水的 after 快照，不能泄漏后续交易形成的当前余额。
    let spot_wallet = sqlx::query_as::<_, MarginWalletAccountSnapshot>(
        r#"SELECT asset_id, available_after AS available, frozen_after AS frozen,
                  locked_after AS locked
           FROM wallet_ledger
           WHERE user_id = ? AND asset_id = ?
             AND ref_type = 'margin_transfer' AND ref_id = ?
           ORDER BY id ASC
           LIMIT 1"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(transfer_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| {
        AppError::Internal(format!(
            "margin transfer {transfer_id} is missing its spot wallet ledger snapshot"
        ))
    })?;
    let margin_wallet = sqlx::query_as::<_, MarginWalletAccountSnapshot>(
        r#"SELECT asset_id, available_after AS available, frozen_after AS frozen,
                  locked_after AS locked
           FROM margin_wallet_ledger
           WHERE user_id = ? AND asset_id = ?
             AND ref_type = 'margin_transfer' AND ref_id = ?
           ORDER BY id ASC
           LIMIT 1"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(transfer_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| {
        AppError::Internal(format!(
            "margin transfer {transfer_id} is missing its margin wallet ledger snapshot"
        ))
    })?;
    Ok((spot_wallet, margin_wallet))
}
