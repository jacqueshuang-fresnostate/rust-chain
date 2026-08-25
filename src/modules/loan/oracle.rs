//! 借贷风控的服务端权威行情适配器。
//!
//! 抵押贷申请、放款审批、健康度查询与强制清算全部经由本模块读取行情接入链写入 Redis 的 ticker。
//! 价格必须为正、符号与配置完全一致、观测时间不在未来且未超过产品快照的最大年龄。
//! 任何缓存缺失、连接缺失或陈旧价都失败关闭，不回退客户价、产品发行价或历史快照。

use crate::{
    error::{AppError, AppResult},
    modules::{
        loan::domain::LOAN_ORACLE_SOURCE_MARKET_TICKER_REDIS,
        market::{ValidatedMarketSymbol, market_ticker_redis_key},
        wallet::{MAX_ASSET_PRECISION_SCALE, truncate_amount_to_asset_precision},
    },
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, TimeDelta, Utc};
use redis::{AsyncCommands, aio::ConnectionManager};
use serde::Deserialize;

/// 允许行情写入端与 API 节点时钟最多相差五秒，超过即视为时间契约破坏。
const MAX_FUTURE_SKEW_SECONDS: i64 = 5;
const MAX_ORACLE_AGE_SECONDS: u64 = 86_400;

/// Redis ticker JSON 中借贷风控依赖的最小字段集。
#[derive(Debug, Deserialize)]
struct CachedLoanTickerPayload {
    symbol: String,
    last_price: BigDecimal,
    #[serde(with = "crate::time::unix_millis")]
    observed_at: DateTime<Utc>,
}

/// 通过符号、来源、正数与新鲜度校验的借贷权威价格。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoanOraclePrice {
    pub symbol: String,
    pub source: String,
    pub price: BigDecimal,
    pub observed_at: DateTime<Utc>,
}

/// 读取并校验一份抵押贷 ticker，是所有资金与风险用例的唯一取价入口。
pub async fn load_fresh_loan_oracle_price(
    redis: Option<&ConnectionManager>,
    source: &str,
    symbol: &str,
    max_age_seconds: u64,
    now: DateTime<Utc>,
) -> AppResult<LoanOraclePrice> {
    if source != LOAN_ORACLE_SOURCE_MARKET_TICKER_REDIS {
        return Err(AppError::Validation(
            "unsupported loan oracle_source".to_owned(),
        ));
    }
    if max_age_seconds == 0 || max_age_seconds > MAX_ORACLE_AGE_SECONDS {
        return Err(AppError::Validation(
            "oracle_max_age_seconds must be between 1 and 86400".to_owned(),
        ));
    }
    let normalized_symbol = ValidatedMarketSymbol::from_raw(symbol)
        .map_err(|_| AppError::Validation("invalid loan oracle_symbol".to_owned()))?
        .as_str()
        .to_owned();
    let redis = redis.ok_or_else(|| {
        AppError::Validation("loan oracle Redis connection is required".to_owned())
    })?;
    let mut connection = redis.clone();
    let payload: Option<String> = connection
        .get(market_ticker_redis_key(&normalized_symbol))
        .await?;
    let payload = payload.ok_or_else(|| {
        AppError::Validation(format!(
            "loan oracle ticker is missing for {normalized_symbol}"
        ))
    })?;
    // I/O 等待期间时钟仍在前进；使用调用方逻辑时钟与实际完成时刻中较晚者，避免慢 Redis 放过过期价。
    let validation_now = now.max(Utc::now());
    validate_loan_ticker_payload(
        &payload,
        &normalized_symbol,
        source,
        max_age_seconds,
        validation_now,
    )
}

/// 对缓存 JSON 做不依赖 I/O 的完整契约校验，便于精确覆盖陈旧与未来行情回归。
pub(crate) fn validate_loan_ticker_payload(
    payload: &str,
    expected_symbol: &str,
    source: &str,
    max_age_seconds: u64,
    now: DateTime<Utc>,
) -> AppResult<LoanOraclePrice> {
    let ticker = serde_json::from_str::<CachedLoanTickerPayload>(payload).map_err(|error| {
        AppError::Internal(format!("invalid cached loan ticker payload: {error}"))
    })?;
    let payload_symbol = ValidatedMarketSymbol::from_raw(&ticker.symbol)
        .map_err(|_| AppError::Internal("cached loan ticker symbol is invalid".to_owned()))?
        .as_str()
        .to_owned();
    if payload_symbol != expected_symbol {
        return Err(AppError::Validation(
            "loan oracle ticker symbol does not match product snapshot".to_owned(),
        ));
    }
    if ticker.last_price <= 0 {
        return Err(AppError::Validation(
            "loan oracle price must be positive".to_owned(),
        ));
    }
    ensure_loan_oracle_observation_fresh(ticker.observed_at, max_age_seconds, now)?;
    // 风控计算和 DECIMAL(38,18) 快照必须使用同一个可持久化价格；向零截断对抵押估值保持保守。
    let price = truncate_amount_to_asset_precision(&ticker.last_price, MAX_ASSET_PRECISION_SCALE);
    if price <= 0 {
        return Err(AppError::Validation(
            "loan oracle price is below the supported precision".to_owned(),
        ));
    }
    Ok(LoanOraclePrice {
        symbol: expected_symbol.to_owned(),
        source: source.to_owned(),
        price,
        observed_at: ticker.observed_at,
    })
}

/// 在任何可能等待数据库行锁的资金动作之后重新校验行情年龄，避免旧价格在事务尾部继续生效。
pub(crate) fn ensure_loan_oracle_observation_fresh(
    observed_at: DateTime<Utc>,
    max_age_seconds: u64,
    now: DateTime<Utc>,
) -> AppResult<()> {
    if max_age_seconds == 0 || max_age_seconds > MAX_ORACLE_AGE_SECONDS {
        return Err(AppError::Validation(
            "oracle_max_age_seconds must be between 1 and 86400".to_owned(),
        ));
    }
    if observed_at > now + TimeDelta::seconds(MAX_FUTURE_SKEW_SECONDS) {
        return Err(AppError::Validation(
            "loan oracle ticker observed_at is in the future".to_owned(),
        ));
    }
    let max_age = i64::try_from(max_age_seconds)
        .map_err(|_| AppError::Validation("oracle_max_age_seconds is too large".to_owned()))?;
    if observed_at < now - TimeDelta::seconds(max_age) {
        return Err(AppError::Validation(
            "loan oracle ticker is stale".to_owned(),
        ));
    }
    Ok(())
}
