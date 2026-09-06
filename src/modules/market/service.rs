//! market bounded context service layer.
//!
//! 服务层：封装可复用业务服务和跨实体业务规则。
//! 当前文件只承载与存储无关的行情读模型规则：交易对形态归一、公开查询条数收敛，
//! 以及未配置 MySQL 时的兜底交易对目录。这些结果只用于公开展示与入口预检，
//! 不能替代数据库交易对配置参与下单、结算或任何资金判定。

use crate::{
    error::{AppError, AppResult},
    modules::market::{ValidatedMarketSymbol, presentation::MarketResponse},
};

/// 裁剪并校验交易对，只允许 ASCII 字母数字及 `/`、`-`、`_`，返回去分隔符的大写值。
/// 规范化后为空或超过 32 字符都视为非法，统一转成 `AppError::Validation` 而非内部错误。
/// 本函数只做形态归一，不查询后台交易对配置，因此通过校验不代表该交易对已上架。
pub(crate) fn validate_market_symbol(raw: &str) -> AppResult<ValidatedMarketSymbol> {
    ValidatedMarketSymbol::from_raw(raw).map_err(|error| AppError::Validation(error.to_string()))
}

/// 将公开成交查询条数默认设为 50，并限制在 1～100。
/// 夹紧而非报错，因此传入 0 或超大值只会收敛到边界，调用方无需自行防御非法分页参数。
/// 该上限用于避免单次查询扫描过多成交记录，函数不参与排序口径与交易对上架校验。
pub(crate) fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 100)
}

/// 返回无数据库部署使用的稳定市场列表；仅为公开读模型兜底，不参与交易执行价格。
/// 固定给出外部源 BTCUSDT 与策略源 NEWUSDT 两条，字段是精度 8、最小下单额 1 的占位元数据。
/// 这些值不来自 `trading_pairs`，一旦配置了 MySQL 就必须改读真实交易对，禁止用于下单与结算校验。
pub(crate) fn fallback_markets() -> Vec<MarketResponse> {
    vec![
        MarketResponse::fallback("BTCUSDT", "BTC", "USDT", "external"),
        MarketResponse::fallback("NEWUSDT", "NEW", "USDT", "strategy"),
    ]
}

/// 判断交易对是否属于公开兜底列表；该结果不得替代数据库交易对规则完成资金操作。
/// 入参必须是已规范化的大写无分隔符值，比较为精确匹配，只承认 BTCUSDT 与 NEWUSDT。
/// 仅在 MySQL 缺席时作为公开行情入口的上架预检，命中与否都不反映真实上下架状态。
pub(crate) fn fallback_market_symbol_is_listed(symbol: &str) -> bool {
    matches!(symbol, "BTCUSDT" | "NEWUSDT")
}

/// 把 Redis 中当前 UTC 高周期的形成中快照合并到升序历史；过期槽、错周期、越界时间一律忽略。
/// 只影响本次读模型，不写 Mongo；同槽已有正式历史优先保留，结果按时间排序并保留最新 limit 根。
pub(crate) fn merge_current_kline(
    rows: &mut Vec<super::presentation::KlineResponse>,
    current: super::presentation::KlineResponse,
    query: &super::KlineQuery,
    now: chrono::DateTime<chrono::Utc>,
) {
    let minutes = match query.interval.as_str() {
        "5m" => 5,
        "15m" => 15,
        "1h" => 60,
        "4h" => 240,
        "1d" => 1440,
        _ => return,
    };
    if current.interval != query.interval
        || current.open_time.timestamp().rem_euclid(minutes * 60) != 0
        || current.open_time > now
        || current.open_time + chrono::TimeDelta::minutes(minutes) <= now
        || query.start.is_some_and(|start| current.open_time < start)
        || query.end.is_some_and(|end| current.open_time > end)
        || rows.iter().any(|row| row.open_time == current.open_time)
    {
        return;
    }
    rows.push(current);
    rows.sort_by_key(|row| row.open_time);
    let excess = rows.len().saturating_sub(query.limit as usize);
    rows.drain(..excess);
}
