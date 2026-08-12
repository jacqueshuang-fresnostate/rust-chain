//! 后台行情源配置、凭证与运行状态 DTO。

use super::*;

#[derive(Debug, Deserialize)]
pub struct SaveMarketFeedConfigRequest {
    pub symbols: Vec<String>,
    pub intervals: Vec<String>,
    pub providers: Vec<String>,
    pub enabled: bool,
    pub reason: Option<String>,
}

impl PresentationLayer for SaveMarketFeedConfigRequest {}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct MarketFeedConfigResponse {
    pub id: u64,
    pub name: String,
    pub symbols: Vec<String>,
    pub intervals: Vec<String>,
    pub providers: Vec<String>,
    pub enabled: bool,
    pub version: u64,
    pub applied_version: Option<u64>,
    pub needs_reload: bool,
    pub last_reload_status: Option<String>,
    pub last_reload_error: Option<String>,
    #[serde(default, with = "option_unix_millis")]
    pub last_reloaded_at: Option<DateTime<Utc>>,
}

impl PresentationLayer for MarketFeedConfigResponse {}

#[derive(Debug, Serialize)]
pub struct MarketFeedStatusResponse {
    pub saved_config: Option<MarketFeedConfigResponse>,
    pub runtime: MarketFeedRuntimeStatus,
}

impl PresentationLayer for MarketFeedStatusResponse {}

#[derive(Debug, Deserialize)]
pub struct ReloadMarketFeedRequest {
    pub reason: String,
}

impl PresentationLayer for ReloadMarketFeedRequest {}

#[derive(Debug, Serialize)]
pub struct ReloadMarketFeedResponse {
    pub config: MarketFeedConfigResponse,
    pub runtime: MarketFeedRuntimeStatus,
}

impl PresentationLayer for ReloadMarketFeedResponse {}

#[derive(Debug, Deserialize)]
pub struct UpsertMarketSourceCredentialRequest {
    pub auth_type: String,
    pub api_key: Option<String>,
    pub api_secret: Option<String>,
    pub passphrase: Option<String>,
    pub enabled: bool,
    pub reason: String,
}

impl PresentationLayer for UpsertMarketSourceCredentialRequest {}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct MarketSourceCredentialResponse {
    pub provider: String,
    pub auth_type: String,
    pub api_key_mask: Option<String>,
    pub enabled: bool,
}

impl PresentationLayer for MarketSourceCredentialResponse {}

#[derive(Debug, Serialize)]
pub struct MarketSourceCredentialsResponse {
    pub credentials: Vec<MarketSourceCredentialResponse>,
}

impl PresentationLayer for MarketSourceCredentialsResponse {}

#[derive(Debug, Clone)]
pub struct MarketSourceCredentialSecret {
    pub provider: String,
    pub auth_type: String,
    pub api_key: Option<String>,
    pub api_secret: Option<String>,
    pub passphrase: Option<String>,
}
