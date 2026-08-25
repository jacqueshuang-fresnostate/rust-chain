//! 杠杆上下文内钱包行锁与资金流水的底层适配器。
//!
//! 划转、开仓扣抵押、平仓返还和强平结算都通过这里取得钱包行锁并写入配套流水，
//! 集中在一处是为了让现货钱包和杠杆钱包的加锁写法、三桶快照口径和流水字段保持完全一致。
//! 现货钱包要求账户已存在，杠杆钱包则在加锁前用 INSERT IGNORE 惰性补一行零余额。
//! 所有流水的 `balance_type` 固定为 available，因为杠杆业务只在可用桶内加减，不使用冻结语义；
//! frozen 与 locked 只是被原样记入快照，用于事后对账时还原当时的完整余额结构。
//! 本文件不定义事务边界，也不做任何幂等判定，重复记账的防护由上层的幂等键和仓位终态负责。

use crate::error::{AppError, AppResult};
use bigdecimal::BigDecimal;
use sqlx::{MySql, Transaction};

#[derive(Debug, sqlx::FromRow)]
/// 钱包行锁读取的可用、冻结与锁定三桶快照，后续资金流水必须引用同一版本。
pub(super) struct MarginWalletRow {
    /// 可用余额，杠杆业务唯一会被加减的一桶。
    pub(super) available: BigDecimal,
    /// 冻结余额，杠杆路径不修改，仅原样写入流水快照。
    pub(super) frozen: BigDecimal,
    /// 锁定余额，同样只读不改，余额不足的错误文案里会带上它便于排查。
    pub(super) locked: BigDecimal,
}
/// 在调用方事务内对用户现货钱包执行 FOR UPDATE，固定保证金扣款或退款前的三桶余额。
/// 账户不存在或锁失败即终止；本函数不改余额，调用方须按统一锁序继续并同事务写流水。
///
/// 与杠杆钱包的关键差别是这里不自动建账：现货账户在用户注册时已初始化，查不到属于异常，
/// 因此返回校验错误而不是补一行零余额，避免掩盖账户体系被破坏的问题。
/// 划转路径固定第一个调用它，杠杆钱包锁排在其后，这个顺序是双向划转不死锁的前提。
pub(super) async fn lock_spot_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<MarginWalletRow> {
    sqlx::query_as::<_, MarginWalletRow>(
        r#"SELECT available, frozen, locked
           FROM wallet_accounts
           WHERE user_id = ? AND asset_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::Validation("wallet account is required for margin".to_owned()))
}

/// 在调用方事务内用 INSERT IGNORE 惰性补一行零余额杠杆钱包，供后续行锁与划转使用。
/// 依赖 (user_id, asset_id) 唯一键实现幂等：账户已存在时语句被静默忽略，绝不会覆盖既有余额。
/// 三桶全部初始化为零，因此新建即用不需要额外初始化步骤。
/// 该步骤不写资金流水也不独立提交，失败时随调用方事务一起回滚。
pub(super) async fn ensure_margin_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT IGNORE INTO margin_wallet_accounts (user_id, asset_id, available, frozen, locked)
           VALUES (?, ?, 0, 0, 0)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 在调用方事务内确保并锁定用户保证金钱包，固定划转、开仓或结算使用的余额快照。
