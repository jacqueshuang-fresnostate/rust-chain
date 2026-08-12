use crate::{
    error::{AppError, AppResult},
    modules::market::market_ticker_redis_key,
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use redis::{AsyncCommands, aio::ConnectionManager};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct CachedTickerPayload {
    last_price: BigDecimal,
    #[serde(with = "crate::time::unix_millis")]
    observed_at: DateTime<Utc>,
}

/// 服务端行情缓存中的保证金风险价格与观测时间。
pub(crate) struct MarginRiskTicker {
    pub(crate) last_price: BigDecimal,
    pub(crate) observed_at: DateTime<Utc>,
}
/// 读取行情接入链写入 Redis 的新鲜正价格，作为主动平仓的服务端权威标记价。
/// 缓存缺失、超过六十秒或价格非法即失败，且不会回退到客户端价格或修改资金。
pub(crate) async fn cached_margin_mark_price(
    redis: Option<&ConnectionManager>,
    pair_id: u64,
    symbol: &str,
) -> AppResult<BigDecimal> {
    let ticker = cached_valid_margin_ticker(
        redis,
        pair_id,
        symbol,
        "cached ticker is required to close margin position",
        "margin close ticker",
    )
    .await?;
    Ok(ticker.last_price)
}

/// 读取服务端行情缓存中的价格与观测时间，供保证金风险快照使用。
/// 缺失、陈旧或非法行情返回校验错误；该只读入口不锁仓位或触发强平。
pub(crate) async fn cached_margin_risk_ticker(
    redis: Option<&ConnectionManager>,
    pair_id: u64,
    symbol: &str,
) -> AppResult<MarginRiskTicker> {
    let ticker = cached_valid_margin_ticker(
        redis,
        pair_id,
        symbol,
        "cached ticker is required for margin risk snapshot",
        "margin risk ticker",
    )
    .await?;
    Ok(MarginRiskTicker {
        last_price: ticker.last_price,
        observed_at: ticker.observed_at,
    })
}

/// 读取行情接入链写入 Redis 的新鲜正价格，作为保证金开仓的服务端权威入场价。
/// 行情缺失、陈旧或非法时必须在仓位写入和抵押扣款前失败。
pub(crate) async fn cached_margin_entry_price(
    redis: Option<&ConnectionManager>,
    pair_id: u64,
    symbol: &str,
) -> AppResult<BigDecimal> {
    let ticker = cached_valid_margin_ticker(
        redis,
        pair_id,
        symbol,
        "fresh cached ticker is required to open margin position",
        "margin entry ticker",
    )
    .await?;
    Ok(ticker.last_price)
}

async fn cached_valid_margin_ticker(
    redis: Option<&ConnectionManager>,
    pair_id: u64,
    symbol: &str,
    missing_message: &str,
    label: &str,
) -> AppResult<CachedTickerPayload> {
    let Some(redis) = redis else {
        return Err(AppError::Validation(format!(
            "{missing_message} for pair {pair_id}"
        )));
    };
    let ticker = cached_ticker_price(redis, symbol)
        .await?
        .ok_or_else(|| AppError::Validation(format!("{missing_message} for pair {pair_id}")))?;
    if ticker.last_price <= 0 {
        return Err(AppError::Validation(format!(
            "{label} price must be positive for pair {pair_id}"
        )));
    }
    if ticker.observed_at < Utc::now() - chrono::TimeDelta::seconds(60) {
        return Err(AppError::Validation(format!(
            "{label} is stale for pair {pair_id}"
        )));
    }
    Ok(ticker)
}

async fn cached_ticker_price(
    redis: &ConnectionManager,
    symbol: &str,
) -> AppResult<Option<CachedTickerPayload>> {
    let mut connection = redis.clone();
    let payload: Option<String> = connection.get(market_ticker_redis_key(symbol)).await?;
    payload
        .map(|payload| {
            serde_json::from_str::<CachedTickerPayload>(&payload).map_err(|error| {
                AppError::Internal(format!("invalid cached margin ticker payload: {error}"))
            })
        })
        .transpose()
}
