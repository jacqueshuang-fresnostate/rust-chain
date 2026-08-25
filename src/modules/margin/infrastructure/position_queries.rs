//! 杠杆仓位、钱包与全仓账户的只读 MySQL 适配器。
//!
//! 集中承载用户侧仓位列表与详情、杠杆钱包三桶余额、全仓账户风险快照，
//! 以及后台的仓位历史分页和按币种状态分组的利息汇总。
//! 全部查询走连接池且不加任何行锁，因此不会阻塞开仓、平仓、划转和强平的写事务。
//! 用户侧查询一律把 `user_id` 写进 WHERE 与主键联合过滤，杜绝凭仓位主键越权读取他人持仓。
//! 后台分页统一让行查询与 COUNT 复用同一组谓词构建函数，保证明细与总数口径不会分裂。

use super::query_support::{fetch_admin_page, push_user_email_filter};
use crate::{
    error::{AppError, AppResult},
    modules::margin::presentation::{
        AdminInterestSummaryItem, AdminMarginPositionResponse, MarginCrossAccountResponse,
        MarginPositionResponse, MarginWalletAccountResponse,
    },
};
use bigdecimal::BigDecimal;
use sqlx::{MySql, Pool, QueryBuilder};

#[derive(Debug, Clone, sqlx::FromRow)]
/// 风险快照所需的用户仓位、产品费率和行情交易对字段集合。
pub(crate) struct MarginRiskPositionRow {
    /// 仓位主键，回填到风险快照响应中供前端定位。
    pub(crate) id: u64,
    /// 交易对主键，与 symbol 一起用于取 Redis 行情缓存。
    pub(crate) pair_id: u64,
    /// 交易对符号，来自联表的 `trading_pairs`，是行情缓存键的组成部分。
    pub(crate) symbol: String,
    /// 交易对价格小数位，用于把账户条件强平价保守圆整到真实 tick。
    pub(crate) price_precision: i32,
    /// 保证金计价币种，浮盈、利息和权益都以该币种表示。
    pub(crate) margin_asset: u64,
    /// 持仓方向 long 或 short，决定价差的取号方式。
    pub(crate) direction: String,
    /// 保证金模式；只有 isolated 才能计算单仓预估强平价。
    pub(crate) margin_mode: String,
    /// 开仓时投入的自有保证金，是权益计算的基数。
    pub(crate) margin_amount: BigDecimal,
    /// 名义价值，等于保证金乘杠杆，浮盈和维持保证金都按它折算。
    pub(crate) notional_amount: BigDecimal,
    /// 截至当前已由利息 worker 计提的累计利息，直接抵减权益。
    pub(crate) interest_amount: BigDecimal,
    /// 入场价，未成交仓位为 NULL，此时无法计算风险，应用层会拒绝出快照。
    pub(crate) entry_price: Option<BigDecimal>,
    /// 维持保证金率，取自联表的产品当前配置，改配后立即影响强平线。
    pub(crate) maintenance_margin_rate: BigDecimal,
    /// 仓位状态，只有 opened 才允许计算实时风险。
    pub(crate) status: String,
}
/// 按用户和仓位主键联合读取风险计算所需的字段，三表内联同时取出交易对符号和产品维持保证金率。
/// 维持保证金率从产品表实时联出而非用仓位上的历史值，所以后台调整强平线会立刻反映到风险快照。
/// 未命中返回 None，由应用层统一映射为 NotFound，不区分记录不存在与归属他人。
/// 不加任何行锁，因此不会阻塞该仓位并发的平仓或强平事务，读到的是提交后的最新状态。
pub(crate) async fn load_user_risk_position_by_id(
    pool: &Pool<MySql>,
    user_id: u64,
    position_id: u64,
) -> AppResult<Option<MarginRiskPositionRow>> {
    sqlx::query_as::<_, MarginRiskPositionRow>(
        r#"SELECT positions.id, positions.pair_id, pairs.symbol, pairs.price_precision,
                  positions.margin_asset,
                  positions.direction, positions.margin_mode, positions.margin_amount, positions.notional_amount,
                  positions.interest_amount, positions.entry_price,
                  products.maintenance_margin_rate, positions.status
           FROM margin_positions positions
           INNER JOIN margin_products products ON products.id = positions.product_id
           INNER JOIN trading_pairs pairs ON pairs.id = positions.pair_id
           WHERE positions.id = ? AND positions.user_id = ?
           LIMIT 1"#,
    )
    .bind(position_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

/// 按用户与保证金资产列出权威全仓风险行，范围严格限定为 cross、opened 且入场价非空。
/// 查询同时联出 pair 符号/价格精度和产品当前维持保证金率，使 API 与强平 worker 使用同一口径。
/// pending 限价、逐仓、已关闭、其他用户或其他保证金资产的行在 SQL 谓词中直接排除，不交给应用层二次过滤。
/// 按 pair 与仓位主键升序返回以保持取价和聚合顺序稳定；本路径只读不加锁，并发终态变化会在下次请求中收敛。
pub(crate) async fn list_user_cross_margin_risk_positions(
    pool: &Pool<MySql>,
    user_id: u64,
    margin_asset: u64,
) -> AppResult<Vec<MarginRiskPositionRow>> {
    sqlx::query_as::<_, MarginRiskPositionRow>(
        r#"SELECT positions.id, positions.pair_id, pairs.symbol, pairs.price_precision,
                  positions.margin_asset, positions.direction, positions.margin_mode,
                  positions.margin_amount, positions.notional_amount, positions.interest_amount,
                  positions.entry_price, products.maintenance_margin_rate, positions.status
           FROM margin_positions positions
           INNER JOIN margin_products products ON products.id = positions.product_id
           INNER JOIN trading_pairs pairs ON pairs.id = positions.pair_id
           WHERE positions.user_id = ? AND positions.margin_asset = ?
             AND positions.margin_mode = 'cross' AND positions.status = 'opened'
             AND positions.entry_price IS NOT NULL
           ORDER BY positions.pair_id ASC, positions.id ASC"#,
    )
    .bind(user_id)
    .bind(margin_asset)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

/// 读取指定用户和资产的杠杆钱包 available，作为实时全仓风险权益的共享钱包项。
/// 账户行未建立时返回十八位零，与全仓开仓的惰性建账语义兼容；不会因为只读风险而创建空钱包。
/// 本函数不加 FOR UPDATE 且不开事务，只服务于 API 展示；真实强平仍在 worker 事务内重新锁定钱包。
pub(crate) async fn load_user_cross_margin_wallet_available(
    pool: &Pool<MySql>,
    user_id: u64,
    margin_asset: u64,
) -> AppResult<BigDecimal> {
    Ok(sqlx::query_scalar::<_, BigDecimal>(
        r#"SELECT available
           FROM margin_wallet_accounts
           WHERE user_id = ? AND asset_id = ?
           LIMIT 1"#,
    )
    .bind(user_id)
    .bind(margin_asset)
    .fetch_optional(pool)
    .await?
    .unwrap_or_else(|| BigDecimal::from(0).with_scale(18)))
}

/// 查询单个用户的杠杆仓位列表，用户标识以 `push_bind` 参数化绑定后作为第一个 WHERE 条件。
/// 状态为可选筛选，不传则四种状态混合返回；排序固定按仓位主键倒序，主键唯一因此分页稳定。
/// 只查仓位表不联产品或交易对，返回的是落库快照，不含实时浮盈也不含交易对符号。
/// 钱包汇总接口复用它并固定传 opened，因此这里的默认上限行为对两个入口一致。
pub(crate) async fn list_user_margin_positions(
    pool: &Pool<MySql>,
    user_id: u64,
    status: Option<&str>,
    limit: u32,
) -> AppResult<Vec<MarginPositionResponse>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT positions.id, positions.user_id, positions.product_id, positions.pair_id,
                  positions.margin_asset, positions.wallet_scope, positions.margin_mode,
                  positions.direction, positions.order_type, positions.margin_amount, positions.leverage,
                  positions.notional_amount, positions.borrowed_amount, positions.interest_amount,
                  positions.entry_price, positions.limit_price, positions.exit_price, positions.realized_pnl,
                  positions.closed_at, positions.status, positions.idempotency_key
           FROM margin_positions positions
           WHERE positions.user_id = "#,
    );
    builder.push_bind(user_id);
    if let Some(status) = status {
        builder.push(" AND positions.status = ");
        builder.push_bind(status);
    }
    builder.push(" ORDER BY positions.id DESC LIMIT ");
    builder.push_bind(limit as i64);
    builder
        .build_query_as::<MarginPositionResponse>()
        .fetch_all(pool)
        .await
        .map_err(AppError::from)
}

/// 读取用户可用的杠杆资产目录与 available、frozen、locked 三桶余额，并补上资产符号与图标。
/// 已开启杠杆转入的 active 资产即使尚未建账也用零余额返回，方便客户端展示可转入目录；
/// 已经存在杠杆钱包的资产即使后来关闭转入开关也继续返回，避免隐藏用户存量余额。
/// 按资产主键升序排列以保证前端展示顺序稳定；左连接只读，不创建空账户、不改余额、不生成流水。
pub(crate) async fn list_margin_wallet_accounts(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<Vec<MarginWalletAccountResponse>> {
    sqlx::query_as::<_, MarginWalletAccountResponse>(
        r#"SELECT assets.id AS asset_id,
                  assets.symbol AS asset_symbol,
                  assets.logo_url,
                  assets.margin_transfer_enabled,
                  COALESCE(wallets.available, 0) AS available,
                  COALESCE(wallets.frozen, 0) AS frozen,
                  COALESCE(wallets.locked, 0) AS locked,
                  COALESCE(wallets.available, 0) AS max_transferable_to_spot,
                  NULL AS transfer_to_spot_block_reason,
                  NULL AS cross_account_version,
                  NULL AS transfer_risk_equity,
                  NULL AS transfer_risk_maintenance_margin,
                  NULL AS transfer_risk_observed_at
           FROM assets
           LEFT JOIN margin_wallet_accounts wallets
             ON wallets.asset_id = assets.id AND wallets.user_id = ?
           WHERE (assets.status = 'active' AND assets.margin_transfer_enabled = TRUE)
              OR wallets.id IS NOT NULL
           ORDER BY assets.id ASC"#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

/// 读取用户各保证金币种下的全仓账户风险快照，按保证金币种升序返回。
/// 五个数值列在表里都以 `last_` 前缀存储，查询时改名为业务字段，语义是「强平 worker 上次评估的结果」，
/// 因此可能滞后于最新行情，展示层不应把它当作实时权益，实时值需走单仓风险快照接口。
/// 保证金率在维持保证金为零时落库为 NULL，映射成 Option 表示该比率无意义而非零。
/// 纯读取，不重新估值、不触发强平，也不写回任何字段。
pub(crate) async fn list_user_cross_margin_accounts(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<Vec<MarginCrossAccountResponse>> {
    sqlx::query_as::<_, MarginCrossAccountResponse>(
        r#"SELECT margin_asset, status, last_equity AS equity,
                  last_unrealized_pnl AS unrealized_pnl,
                  last_interest_amount AS interest_amount,
                  last_maintenance_margin AS maintenance_margin,
                  last_margin_ratio AS margin_ratio
           FROM margin_cross_accounts
           WHERE user_id = ?
           ORDER BY margin_asset ASC"#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

/// 按仓位主键与用户标识联合读取单条仓位详情，两个条件同时命中才返回结果。
/// 返回列与用户仓位列表完全一致，因此详情页和列表页的字段解析可以共用一套逻辑。
/// 未命中返回 None 而不是错误，由应用层统一映射为 NotFound，避免在这一层决定 HTTP 语义。
/// 走连接池只读，不加锁也不联表，取到的是仓位行的落库快照，不含强平时间和强平原因。
pub(crate) async fn load_user_position_by_id(
    pool: &Pool<MySql>,
    user_id: u64,
    position_id: u64,
) -> AppResult<Option<MarginPositionResponse>> {
    sqlx::query_as::<_, MarginPositionResponse>(
        r#"SELECT id, user_id, product_id, pair_id, margin_asset, wallet_scope, margin_mode, direction, order_type, margin_amount,
                  leverage, notional_amount, borrowed_amount, interest_amount, entry_price, limit_price,
                  exit_price, realized_pnl, closed_at, status, idempotency_key
           FROM margin_positions
           WHERE id = ? AND user_id = ?
           LIMIT 1"#,
    )
    .bind(position_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

/// 后台仓位列表：行查询与 COUNT 共用同一组谓词，总数才会跟随当前筛选。
/// 后台仓位行与总数共享用户、邮箱、交易对及状态筛选，不修改仓位。
///
/// 相比用户侧列表多取 `liquidated_at` 与 `liquidation_reason` 两列，用于复盘风控处置过程。
/// 邮箱条件因为需要访问用户表，所以要在两个 builder 上各克隆一份，共享谓词函数保证写法一致。
/// 排序按仓位主键倒序，主键唯一，深翻页时不会出现同一条记录跨页重复或被跳过。
pub(crate) async fn list_admin_margin_positions(
    pool: &Pool<MySql>,
    user_id: Option<u64>,
    email: Option<String>,
    pair_id: Option<u64>,
    status: Option<&str>,
    limit: u32,
    offset: u32,
) -> AppResult<(Vec<AdminMarginPositionResponse>, i64)> {
    let mut rows = QueryBuilder::<MySql>::new(
        r#"SELECT id, user_id, product_id, pair_id, margin_asset, wallet_scope, margin_mode, direction, order_type, margin_amount,
                  leverage, notional_amount, borrowed_amount, interest_amount, entry_price, limit_price,
                  exit_price, realized_pnl, closed_at, liquidated_at, liquidation_reason, status,
                  idempotency_key
           FROM margin_positions"#,
    );
    let mut total = QueryBuilder::<MySql>::new("SELECT COUNT(*) FROM margin_positions");
    for builder in [&mut rows, &mut total] {
        push_admin_margin_position_filters(builder, user_id, email.clone(), pair_id, status, false);
    }

    fetch_admin_page(pool, rows, total, " ORDER BY id DESC", limit, offset).await
}

/// 向后台仓位查询追加四个可选筛选条件及可选的「仅已成交」边界，是明细分页与利息汇总共用的唯一谓词来源。
/// 先落一个恒真的 `WHERE 1 = 1`，后续条件无需判断是不是第一个即可统一用 AND 拼接。
/// 用户标识、交易对和状态都走参数化绑定；邮箱条件交由共享助手拼成 EXISTS 子查询。
/// 后台仓位明细传 false 以继续展示待成交委托；利息汇总传 true，禁止把 `entry_price IS NULL` 的挂单借款快照计入资金费口径。
/// 因为行查询和 COUNT 查询都调用它，新增筛选维度时不可能只改一侧而造成总数口径偏差。
fn push_admin_margin_position_filters(
    builder: &mut QueryBuilder<'_, MySql>,
    user_id: Option<u64>,
    email: Option<String>,
    pair_id: Option<u64>,
    status: Option<&str>,
    filled_only: bool,
) {
    builder.push(" WHERE 1 = 1");
    if filled_only {
        builder.push(" AND entry_price IS NOT NULL");
    }
    if let Some(user_id) = user_id {
        builder.push(" AND user_id = ");
        builder.push_bind(user_id);
    }
    push_user_email_filter(builder, "user_id", email);
    if let Some(pair_id) = pair_id {
        builder.push(" AND pair_id = ");
        builder.push_bind(pair_id);
    }
    if let Some(status) = status {
        builder.push(" AND status = ");
        builder.push_bind(status.to_owned());
    }
}

/// 后台按仓位主键读取单条详情，不带用户维度约束，可查看任意账户的持仓。
/// 返回列与后台列表一致，含强平时间与强平原因，便于详情页和列表页共用同一套解析。
/// 未命中返回 None，由应用层映射为 NotFound；不加行锁，也不会顺带触发计提或结算。
pub(crate) async fn load_admin_margin_position_by_id(
    pool: &Pool<MySql>,
    position_id: u64,
) -> AppResult<Option<AdminMarginPositionResponse>> {
    sqlx::query_as::<_, AdminMarginPositionResponse>(
        r#"SELECT id, user_id, product_id, pair_id, margin_asset, wallet_scope, margin_mode, direction, order_type, margin_amount,
                  leverage, notional_amount, borrowed_amount, interest_amount, entry_price, limit_price,
                  exit_price, realized_pnl, closed_at, liquidated_at, liquidation_reason, status,
                  idempotency_key
           FROM margin_positions
           WHERE id = ?
           LIMIT 1"#,
    )
    .bind(position_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

/// 后台资金费汇总：只聚合已成交仓位，分组行与分组总数共用同一组谓词，总数按分组键去重统计。
/// 按同一筛选聚合仓位利息与分页总数，该查询不执行计提。
///
/// 按保证金币种与仓位状态两列分组，输出仓位笔数、借款额合计和已计提利息合计。
/// 两个合计用 COALESCE 兜底为零，避免筛选后没有匹配行时返回 NULL 而反序列化失败。
/// 总数写成 `COUNT(DISTINCT margin_asset, status)`，统计的是分组个数而非仓位条数，与分页语义对齐。
/// 数值全部取自仓位行上由利息 worker 写入的既有列，本查询只做聚合，不执行任何计提或结算。
pub(crate) async fn list_admin_interest_summary(
    pool: &Pool<MySql>,
    user_id: Option<u64>,
    email: Option<String>,
    pair_id: Option<u64>,
    status: Option<&str>,
    limit: u32,
    offset: u32,
) -> AppResult<(Vec<AdminInterestSummaryItem>, i64)> {
    let mut rows = QueryBuilder::<MySql>::new(
        r#"SELECT margin_asset, status, COUNT(*) AS position_count,
                  COALESCE(SUM(borrowed_amount), 0) AS borrowed_amount,
                  COALESCE(SUM(interest_amount), 0) AS interest_amount
           FROM margin_positions"#,
    );
    let mut total = QueryBuilder::<MySql>::new(
        "SELECT COUNT(DISTINCT margin_asset, status) FROM margin_positions",
    );
    for builder in [&mut rows, &mut total] {
        push_admin_margin_position_filters(builder, user_id, email.clone(), pair_id, status, true);
    }
    // 分组键 (margin_asset, status) 本身唯一，排序无需再补主键。
    rows.push(" GROUP BY margin_asset, status");

    fetch_admin_page(
        pool,
        rows,
        total,
        " ORDER BY margin_asset ASC, status ASC",
        limit,
        offset,
    )
    .await
}
