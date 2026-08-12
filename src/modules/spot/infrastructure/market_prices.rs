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
use std::str::FromStr;

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
    let price = BigDecimal::from_str(last_price)
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
    let stale_before = chrono::Utc::now().timestamp_millis() - 60_000;
    if observed_at < stale_before {
        return Err(AppError::Validation("spot ticker is stale".to_owned()));
    }
    Ok(Some(price))
}

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
             AND orders.trigger_price >= ?
             AND orders.price >= ?
           ORDER BY orders.trigger_price DESC, orders.price DESC, orders.id ASC
           LIMIT ?"#,
    )
    .bind(pair_symbol)
    .bind(market_price)
    .bind(market_price)
    .bind(i64::from(limit))
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|row| row.0).collect())
}

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
             AND orders.trigger_price <= ?
             AND orders.price <= ?
           ORDER BY orders.trigger_price ASC, orders.price ASC, orders.id ASC
           LIMIT ?"#,
    )
    .bind(pair_symbol)
    .bind(market_price)
    .bind(market_price)
    .bind(i64::from(limit))
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|row| row.0).collect())
}
