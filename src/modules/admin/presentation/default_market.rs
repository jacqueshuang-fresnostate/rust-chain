//! 常驻默认行情的版本化配置、运行状态及无副作用预览 DTO。

use super::*;
use crate::modules::market::synthetic_default::DefaultMarketParameters;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SaveDefaultMarketRequest {
    pub(crate) expected_version: u32,
    pub(crate) enabled: bool,
    pub(crate) initial_price: Option<BigDecimal>,
    pub(crate) config: DefaultMarketParameters,
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PauseDefaultMarketRequest {
    pub(crate) expected_version: u32,
    pub(crate) all_market_paused: bool,
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PreviewDefaultMarketRequest {
    pub(crate) expected_version: u32,
    pub(crate) initial_price: Option<BigDecimal>,
    pub(crate) config: DefaultMarketParameters,
}

#[derive(Debug, Serialize)]
pub(crate) struct DefaultMarketResponse {
    pub(crate) pair_id: u64,
    pub(crate) symbol: String,
    pub(crate) market_type: String,
    pub(crate) configured: bool,
    pub(crate) version: u32,
    pub(crate) enabled: bool,
    pub(crate) all_market_paused: bool,
    pub(crate) initial_price: Option<BigDecimal>,
    pub(crate) config: DefaultMarketParameters,
    pub(crate) seed: String,
    #[serde(with = "option_unix_millis")]
    pub(crate) updated_at: Option<DateTime<Utc>>,
    pub(crate) runtime: DefaultMarketRuntimeResponse,
    pub(crate) reference_pair: Option<DefaultMarketReferencePairResponse>,
}

#[derive(Debug, Default, Serialize, sqlx::FromRow)]
pub(crate) struct DefaultMarketRuntimeResponse {
    pub(crate) generation: u64,
    pub(crate) active_source: String,
    pub(crate) strategy_id: Option<u64>,
    pub(crate) strategy_version: Option<i32>,
    pub(crate) default_version: Option<u32>,
    pub(crate) last_price: Option<BigDecimal>,
    #[serde(with = "option_unix_millis")]
    pub(crate) last_tick_at: Option<DateTime<Utc>>,
    pub(crate) error_message: Option<String>,
    #[sqlx(skip)]
    pub(crate) follow: Option<DefaultMarketFollowStatusResponse>,
}

#[derive(Debug, Serialize)]
pub(crate) struct DefaultMarketPreviewResponse {
    pub(crate) pair_id: u64,
    pub(crate) version: u32,
    pub(crate) seed: String,
    pub(crate) start_price: BigDecimal,
    pub(crate) samples: Vec<MarketStrategyRecoverySampleResponse>,
    pub(crate) follow_preview: Option<DefaultMarketFollowPreviewResponse>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub(crate) struct DefaultMarketReferencePairResponse {
    pub(crate) id: u64,
    pub(crate) symbol: String,
    pub(crate) status: String,
    pub(crate) market_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DefaultMarketFollowStatusResponse {
    pub(crate) mode: DefaultMarketEffectiveFollowMode,
    pub(crate) reference_pair_id: u64,
    pub(crate) reference_symbol: String,
    pub(crate) reference_price: Option<BigDecimal>,
    #[serde(with = "option_unix_millis")]
    pub(crate) reference_observed_at: Option<DateTime<Utc>>,
    pub(crate) fallback_reason: Option<String>,
    #[serde(with = "unix_millis")]
    pub(crate) switched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DefaultMarketEffectiveFollowMode {
    Following,
    Fallback,
}

#[derive(Debug, Serialize)]
pub(crate) struct DefaultMarketFollowPreviewResponse {
    pub(crate) kind: &'static str,
    pub(crate) reference_pair_id: u64,
    pub(crate) reference_symbol: String,
    #[serde(with = "unix_millis")]
    pub(crate) range_start: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) range_end: DateTime<Utc>,
    pub(crate) reference_sample_count: usize,
    pub(crate) warning: Option<String>,
}
