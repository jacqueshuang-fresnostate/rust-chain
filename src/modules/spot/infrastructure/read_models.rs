//! 现货订单/成交读模型与旧 `SpotRepository` SQLx 适配器。
//!
//! 本模块只拥有连接池级读取和旧仓储 CRUD；分页行查询与 COUNT 始终复用同一筛选谓词。
//! 它不开始资金事务，也不修改钱包余额或流水。

use super::common::{
    SYSTEM_SPOT_LIQUIDITY_EMAIL, map_spot_sqlx_error, order_side_as_str, order_status_as_str,
    order_type_as_str, parse_order_side, parse_order_status, parse_order_type,
    parse_spot_u64_identifier,
};
use crate::{
    error::{AppError, AppResult},
    modules::spot::{
        NewOrder, NewSpotTrade, SpotOrder, SpotTrade,
        presentation::{SpotOrderResponse, SpotTradeResponse},
    },
};
use bigdecimal::BigDecimal;
use sqlx::{MySql, Pool, QueryBuilder};

/// 分页排序必须带唯一列 id，否则同一 created_at 的行会在页间重复或丢失。
const SPOT_ORDER_ORDER_BY: &str = " ORDER BY orders.created_at DESC, orders.id DESC";
const SPOT_TRADE_ORDER_BY: &str = " ORDER BY trades.created_at DESC, trades.id DESC";

/// 行查询与 COUNT 查询必须由同一组过滤谓词构建，返回总数才能与当前筛选一致。
async fn fetch_admin_page<T>(
    pool: &Pool<MySql>,
    mut rows: QueryBuilder<'_, MySql>,
    mut total: QueryBuilder<'_, MySql>,
    order_by: &str,
    limit: u32,
    offset: u32,
) -> AppResult<(Vec<T>, i64)>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
{
    rows.push(order_by);
    rows.push(" LIMIT ");
    rows.push_bind(limit as i64);
    rows.push(" OFFSET ");
    rows.push_bind(offset as i64);

    let items = rows.build_query_as::<T>().fetch_all(pool).await?;
    let total = total.build_query_scalar::<i64>().fetch_one(pool).await?;

    Ok((items, total))
}

pub(crate) struct SpotOrderListFilter {
    pub(crate) user_id: Option<u64>,
    pub(crate) pair_id: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) email: Option<String>,
    pub(crate) include_internal: bool,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
}

pub(crate) struct SpotTradeListFilter {
    pub(crate) pair_id: Option<String>,
    pub(crate) user_id: Option<u64>,
    pub(crate) email: Option<String>,
    pub(crate) include_internal: bool,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
}

#[derive(Debug, sqlx::FromRow)]
pub(super) struct SpotOrderQueryRow {
    id: u64,
    user_id: u64,
    user_email: Option<String>,
    pair_id: String,
    side: String,
    order_type: String,
    price: Option<BigDecimal>,
    trigger_price: Option<BigDecimal>,
    quantity: BigDecimal,
    filled_quantity: BigDecimal,
    average_price: Option<BigDecimal>,
    status: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, sqlx::FromRow)]
pub(super) struct SpotTradeQueryRow {
    id: u64,
    pair_id: String,
    buy_order_id: u64,
    sell_order_id: u64,
    price: BigDecimal,
    quantity: BigDecimal,
    fee: BigDecimal,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct MySqlSpotRepository {
    pool: Pool<MySql>,
}

impl MySqlSpotRepository {
    /// 保存 MySQL 池供现货读模型、幂等查询与订单仓储方法复用；构造时不获取连接或锁订单。
    pub fn new(pool: Pool<MySql>) -> Self {
        Self { pool }
    }

    /// 返回MySQL 连接池引用，该只读访问不会触发外部查询或业务状态变更。
    pub fn pool(&self) -> &Pool<MySql> {
        &self.pool
    }