/// 调用方须遵守现货后保证金的锁序；锁取失败时不得继续余额与流水写入。
///
/// 先补账户再加锁，因此对首次使用某币种的用户也能成功返回一份全零快照，
/// 用 `fetch_one` 而非 `fetch_optional` 正是基于这个前置保证，查不到即视为异常上抛。
/// 全仓开仓、全仓结算和划转入账都走它，逐仓开仓则用不建账的版本以便判断能否回退现货。
pub(super) async fn lock_margin_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<MarginWalletRow> {
    ensure_margin_wallet_row(tx, user_id, asset_id).await?;
    sqlx::query_as::<_, MarginWalletRow>(
        r#"SELECT available, frozen, locked
           FROM margin_wallet_accounts
           WHERE user_id = ? AND asset_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(AppError::from)
}

/// 在调用方事务内尝试锁定既有保证金钱包；账户不存在返回空而不自动建账。
/// 该读锁不改变余额，调用方根据结果选择资金域并负责同事务提交或回滚。
///
/// 专供逐仓开仓的资金域选择：返回 None 或余额不足时调用方回退到现货钱包扣款。
/// 如果这里像 `lock_margin_wallet_row` 那样自动建账，就会给从未参与杠杆的用户凭空建出空账户，
/// 且无法区分「没有账户」和「账户余额为零」，所以刻意保留不建账的语义。
pub(super) async fn lock_existing_margin_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<Option<MarginWalletRow>> {
    sqlx::query_as::<_, MarginWalletRow>(
        r#"SELECT available, frozen, locked
           FROM margin_wallet_accounts
           WHERE user_id = ? AND asset_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::from)
}

/// 不加锁预读逐仓可用余额，只用于在事务取得任何钱包锁前选择唯一资金域。
/// 真正扣款仍会在所选钱包的 FOR UPDATE 快照上复核，预读变化时失败重试而不改走另一把锁。
pub(super) async fn load_existing_margin_wallet_available(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<Option<BigDecimal>> {
    sqlx::query_scalar::<_, BigDecimal>(
        r#"SELECT available
           FROM margin_wallet_accounts
           WHERE user_id = ? AND asset_id = ?
           LIMIT 1"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::from)
}

#[allow(clippy::too_many_arguments)] // 账本必须显式记录三桶快照和业务引用，聚合会降低资金审计可读性。
/// 在调用方事务内追加现货钱包流水，三桶余额后快照必须对应同次保证金资金变更。
/// 写入失败由调用方连同钱包与仓位回滚；同一业务重放不得产生第二笔流水。
///
/// 写的是全局现货流水表 `wallet_ledger`，杠杆业务在其中通过 `change_type` 与 `ref_type` 区分自己的记录，
/// 例如划转出账、开仓扣抵押和平仓返还各用不同类型，`ref_id` 指向划转编号或仓位主键。
/// `amount` 带符号，扣减传负、入账传正，配合 after 快照即可离线复算任意时点的余额。
/// 本函数不做去重，重复调用会写出两条流水，防重完全依赖上层的幂等键和仓位终态判定。
pub(super) async fn insert_spot_wallet_ledger(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    change_type: &str,
    amount: &BigDecimal,
    balance_after: &BigDecimal,
    available_after: &BigDecimal,
    frozen_after: &BigDecimal,
    locked_after: &BigDecimal,
    ref_type: &str,
    ref_id: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, ?, ?, 'available', ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(change_type)
    .bind(amount)
    .bind(balance_after)
    .bind(available_after)
    .bind(frozen_after)
    .bind(locked_after)
    .bind(ref_type)
    .bind(ref_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)] // 账本必须显式记录三桶快照和业务引用，聚合会降低资金审计可读性。
/// 在调用方事务内追加保证金钱包流水，金额及三桶快照必须与余额更新保持一致。
/// 写入失败由调用方整体回滚；幂等键或仓位终态须阻止重复记账。
///
/// 写的是独立的 `margin_wallet_ledger` 表，与现货流水物理隔离，两账套各自对账互不干扰。
/// 全仓的两类结算在这里留痕：主动单仓平仓记有符号权益，账户级强平则恰好记一条
/// `-available_before` 流水并把 available 归零，不把仓位权益正向入账；穿仓缺口独立登记在全仓账户坏账字段。
/// 划转的入账与出账也各写一条，与现货侧同 `transfer_id` 配对，构成可交叉核对的双边记录。
pub(super) async fn insert_margin_wallet_ledger(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    change_type: &str,
    amount: &BigDecimal,
    balance_after: &BigDecimal,
    available_after: &BigDecimal,
    frozen_after: &BigDecimal,
    locked_after: &BigDecimal,
    ref_type: &str,
    ref_id: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO margin_wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, ?, ?, 'available', ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(change_type)
    .bind(amount)
    .bind(balance_after)
    .bind(available_after)
    .bind(frozen_after)
    .bind(locked_after)
    .bind(ref_type)
    .bind(ref_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
