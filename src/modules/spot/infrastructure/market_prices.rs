//! 现货行情价格缓存读取与可触发挂单 ID 查询。
//!
//! Redis `last_price` 是市价执行权威来源，必须为 60 秒内正数行情；客户端参考价不在此处兜底。
//! MySQL 触发查询只筛选候选订单，不开始事务、不锁钱包，最终价格条件仍由应用层在订单锁后复核。

use crate::{
    error::{AppError, AppResult},
    modules::market::market_ticker_redis_key,
};
use bigdecimal::BigDecimal;
use redis::{AsyncCommands, aio::ConnectionManager};
use serde_json::Value;
use sqlx::{MySql, Pool};

/// 读取行情接入链写入 Redis 的新鲜最新价，作为现货市价执行和触发判断的服务端权威价格。
/// 缺失、过期、非正或损坏载荷均返回错误，不得回退使用客户端参考价完成成交。
pub(crate) async fn latest_spot_market_price(
    redis: Option<&ConnectionManager>,
    pair_symbol: &str,
) -> AppResult<Option<BigDecimal>> {
    let Some(redis) = redis else {
        return Ok(None);
    };
    let mut connection = redis.clone();
    let payload: Option<String> = connection
        .get(market_ticker_redis_key(pair_symbol))
        .await
        .map_err(AppError::from)?;
    let Some(payload) = payload else {
        return Ok(None);
    };
    let value = serde_json::from_str::<Value>(&payload)
        .map_err(|error| AppError::Internal(format!("invalid cached ticker payload: {error}")))?;
    let last_price = value
        .get("last_price")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::Internal("cached ticker is missing last_price".to_owned()))?;
    let price = crate::numeric::parse_decimal_input(last_price)
        .map_err(|_| AppError::Internal("cached ticker last_price is invalid".to_owned()))?;
    if price <= 0 {
        return Err(AppError::Validation(
            "market price must be positive".to_owned(),
        ));
    }
    let observed_at = value
        .get("observed_at")
        .and_then(Value::as_i64)
        .ok_or_else(|| AppError::Internal("cached ticker is missing observed_at".to_owned()))?;
    let now = chrono::Utc::now().timestamp_millis();
    let age = now
        .checked_sub(observed_at)
        .ok_or_else(|| AppError::Validation("spot ticker time is out of range".to_owned()))?;
    if !(0..=60_000).contains(&age) {
        return Err(AppError::Validation("spot ticker is stale".to_owned()));
    }
    Ok(Some(price))
}

/// 处理已触发限价买单标识的现货基础设施适配逻辑，保持存储或外部协议的既有边界。
/// 按服务端行情筛选可触发限价买单主键并稳定排序，查询本身不锁钱包。
pub(crate) async fn triggered_limit_buy_order_ids(
    pool: &Pool<MySql>,
    pair_symbol: &str,
    market_price: &BigDecimal,
    limit: u32,
) -> AppResult<Vec<u64>> {
    let rows = sqlx::query_as::<_, (u64,)>(
        r#"SELECT orders.id
           FROM spot_orders orders
           INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
           WHERE REPLACE(REPLACE(REPLACE(UPPER(pairs.symbol), '-', ''), '/', ''), '_', '') =
                 REPLACE(REPLACE(REPLACE(UPPER(?), '-', ''), '/', ''), '_', '')
             AND orders.side = 'buy'
             AND orders.order_type = 'limit'
             AND orders.status IN ('pending', 'open', 'partially_filled')
             AND orders.price >= ?
           ORDER BY orders.price DESC, orders.id ASC
           LIMIT ?"#,
    )
    .bind(pair_symbol)
    .bind(market_price)
    .bind(i64::from(limit))
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|row| row.0).collect())
}

/// 处理已触发限价卖单标识的现货基础设施适配逻辑，保持存储或外部协议的既有边界。
/// 按服务端行情筛选可触发限价卖单主键并稳定排序，查询本身不锁钱包。
pub(crate) async fn triggered_limit_sell_order_ids(
    pool: &Pool<MySql>,
    pair_symbol: &str,
    market_price: &BigDecimal,
    limit: u32,
) -> AppResult<Vec<u64>> {
    let rows = sqlx::query_as::<_, (u64,)>(
        r#"SELECT orders.id
           FROM spot_orders orders
           INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
           WHERE REPLACE(REPLACE(REPLACE(UPPER(pairs.symbol), '-', ''), '/', ''), '_', '') =
                 REPLACE(REPLACE(REPLACE(UPPER(?), '-', ''), '/', ''), '_', '')
             AND orders.side = 'sell'
             AND orders.order_type = 'limit'
             AND orders.status IN ('pending', 'open', 'partially_filled')
             AND orders.price <= ?
           ORDER BY orders.price ASC, orders.id ASC
           LIMIT ?"#,
    )
    .bind(pair_symbol)
    .bind(market_price)
    .bind(i64::from(limit))
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|row| row.0).collect())
}