    /// 从 MySQL 持久化数据读取交易对规则，保持现货既有归属过滤、可见性及排序条件。
    /// 通过仓储适配器读取现货交易对规则；不存在返回领域仓储错误。
    pub async fn load_pair_rule_async(
        &self,
        pair_id: &str,
    ) -> Result<crate::modules::spot::TradingPairRule, crate::modules::spot::SpotServiceError> {
        let row = sqlx::query_as::<_, (u64, String, i32, i32, BigDecimal, String)>(
            r#"SELECT id, symbol, price_precision, qty_precision, min_order_value, status
               FROM trading_pairs
               WHERE symbol = ? OR id = ?
               LIMIT 1"#,
        )
        .bind(pair_id)
        .bind(pair_id.parse::<u64>().ok())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_spot_sqlx_error)?
        .ok_or_else(|| {
            crate::modules::spot::SpotServiceError::Repository(format!(
                "missing trading pair: {pair_id}"
            ))
        })?;

        let (_id, symbol, price_precision, quantity_precision, min_order_value, status) = row;
        Ok(crate::modules::spot::TradingPairRule {
            pair_id: symbol,
            price_precision: price_precision as u32,
            quantity_precision: quantity_precision as u32,
            min_order_value,
            enabled: status == "active",
        })
    }

    /// 通过仓储接口写入现货订单实体；唯一请求标识冲突必须返回错误供上层幂等处理。
    /// 数据库失败由调用方回滚；涉及资金时余额、流水与业务状态必须同事务且幂等重放不重复入账。
    pub async fn insert_order_async(
        &self,
        new_order: NewOrder,
        idempotency_key: Option<&str>,
    ) -> Result<SpotOrder, crate::modules::spot::SpotServiceError> {
        let user_id = parse_spot_u64_identifier("user_id", &new_order.user_id)?;
        let pair_db_id = resolve_pair_id(&self.pool, &new_order.pair_id).await?;
        let result = sqlx::query(
            r#"INSERT INTO spot_orders
               (user_id, pair_id, side, order_type, price, trigger_price, quantity, filled_quantity, status, idempotency_key)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
               ON DUPLICATE KEY UPDATE id = LAST_INSERT_ID(id)"#,
        )
        .bind(user_id)
        .bind(pair_db_id)
        .bind(order_side_as_str(new_order.side))
        .bind(order_type_as_str(new_order.order_type))
        .bind(&new_order.price)
        .bind(&new_order.trigger_price)
        .bind(&new_order.quantity)
        .bind(&new_order.filled_quantity)
        .bind(order_status_as_str(new_order.status))
        .bind(idempotency_key)
        .execute(&self.pool)
        .await
        .map_err(map_spot_sqlx_error)?;
        self.load_order_async(&result.last_insert_id().to_string())
            .await
    }

    /// 从 MySQL 持久化数据读取仓储订单实体，保持现货既有归属过滤、可见性及排序条件。
    /// 通过仓储适配器读取现货订单实体，未知枚举或损坏金额返回错误而不伪造状态。
    pub async fn load_order_async(
        &self,
        order_id: &str,
    ) -> Result<SpotOrder, crate::modules::spot::SpotServiceError> {
        let row = sqlx::query_as::<
            _,
            (
                u64,
                u64,
                String,
                String,
                String,
                Option<BigDecimal>,
                Option<BigDecimal>,
                BigDecimal,
                BigDecimal,
                String,
            ),
        >(
            r#"SELECT orders.id, orders.user_id, pairs.symbol, orders.side, orders.order_type,
                      orders.price, orders.trigger_price, orders.quantity, orders.filled_quantity, orders.status
               FROM spot_orders orders
               INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
               WHERE orders.id = ?
               LIMIT 1"#,
        )
        .bind(parse_spot_u64_identifier("order_id", order_id)?)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_spot_sqlx_error)?
        .ok_or_else(|| {
            crate::modules::spot::SpotServiceError::Repository(format!("missing spot order: {order_id}"))
        })?;

