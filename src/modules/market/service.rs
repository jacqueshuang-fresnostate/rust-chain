//! market bounded context service layer.
//!
//! 服务层：封装可复用业务服务和跨实体业务规则。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的业务服务逐步迁入。

use crate::{
    error::{AppError, AppResult},
    modules::market::{ValidatedMarketSymbol, presentation::MarketResponse},
};

/// 裁剪并校验交易对，只允许 ASCII 字母数字及 `/`、`-`、`_`，返回去分隔符的大写值。
pub(crate) fn validate_market_symbol(raw: &str) -> AppResult<ValidatedMarketSymbol> {
    ValidatedMarketSymbol::from_raw(raw).map_err(|error| AppError::Validation(error.to_string()))
}

/// 将公开成交查询条数默认设为 50，并限制在 1～100。
pub(crate) fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 100)
}

/// 返回无数据库部署使用的稳定市场列表；仅为公开读模型兜底，不参与交易执行价格。
pub(crate) fn fallback_markets() -> Vec<MarketResponse> {
    vec![
        MarketResponse::fallback("BTCUSDT", "BTC", "USDT", "external"),
        MarketResponse::fallback("NEWUSDT", "NEW", "USDT", "strategy"),
    ]
}

/// 判断交易对是否属于公开兜底列表；该结果不得替代数据库交易对规则完成资金操作。
pub(crate) fn fallback_market_symbol_is_listed(symbol: &str) -> bool {
    matches!(symbol, "BTCUSDT" | "NEWUSDT")
}