/// 处理已触发限价买单标识的现货基础设施适配逻辑，保持存储或外部协议的既有边界。
/// 旧买单保留双重 <=；显式单阈值命中即入候选，已激活单仅检查限价，执行事务再次复核。
pub(crate) async fn triggered_stop_limit_buy_order_ids(
    pool: &Pool<MySql>,
    pair_symbol: &str,
    market_price: &BigDecimal,
    limit: u32,
) -> AppResult<Vec<u64>> {
    let rows = sqlx::query_as::<_, (u64,)>(
        r#"SELECT orders.id
           FROM spot_orders orders
           INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
           WHERE REPLACE(REPLACE(REPLACE(UPPER(pairs.symbol), '-', ''), '/', ''), '_', '') =
                 REPLACE(REPLACE(REPLACE(UPPER(?), '-', ''), '/', ''), '_', '')
             AND orders.side = 'buy'
             AND orders.order_type = 'stop_limit'
             AND orders.status IN ('pending', 'open', 'partially_filled')
             AND (
                 (orders.trigger_direction IS NULL AND orders.trigger_price >= ? AND orders.price >= ?)
                 OR (orders.trigger_direction IS NOT NULL AND (
                     (orders.triggered_at IS NOT NULL AND orders.price >= ?)
                     OR (orders.triggered_at IS NULL AND (
                         (orders.trigger_direction = 'rising' AND orders.trigger_price <= ?)
                         OR (orders.trigger_direction = 'falling' AND orders.trigger_price >= ?)
                     ))
                 ))
             )
           ORDER BY orders.trigger_price DESC, orders.price DESC, orders.id ASC
           LIMIT ?"#,
    )
    .bind(pair_symbol)
    .bind(market_price)
    .bind(market_price)
    .bind(market_price)
    .bind(market_price)
    .bind(market_price)
    .bind(i64::from(limit))
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|row| row.0).collect())
}

/// 处理已触发限价卖单标识的现货基础设施适配逻辑，保持存储或外部协议的既有边界。
/// 旧卖单保留双重 >=；显式单阈值命中即入候选，已激活单仅检查限价，执行事务再次复核。
pub(crate) async fn triggered_stop_limit_sell_order_ids(
    pool: &Pool<MySql>,
    pair_symbol: &str,
    market_price: &BigDecimal,
    limit: u32,
) -> AppResult<Vec<u64>> {
    let rows = sqlx::query_as::<_, (u64,)>(
        r#"SELECT orders.id
           FROM spot_orders orders
           INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
           WHERE REPLACE(REPLACE(REPLACE(UPPER(pairs.symbol), '-', ''), '/', ''), '_', '') =
                 REPLACE(REPLACE(REPLACE(UPPER(?), '-', ''), '/', ''), '_', '')
             AND orders.side = 'sell'
             AND orders.order_type = 'stop_limit'
             AND orders.status IN ('pending', 'open', 'partially_filled')
             AND (
                 (orders.trigger_direction IS NULL AND orders.trigger_price <= ? AND orders.price <= ?)
                 OR (orders.trigger_direction IS NOT NULL AND (
                     (orders.triggered_at IS NOT NULL AND orders.price <= ?)
                     OR (orders.triggered_at IS NULL AND (
                         (orders.trigger_direction = 'rising' AND orders.trigger_price <= ?)
                         OR (orders.trigger_direction = 'falling' AND orders.trigger_price >= ?)
                     ))
                 ))
             )
           ORDER BY orders.trigger_price ASC, orders.price ASC, orders.id ASC
           LIMIT ?"#,
    )
    .bind(pair_symbol)
    .bind(market_price)
    .bind(market_price)
    .bind(market_price)
    .bind(market_price)
    .bind(market_price)
    .bind(i64::from(limit))
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|row| row.0).collect())
}