        Ok(SpotOrder {
            id: row.0.to_string(),
            user_id: row.1.to_string(),
            pair_id: row.2,
            side: parse_order_side(&row.3),
            order_type: parse_order_type(&row.4),
            price: row.5,
            trigger_price: row.6,
            quantity: row.7,
            filled_quantity: row.8,
            status: parse_order_status(&row.9),
        })
    }

    /// 保存现货订单当前成交量、均价和状态，仓储更新失败不得伪造已完成状态。
    /// 数据库失败由调用方回滚；涉及资金时余额、流水与业务状态必须同事务且幂等重放不重复入账。
    pub async fn save_order_async(
        &self,
        order: SpotOrder,
    ) -> Result<(), crate::modules::spot::SpotServiceError> {
        let order_db_id = order.id.parse::<u64>().map_err(|_| {
            crate::modules::spot::SpotServiceError::Repository("invalid spot order id".to_string())
        })?;
        let pair_db_id = resolve_pair_id(&self.pool, &order.pair_id).await?;
        sqlx::query(
            r#"UPDATE spot_orders
               SET pair_id = ?, side = ?, order_type = ?, price = ?, trigger_price = ?, quantity = ?,
                   filled_quantity = ?, status = ?
               WHERE id = ?"#,
        )
        .bind(pair_db_id)
        .bind(order_side_as_str(order.side))
        .bind(order_type_as_str(order.order_type))
        .bind(order.price)
        .bind(order.trigger_price)
        .bind(order.quantity)
        .bind(order.filled_quantity)
        .bind(order_status_as_str(order.status))
        .bind(order_db_id)
        .execute(&self.pool)
        .await
        .map_err(map_spot_sqlx_error)?;
        Ok(())
    }

    /// 写入现货逐笔成交记录；幂等键确保同一撮合结果不会重复落库。
    /// 数据库失败由调用方回滚；涉及资金时余额、流水与业务状态必须同事务且幂等重放不重复入账。
    pub async fn insert_trade_async(
        &self,
        trade: NewSpotTrade,
    ) -> Result<SpotTrade, crate::modules::spot::SpotServiceError> {
        let pair_db_id = resolve_pair_id(&self.pool, &trade.pair_id).await?;
        let buy_order_id = parse_spot_u64_identifier("buy_order_id", &trade.buy_order_id)?;
        let sell_order_id = parse_spot_u64_identifier("sell_order_id", &trade.sell_order_id)?;
        let result = sqlx::query(
            r#"INSERT INTO spot_trades
               (pair_id, buy_order_id, sell_order_id, price, quantity, fee)
               VALUES (?, ?, ?, ?, ?, ?)"#,
        )
        .bind(pair_db_id)
        .bind(buy_order_id)
        .bind(sell_order_id)
        .bind(&trade.price)
        .bind(&trade.quantity)
        .bind(&trade.fee)
        .execute(&self.pool)
        .await
        .map_err(map_spot_sqlx_error)?;
        load_trade_by_id_async(&self.pool, result.last_insert_id()).await
    }

    /// 从 MySQL 持久化数据查询逐笔成交，保持现货既有归属过滤、可见性及排序条件。
    /// 按交易对读取逐笔成交并保持时间顺序；该仓储路径不修改订单。
    pub async fn list_trades_by_pair_async(
        &self,
        pair_id: &str,
        limit: u32,
    ) -> Result<Vec<SpotTrade>, crate::modules::spot::SpotServiceError> {
        let pair_db_id = resolve_pair_id(&self.pool, pair_id).await?;
        let rows = sqlx::query_as::<
            _,
            (
                u64,
                String,
                u64,
                u64,
                BigDecimal,
                BigDecimal,
                BigDecimal,
                chrono::DateTime<chrono::Utc>,
            ),
        >(
            r#"SELECT trades.id, pairs.symbol, trades.buy_order_id, trades.sell_order_id,
                      trades.price, trades.quantity, trades.fee, trades.created_at
               FROM spot_trades trades
               INNER JOIN trading_pairs pairs ON pairs.id = trades.pair_id
               WHERE trades.pair_id = ?
               ORDER BY trades.id DESC
               LIMIT ?"#,
        )
        .bind(pair_db_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(map_spot_sqlx_error)?;

        let mut trades = Vec::with_capacity(rows.len());
        for row in rows {
            let (id, pair_id, buy_order_id, sell_order_id, price, quantity, fee, created_at) = row;
            trades.push(SpotTrade {
                id: id.to_string(),
                pair_id,
                buy_order_id: buy_order_id.to_string(),
                sell_order_id: sell_order_id.to_string(),
                price,
                quantity,
                fee,
                created_at,
            });
        }
        Ok(trades)
    }
}

