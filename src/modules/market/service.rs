//! market bounded context service layer.
//!
//! 服务层：封装可复用业务服务和跨实体业务规则。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的业务服务逐步迁入。

use crate::{
    architecture::ServiceLayer,
    error::{AppError, AppResult},
    modules::market::{ValidatedMarketSymbol, presentation::MarketResponse},
};

#[derive(Debug)]
pub struct ServiceLayerMarker;

impl ServiceLayer for ServiceLayerMarker {}

pub(crate) fn validate_market_symbol(raw: &str) -> AppResult<ValidatedMarketSymbol> {
    ValidatedMarketSymbol::from_raw(raw).map_err(|error| AppError::Validation(error.to_string()))
}

pub(crate) fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 100)
}

pub(crate) fn fallback_markets() -> Vec<MarketResponse> {
    vec![
        MarketResponse::fallback("BTCUSDT", "BTC", "USDT", "external"),
        MarketResponse::fallback("NEWUSDT", "NEW", "USDT", "strategy"),
    ]
}

pub(crate) fn fallback_market_symbol_is_listed(symbol: &str) -> bool {
    matches!(symbol, "BTCUSDT" | "NEWUSDT")
}
