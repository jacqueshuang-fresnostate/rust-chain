//! 杠杆仓位写路径与行锁适配器。
//!
//! 覆盖开仓时的产品锁定、仓位插入与幂等键占位、资金域回写、全仓账户补建，
//! 以及平仓和撤销前的仓位行锁与批量候选主键枚举。
//! 锁序在整个上下文内统一为「先产品或仓位，后钱包」，本文件只负责前半段，
//! 钱包锁由结算与划转适配器在同一事务的后续步骤取得，两者顺序不可颠倒。
//! 插入仓位的返回类型保留原始 `sqlx::Error`，因为调用方需要据唯一键冲突判定幂等重放。
//! 全部函数都在调用方给定的事务或连接池上执行，自身不 begin、不 commit、不 rollback。

use super::query_support::zero_amount;
use crate::{
    error::{AppError, AppResult},
    modules::margin::presentation::MarginPositionResponse,
};
use bigdecimal::BigDecimal;
use sqlx::{MySql, Pool, QueryBuilder, Transaction, types::Json as SqlxJson};

#[derive(Debug, sqlx::FromRow)]
/// 开仓事务锁定的产品、交易对、保证金币种、模式和杠杆规则快照。
pub(crate) struct MarginOpenProductRule {
    /// 产品主键，写入仓位行的 `product_id`。
    pub(crate) id: u64,
    /// 交易对主键，同时用于写仓位和拼行情缓存键。
    pub(crate) pair_id: u64,
    /// 交易对符号，来自联表，取入场价时作为行情缓存键的一部分。
    pub(crate) symbol: String,
    /// 保证金计价币种，决定抵押从哪个资产的钱包扣。
    pub(crate) margin_asset: u64,
    /// 产品默认保证金模式，请求未指定模式时采用。
    pub(crate) margin_mode: String,
    /// 产品支持的保证金模式集合，选定模式必须在其中。
    pub(crate) margin_modes: SqlxJson<Vec<String>>,
    /// 可选杠杆档位文本，请求杠杆必须精确命中其中一项。
    pub(crate) leverage_levels: SqlxJson<Vec<String>>,
    /// 单笔开仓最小保证金额，低于此值拒绝开仓。
    pub(crate) min_margin: BigDecimal,
    /// 单笔开仓最大保证金额，None 表示不设上限。
    pub(crate) max_margin: Option<BigDecimal>,
    /// 借款小时利率，开仓时复查其合法性，之后由利息 worker 按它计提。
    pub(crate) hourly_interest_rate: BigDecimal,
    /// 产品状态，锁定后立即判定，非 active 一律按 NotFound 处理。
    pub(crate) status: String,
}