/// 从 MySQL 持久化数据查询现货订单，保持现货既有归属过滤、可见性及排序条件。
/// 按用户与筛选读取现货订单，用户条件始终由调用方显式传入。
pub(crate) async fn list_spot_orders(
    pool: &Pool<MySql>,
    filter: SpotOrderListFilter,
) -> AppResult<Vec<SpotOrderResponse>> {
    let mut builder = base_spot_orders_query(filter.include_internal);
    push_spot_order_list_filters(&mut builder, &filter);
    builder.push(SPOT_ORDER_ORDER_BY);
    builder.push(" LIMIT ");
    builder.push_bind(i64::from(filter.limit));

    let rows = builder
        .build_query_as::<SpotOrderQueryRow>()
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(SpotOrderResponse::from).collect())
}

/// 后台订单列表：行查询与 COUNT 共用同一组谓词，总数才会跟随当前筛选。
/// 后台订单行与总数使用同一筛选条件，分页失败不返回部分结果。
pub(crate) async fn list_admin_spot_orders_page(
    pool: &Pool<MySql>,
    filter: SpotOrderListFilter,
) -> AppResult<(Vec<SpotOrderResponse>, i64)> {
    let mut rows = base_spot_orders_query(filter.include_internal);
    let mut total = base_spot_orders_count_query();
    for builder in [&mut rows, &mut total] {
        push_spot_order_list_filters(builder, &filter);
    }

    let (rows, total) = fetch_admin_page::<SpotOrderQueryRow>(
        pool,
        rows,
        total,
        SPOT_ORDER_ORDER_BY,
        filter.limit,
        filter.offset,
    )
    .await?;
    Ok((
        rows.into_iter().map(SpotOrderResponse::from).collect(),
        total,
    ))
}

/// 从 MySQL 持久化数据读取现货订单，保持现货既有归属过滤、可见性及排序条件。
/// 按数据库主键读取订单详情，不存在返回 NotFound 且不锁钱包。
pub(crate) async fn load_spot_order_by_id(
    pool: &Pool<MySql>,
    order_id: u64,
) -> AppResult<SpotOrderResponse> {
    let mut builder = base_spot_orders_query(true);
    builder.push(" WHERE orders.id = ");
    builder.push_bind(order_id);
    builder
        .build_query_as::<SpotOrderQueryRow>()
        .fetch_optional(pool)
        .await?
        .map(SpotOrderResponse::from)
        .ok_or(AppError::NotFound)
}

