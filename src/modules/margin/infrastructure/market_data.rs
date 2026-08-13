//! 杠杆定价所依赖的服务端行情缓存适配器。
//!
//! 开仓入场价、平仓标记价和风险快照价格全部从这里取，来源是行情接入链写入 Redis 的 ticker 缓存。
//! 三个入口共用同一套有效性判定：缓存必须存在、价格必须为正、观测时间不得早于当前六十秒。
//! 任何一项不满足都返回校验错误，绝不回退到客户端传入的价格，这是杠杆不接受用户报价的实现保证。
//! 本文件只读 Redis，不访问 MySQL、不加锁、不产生任何资金写入，失败时调用方须在动账之前中止。

use crate::{
    error::{AppError, AppResult},
    modules::market::market_ticker_redis_key,
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use redis::{AsyncCommands, aio::ConnectionManager};
use serde::Deserialize;

/// Redis ticker 缓存中本模块关心的两个字段，其余字段在反序列化时被忽略。
#[derive(Debug, Deserialize)]
struct CachedTickerPayload {
    /// 最新成交价，必须为正数才会被采纳。
    last_price: BigDecimal,
    /// 行情观测时间，以 Unix 毫秒存储，用于判定缓存是否已经陈旧。
    #[serde(with = "crate::time::unix_millis")]
    observed_at: DateTime<Utc>,
}

/// 服务端行情缓存中的保证金风险价格与观测时间。
pub(crate) struct MarginRiskTicker {
    /// 校验通过的最新价，直接作为风险快照的标记价参与浮盈计算。
    pub(crate) last_price: BigDecimal,
    /// 该价格的观测时间，随快照一起返回给客户端用于判断数据新鲜度。
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

/// 三个取价入口共用的行情有效性闸门，依次检查连接、缓存、价格符号和新鲜度四项。
/// Redis 未配置时直接报校验错误而不是当作缺失行情，两种情况文案相同但都不允许继续动账。
/// 价格必须严格大于零，避免撮合异常写入的零价被用作入场价或标记价而算出无穷大的仓位。
/// 观测时间早于当前六十秒即判为陈旧，宁可让开仓和平仓失败也不用过期价格结算资金。
/// `missing_message` 与 `label` 只用于区分调用来源的错误文案，不影响判定逻辑。
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

/// 按交易对符号拼出行情缓存键并读取原始 JSON，键格式与行情接入链写入端共用同一个构造函数。
/// 键不存在返回 None 表示缓存缺失，由上层转成校验错误；JSON 结构不合法则报内部错误，
/// 因为那意味着写入端与读取端的数据契约不一致，属于服务端缺陷而非用户输入问题。
/// 克隆连接管理器后使用，`ConnectionManager` 内部是共享句柄，这里不新建连接。
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