#[derive(Debug, sqlx::FromRow)]
/// 关闭或取消事务锁定的仓位快照，包含资金域、入场价、利息和当前状态。
pub(crate) struct LockedMarginPositionRow {
    /// 仓位主键，后续结算、流水引用和状态迁移都以它定位。
    pub(crate) id: u64,
    /// 交易对主键，与 symbol 一起用于取平仓所需的标记价。
    pub(crate) pair_id: u64,
    /// 交易对符号，来自联表的 `trading_pairs`。
    pub(crate) symbol: String,
    /// 保证金币种，决定资金退回哪个资产的钱包。
    pub(crate) margin_asset: u64,
    /// 开仓时实际扣款的资金域 spot 或 margin，平仓与撤销据此原路返还。
    pub(crate) wallet_scope: String,
    /// 保证金模式，决定平仓走逐仓非负返还还是全仓有符号权益结算。
    pub(crate) margin_mode: String,
    /// 持仓方向 long 或 short，决定平仓盈亏的价差取号。
    pub(crate) direction: String,
    /// 开仓投入的自有保证金，是撤销退款额和平仓权益计算的基数。
    pub(crate) margin_amount: BigDecimal,
    /// 名义价值，平仓时按它与价格变动比例折算已实现盈亏。
    pub(crate) notional_amount: BigDecimal,
    /// 已计提的累计利息，平仓时从权益中扣除。
    pub(crate) interest_amount: BigDecimal,
    /// 入场价，为 NULL 表示仓位未成交，是撤销与平仓的分流依据。
    pub(crate) entry_price: Option<BigDecimal>,
    /// 加锁瞬间的仓位状态，非 opened 时按终态重放处理而不重复结算。
    pub(crate) status: String,
}
/// 枚举用户可平仓的仓位主键，条件是状态 opened 且入场价非空，即已真实成交的持仓。
/// 可选的 `product_id` 用于把批量平仓收窄到单个产品，不传则覆盖该用户全部持仓。
/// 按主键升序返回，与撤销枚举保持同一顺序，让两类批处理的加锁次序一致，减少互相阻塞的机会。
/// 只取主键不取整行，因为批处理会逐笔重新开事务加锁读取，这里读到的行随后可能已被强平改变。
pub(crate) async fn load_open_position_ids(
    pool: &Pool<MySql>,
    user_id: u64,
    product_id: Option<u64>,
) -> AppResult<Vec<u64>> {
    let mut builder =
        QueryBuilder::<MySql>::new("SELECT id FROM margin_positions WHERE user_id = ");
    builder.push_bind(user_id);
    builder.push(" AND status = 'opened' AND entry_price IS NOT NULL");
    if let Some(product_id) = product_id {
        builder.push(" AND product_id = ");
        builder.push_bind(product_id);
    }
    builder.push(" ORDER BY id ASC");
    builder
        .build_query_scalar::<u64>()
        .fetch_all(pool)
        .await
        .map_err(AppError::from)
}

/// 枚举用户可撤销的仓位主键，条件是状态 opened 且入场价为 NULL，即尚未成交的挂单式仓位。
/// 与可平仓枚举互为补集，两者的唯一差别就是入场价的空与非空，因此同一仓位不会同时出现在两个列表里。
/// 可选的 `product_id` 同样用于收窄范围；返回按主键升序，供批量撤销逐笔独立加锁处理。
/// 这里的候选身份只是初筛，真正的可撤销判定在加锁后由应用层重新校验，避免并发成交造成误撤。
pub(crate) async fn load_cancelable_position_ids(
    pool: &Pool<MySql>,
    user_id: u64,
    product_id: Option<u64>,
) -> AppResult<Vec<u64>> {
    let mut builder =
        QueryBuilder::<MySql>::new("SELECT id FROM margin_positions WHERE user_id = ");
    builder.push_bind(user_id);
    builder.push(" AND status = 'opened' AND entry_price IS NULL");
    if let Some(product_id) = product_id {
        builder.push(" AND product_id = ");
        builder.push_bind(product_id);
    }
    builder.push(" ORDER BY id ASC");
    builder
        .build_query_scalar::<u64>()
        .fetch_all(pool)
        .await
        .map_err(AppError::from)
}

/// 在开仓事务内按用户与幂等键锁定既有仓位，用于并发重复请求的逐字段核对。
/// 未命中不产生写入；命中后调用方必须复用原结果或在异参时返回冲突。
///
/// 之所以带 FOR UPDATE，是因为它专门服务于唯一键冲突之后的重放：此刻另一个事务可能正在提交，
/// 加锁读会等待对方落定，避免在提交瞬间读到空结果而把重放误判成新请求。
/// 幂等键的唯一性作用域是单个用户，SQL 因此同时约束 `user_id` 和 `idempotency_key`。
pub(crate) async fn existing_position_for_idempotency_key(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    idempotency_key: &str,
) -> AppResult<Option<MarginPositionResponse>> {
    sqlx::query_as::<_, MarginPositionResponse>(
        r#"SELECT id, user_id, product_id, pair_id, margin_asset, wallet_scope, margin_mode, direction, margin_amount,
                  leverage, notional_amount, borrowed_amount, interest_amount, entry_price,
                  exit_price, realized_pnl, closed_at, status, idempotency_key
           FROM margin_positions
           WHERE user_id = ? AND idempotency_key = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .bind(idempotency_key)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::from)
}