/// 从 MySQL 持久化数据查询现货订单，保持现货既有归属过滤、可见性及排序条件。
/// 只返回本人 pending/open/partially_filled 订单主键，供批撤逐笔事务处理。
pub(crate) async fn list_user_cancellable_spot_order_ids(
    pool: &Pool<MySql>,
    user_id: u64,
    pair_id: Option<String>,
) -> AppResult<Vec<u64>> {
    let mut builder = QueryBuilder::<MySql>::new(
        r#"SELECT orders.id
           FROM spot_orders orders
           INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
           WHERE orders.user_id = "#,
    );
    builder.push_bind(user_id);
    builder.push(" AND orders.status IN ('pending', 'open', 'partially_filled')");
    if let Some(pair_id) = pair_id {
        let pair_db_id = pair_id.parse::<u64>().ok();
        builder.push(" AND (pairs.symbol = ");
        builder.push_bind(pair_id);
        builder.push(" OR pairs.id = ");
        builder.push_bind(pair_db_id);
        builder.push(")");
    }
    builder.push(" ORDER BY orders.id ASC");
    builder
        .build_query_scalar::<u64>()
        .fetch_all(pool)
        .await
        .map_err(AppError::from)
}

/// 从 MySQL 持久化数据查询逐笔成交，保持现货既有归属过滤、可见性及排序条件。
/// 按用户作为买方或卖方过滤成交，避免泄露无关账户交易。
pub(crate) async fn list_spot_trades(
    pool: &Pool<MySql>,
    filter: SpotTradeListFilter,
) -> AppResult<Vec<SpotTradeResponse>> {
    let mut builder = base_spot_trades_query();
    push_spot_trade_list_filters(&mut builder, &filter);
    builder.push(SPOT_TRADE_ORDER_BY);
    builder.push(" LIMIT ");
    builder.push_bind(i64::from(filter.limit));

    let rows = builder
        .build_query_as::<SpotTradeQueryRow>()
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(SpotTradeResponse::from).collect())
}

/// 后台成交列表：行查询与 COUNT 共用同一组谓词，总数才会跟随当前筛选。
/// 后台成交行与总数共享过滤条件；查询不重放成交或资金流水。
pub(crate) async fn list_admin_spot_trades_page(
    pool: &Pool<MySql>,
    filter: SpotTradeListFilter,
) -> AppResult<(Vec<SpotTradeResponse>, i64)> {
    let mut rows = base_spot_trades_query();
    let mut total = base_spot_trades_count_query();
    for builder in [&mut rows, &mut total] {
        push_spot_trade_list_filters(builder, &filter);
    }

    let (rows, total) = fetch_admin_page::<SpotTradeQueryRow>(
        pool,
        rows,
        total,
        SPOT_TRADE_ORDER_BY,
        filter.limit,
        filter.offset,
    )
    .await?;
    Ok((
        rows.into_iter().map(SpotTradeResponse::from).collect(),
        total,
    ))
}

/// 处理现货订单的现货基础设施适配逻辑，保持存储或外部协议的既有边界。
/// 构造统一现货订单读模型基础 SQL，用户和后台过滤由调用方参数化追加。
pub(super) fn base_spot_orders_query(
    include_internal_trades: bool,
) -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(format!(
        r#"SELECT orders.id, orders.user_id, users.email AS user_email, pairs.symbol AS pair_id, orders.side,
                  orders.order_type, orders.price, orders.trigger_price, orders.quantity, orders.filled_quantity,
                  orders.status, orders.created_at,
                  {} AS average_price
           FROM spot_orders orders
           INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
           LEFT JOIN users ON users.id = orders.user_id"#,
        spot_order_average_price_sql(include_internal_trades)
    ))
}

fn base_spot_orders_count_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT COUNT(*)
           FROM spot_orders orders
           INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
           LEFT JOIN users ON users.id = orders.user_id"#,
    )
}

fn base_spot_trades_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT trades.id, pairs.symbol AS pair_id, trades.buy_order_id, trades.sell_order_id,
                  trades.price, trades.quantity, trades.fee, trades.created_at
           FROM spot_trades trades
           INNER JOIN trading_pairs pairs ON pairs.id = trades.pair_id
           INNER JOIN spot_orders buy_orders ON buy_orders.id = trades.buy_order_id
           INNER JOIN spot_orders sell_orders ON sell_orders.id = trades.sell_order_id"#,
    )
}

fn base_spot_trades_count_query() -> QueryBuilder<'static, MySql> {
    QueryBuilder::<MySql>::new(
        r#"SELECT COUNT(*)
           FROM spot_trades trades
           INNER JOIN trading_pairs pairs ON pairs.id = trades.pair_id
           INNER JOIN spot_orders buy_orders ON buy_orders.id = trades.buy_order_id
           INNER JOIN spot_orders sell_orders ON sell_orders.id = trades.sell_order_id"#,
    )
}

fn spot_order_average_price_sql(include_internal_trades: bool) -> &'static str {
    if include_internal_trades {
        return r#"CAST((
             SELECT SUM(trades.price * trades.quantity) / NULLIF(SUM(trades.quantity), 0)
             FROM spot_trades trades
             WHERE trades.buy_order_id = orders.id OR trades.sell_order_id = orders.id
           ) AS DECIMAL(38,18))"#;
    }
    r#"CAST((
             SELECT SUM(trades.price * trades.quantity) / NULLIF(SUM(trades.quantity), 0)
             FROM spot_trades trades
             INNER JOIN spot_orders average_buy_orders ON average_buy_orders.id = trades.buy_order_id
             INNER JOIN users average_buy_users ON average_buy_users.id = average_buy_orders.user_id
             INNER JOIN spot_orders average_sell_orders ON average_sell_orders.id = trades.sell_order_id
             INNER JOIN users average_sell_users ON average_sell_users.id = average_sell_orders.user_id
             WHERE (trades.buy_order_id = orders.id OR trades.sell_order_id = orders.id)
               AND average_buy_users.email <> '__system_spot_liquidity@internal.local'
               AND average_sell_users.email <> '__system_spot_liquidity@internal.local'
           ) AS DECIMAL(38,18))"#
}

fn push_spot_order_list_filters(
    builder: &mut QueryBuilder<'_, MySql>,
    filter: &SpotOrderListFilter,
) {
    builder.push(" WHERE 1 = 1");
    if let Some(user_id) = filter.user_id {
        builder.push(" AND orders.user_id = ");
        builder.push_bind(user_id);
    }
    if let Some(pair_id) = filter.pair_id.clone() {
        builder.push(" AND pairs.symbol = ");
        builder.push_bind(pair_id);
    }
    if let Some(status) = filter.status.clone() {
        builder.push(" AND orders.status = ");
        builder.push_bind(status);
    }
    if let Some(email) = filter.email.clone() {
        builder.push(" AND users.email = ");
        builder.push_bind(email);
    }
    if !filter.include_internal {
        builder.push(" AND users.email <> ");
        builder.push_bind(SYSTEM_SPOT_LIQUIDITY_EMAIL);
    }
}

fn push_spot_trade_list_filters(
    builder: &mut QueryBuilder<'_, MySql>,
    filter: &SpotTradeListFilter,
) {
    builder.push(" WHERE 1 = 1");
    if let Some(pair_id) = filter.pair_id.clone() {
        builder.push(" AND pairs.symbol = ");
        builder.push_bind(pair_id);
    }
    if let Some(user_id) = filter.user_id {
        builder.push(" AND (buy_orders.user_id = ");
        builder.push_bind(user_id);
        builder.push(" OR sell_orders.user_id = ");
        builder.push_bind(user_id);
        builder.push(")");
    }
    if let Some(email) = filter.email.clone() {
        builder.push(
            r#" AND EXISTS (
                   SELECT 1 FROM users
                   WHERE users.email = "#,
        );
        builder.push_bind(email);
        builder.push(" AND (users.id = buy_orders.user_id OR users.id = sell_orders.user_id))");
    }
    if !filter.include_internal {
        builder.push(
            r#" AND NOT EXISTS (
                   SELECT 1 FROM users
                   WHERE users.email = "#,
        );
        builder.push_bind(SYSTEM_SPOT_LIQUIDITY_EMAIL);
        builder.push(" AND (users.id = buy_orders.user_id OR users.id = sell_orders.user_id))");
    }
}