/// 在事务前只读检查用户幂等键对应仓位，便于重放绕过行情和钱包扣款。
/// 异参判断由应用层执行；查询失败不得继续创建新仓位。
///
/// 与加锁版本 SQL 只差一个 FOR UPDATE，用于开仓最开始的快速预检：命中即可直接返回原仓位，
/// 连 Redis 行情和产品行锁都不必碰，让重复提交的成本降到一次索引查找。
/// 不加锁意味着并发首次请求尚未提交时这里读不到，会放行到插入分支，由唯一键冲突兜底。
pub(crate) async fn existing_position_for_idempotency_key_readonly(
    pool: &Pool<MySql>,
    user_id: u64,
    idempotency_key: &str,
) -> AppResult<Option<MarginPositionResponse>> {
    sqlx::query_as::<_, MarginPositionResponse>(
        r#"SELECT id, user_id, product_id, pair_id, margin_asset, wallet_scope, margin_mode, direction, margin_amount,
                  leverage, notional_amount, borrowed_amount, interest_amount, entry_price,
                  exit_price, realized_pnl, closed_at, status, idempotency_key
           FROM margin_positions
           WHERE user_id = ? AND idempotency_key = ?
           LIMIT 1"#,
    )
    .bind(user_id)
    .bind(idempotency_key)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

/// 在开仓事务内锁定启用产品及交易对规则，固定杠杆、模式和保证金币种快照。
/// 产品不存在或停用即失败；调用方在该锁后写仓位并扣抵押。
///
/// 与用户设置路径不同，这里的状态条件不写进 WHERE 而是先锁行再判 `status != "active"`，
/// 因此停用产品也会被短暂加锁，但同样返回 NotFound，对外不区分不存在与已停用。
/// 该锁是整个开仓事务的第一把锁，持有到提交，把杠杆档位、保证金区间和利率固定在同一版本上，
/// 保证并发改配不会让校验依据在校验通过后失效。
pub(crate) async fn lock_active_open_product(
    tx: &mut Transaction<'_, MySql>,
    product_id: u64,
) -> AppResult<MarginOpenProductRule> {
    let product = sqlx::query_as::<_, MarginOpenProductRule>(
        r#"SELECT products.id, products.pair_id, pairs.symbol, products.margin_asset,
                  products.margin_mode, products.margin_modes, products.leverage_levels, products.min_margin,
                  products.max_margin, products.hourly_interest_rate, products.status
           FROM margin_products products
           INNER JOIN trading_pairs pairs ON pairs.id = products.pair_id
           WHERE products.id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(product_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    if product.status != "active" {
        return Err(AppError::NotFound);
    }
    Ok(product)
}

#[allow(clippy::too_many_arguments)] // 仓位快照字段与 SQL 列一一对应，事务边界由应用层持有。
/// 在调用方事务内写入开仓快照并占用用户幂等键，入场价来自服务端行情缓存。
/// 唯一键冲突交由应用层重放核对；插入失败不得继续扣抵押或写资金流水。
///
/// 状态硬编码为 opened，累计利息初始化为十八位精度的零，计提起点 `interest_accrued_at`
/// 取数据库当前时间而非应用侧时间，避免多实例时钟漂移导致首个计息窗口偏长或偏短。
/// `wallet_scope` 不在这里写入，因为实际扣款账户要等结算适配器选完才知道，由后续语句回填。
/// 返回类型保留原始 `sqlx::Error`，调用方据此区分唯一键冲突与真实数据库故障。
/// 该插入必须先于任何钱包扣款执行，先占键后扣钱是防止同键并发重复扣抵押的核心顺序。
pub(crate) async fn insert_margin_position(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    product: &MarginOpenProductRule,
    margin_mode: &str,
    direction: &str,
    margin_amount: &BigDecimal,
    leverage: &BigDecimal,
    notional_amount: &BigDecimal,
    borrowed_amount: &BigDecimal,
    entry_price: &BigDecimal,
    idempotency_key: &str,
) -> Result<u64, sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO margin_positions
           (user_id, product_id, pair_id, margin_asset, margin_mode, direction, margin_amount,
            leverage, notional_amount, borrowed_amount, interest_amount, interest_accrued_at,
            entry_price, status, idempotency_key)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP(6), ?, 'opened', ?)"#,
    )
    .bind(user_id)
    .bind(product.id)
    .bind(product.pair_id)
    .bind(product.margin_asset)
    .bind(margin_mode)
    .bind(direction)
    .bind(margin_amount)
    .bind(leverage)
    .bind(notional_amount)
    .bind(borrowed_amount)
    .bind(zero_amount())
    .bind(entry_price)
    .bind(idempotency_key)
    .execute(&mut **tx)
    .await
    .map(|result| result.last_insert_id())
}