impl From<SpotOrderQueryRow> for SpotOrderResponse {
    fn from(order: SpotOrderQueryRow) -> Self {
        Self {
            id: order.id.to_string(),
            user_id: order.user_id.to_string(),
            user_email: order.user_email,
            pair_id: order.pair_id,
            side: parse_order_side(&order.side),
            order_type: parse_order_type(&order.order_type),
            price: order.price,
            trigger_price: order.trigger_price,
            quantity: order.quantity,
            filled_quantity: order.filled_quantity,
            average_price: order.average_price,
            status: parse_order_status(&order.status),
            created_at: Some(order.created_at),
        }
    }
}

impl From<SpotTradeQueryRow> for SpotTradeResponse {
    fn from(row: SpotTradeQueryRow) -> Self {
        Self {
            id: row.id.to_string(),
            pair_id: row.pair_id,
            buy_order_id: row.buy_order_id.to_string(),
            sell_order_id: row.sell_order_id.to_string(),
            price: row.price,
            quantity: row.quantity,
            fee: row.fee,
            created_at: row.created_at,
        }
    }
}

impl From<SpotTradeQueryRow> for SpotTrade {
    fn from(row: SpotTradeQueryRow) -> Self {
        Self {
            id: row.id.to_string(),
            pair_id: row.pair_id,
            buy_order_id: row.buy_order_id.to_string(),
            sell_order_id: row.sell_order_id.to_string(),
            price: row.price,
            quantity: row.quantity,
            fee: row.fee,
            created_at: row.created_at,
        }
    }
}

async fn resolve_pair_id(
    pool: &Pool<MySql>,
    pair_id: &str,
) -> Result<u64, crate::modules::spot::SpotServiceError> {
    if let Ok(pair_db_id) = pair_id.parse::<u64>() {
        return Ok(pair_db_id);
    }

    sqlx::query_as::<_, (u64,)>(r#"SELECT id FROM trading_pairs WHERE symbol = ? LIMIT 1"#)
        .bind(pair_id)
        .fetch_optional(pool)
        .await
        .map_err(map_spot_sqlx_error)?
        .map(|(id,)| id)
        .ok_or_else(|| {
            crate::modules::spot::SpotServiceError::Repository(format!(
                "missing trading pair: {pair_id}"
            ))
        })
}

async fn load_trade_by_id_async(
    pool: &Pool<MySql>,
    trade_id: u64,
) -> Result<SpotTrade, crate::modules::spot::SpotServiceError> {
    let row = sqlx::query_as::<
        _,
        (
            u64,
            String,
            u64,
            u64,
            BigDecimal,
            BigDecimal,
            BigDecimal,
            chrono::DateTime<chrono::Utc>,
        ),
    >(
        r#"SELECT trades.id, pairs.symbol, trades.buy_order_id, trades.sell_order_id,
                      trades.price, trades.quantity, trades.fee, trades.created_at
               FROM spot_trades trades
               INNER JOIN trading_pairs pairs ON pairs.id = trades.pair_id
               WHERE trades.id = ?
               LIMIT 1"#,
    )
    .bind(trade_id)
    .fetch_optional(pool)
    .await
    .map_err(map_spot_sqlx_error)?
    .ok_or_else(|| {
        crate::modules::spot::SpotServiceError::Repository(format!(
            "missing spot trade: {trade_id}"
        ))
    })?;

    Ok(SpotTrade {
        id: row.0.to_string(),
        pair_id: row.1,
        buy_order_id: row.2.to_string(),
        sell_order_id: row.3.to_string(),
        price: row.4,
        quantity: row.5,
        fee: row.6,
        created_at: row.7,
    })
}