/// 在开仓事务内记录抵押实际来源为 spot 或 margin，供关闭、取消和强平原路返还。
/// 更新失败须回滚仓位、钱包与流水，禁止以默认资金域替代缺失快照。
///
/// 之所以要在插入之后单独回填，是因为逐仓开仓允许先试杠杆钱包、余额不足再回退现货，
/// 实际扣的是哪个账户只有在结算适配器执行完才确定，插入仓位时无法预知。
/// 这一列一旦写错，平仓和撤销就会把钱退到另一个账户，因此它与扣款必须在同一事务内成对提交。
pub(crate) async fn set_margin_position_wallet_scope(
    tx: &mut Transaction<'_, MySql>,
    position_id: u64,
    wallet_scope: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE margin_positions SET wallet_scope = ? WHERE id = ?")
        .bind(wallet_scope)
        .bind(position_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// 确保全仓账户存在；账户按用户和保证金资产唯一，避免不同交易对各自分账。
/// 用 INSERT ... ON DUPLICATE KEY UPDATE 把状态重置为 active，因此曾被强平置为其他状态的账户
/// 会在再次开全仓时自动恢复可用，同时这条语句天然幂等，并发或重放都不会产生第二行。
/// 只保证账户行存在，不初始化任何权益字段，那些 `last_` 列由强平 worker 后续刷新。
/// 只有全仓开仓才调用它，逐仓不需要账户级聚合。
pub(crate) async fn ensure_cross_margin_account(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    margin_asset: u64,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO margin_cross_accounts (user_id, margin_asset) VALUES (?, ?) ON DUPLICATE KEY UPDATE status = 'active'",
    )
    .bind(user_id)
    .bind(margin_asset)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 按用户和仓位标识执行 FOR UPDATE，固定关闭或取消前的状态与资金来源。
/// 未命中返回空；调用方必须先锁仓位再锁钱包，并在同一事务更新终态。
///
/// 这是平仓与撤销的第一把锁，把状态、入场价、资金域和利息固定在同一版本上，
/// 使后续的模式分流、可撤销判定和金额计算不会被并发的强平或另一次平仓插入中间。
/// 联表取交易对符号，是为了让平仓路径能直接拿它去查行情缓存而无需二次查询。
/// 用户标识写进 WHERE 而非事后校验，防止越权操作他人仓位；未命中时返回 None 由应用层映射 NotFound。
pub(crate) async fn lock_user_position_by_id(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    position_id: u64,
) -> AppResult<Option<LockedMarginPositionRow>> {
    sqlx::query_as::<_, LockedMarginPositionRow>(
        r#"SELECT positions.id, positions.pair_id, pairs.symbol,
                  positions.margin_asset, positions.wallet_scope, positions.margin_mode,
                  positions.direction, positions.margin_amount,
                  positions.notional_amount, positions.interest_amount, positions.entry_price,
                  positions.status
           FROM margin_positions positions
           INNER JOIN trading_pairs pairs ON pairs.id = positions.pair_id
           WHERE positions.id = ? AND positions.user_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(position_id)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::from)
}
