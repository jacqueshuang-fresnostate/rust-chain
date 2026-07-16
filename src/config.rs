use config::{Config, ConfigError, Environment};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    #[serde(default = "default_app_env")]
    pub app_env: String,
    #[serde(default = "default_host")]
    pub app_host: IpAddr,
    #[serde(default = "default_port")]
    pub app_port: u16,
    pub database_url: SecretString,
    pub mongodb_uri: SecretString,
    pub mongodb_database: String,
    pub redis_url: SecretString,
    pub rabbitmq_url: SecretString,
    pub jwt_secret: SecretString,
    #[serde(default)]
    pub credential_encryption_key: Option<SecretString>,
    #[serde(default = "default_access_ttl")]
    pub jwt_access_ttl_seconds: u64,
    #[serde(default = "default_refresh_ttl")]
    pub jwt_refresh_ttl_seconds: u64,
    pub bitget_rest_base_url: String,
    pub bitget_ws_url: String,
    pub htx_rest_base_url: String,
    pub htx_ws_url: String,
    #[serde(default = "default_coinbase_rest_base_url")]
    pub coinbase_rest_base_url: String,
    #[serde(default = "default_coinbase_ws_url")]
    pub coinbase_ws_url: String,
    #[serde(default, deserialize_with = "deserialize_env_vec")]
    pub market_feed_symbols: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_env_vec")]
    pub market_feed_intervals: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_env_vec")]
    pub market_feed_providers: Vec<String>,
    #[serde(default = "default_market_feed_reconnect_seconds")]
    pub market_feed_reconnect_seconds: u64,
    #[serde(default = "default_market_feed_rest_fallback_timeout_seconds")]
    pub market_feed_rest_fallback_timeout_seconds: u64,
    #[serde(default = "default_event_inbox_retry_scan_seconds")]
    pub event_inbox_retry_scan_seconds: u64,
    #[serde(default = "default_event_outbox_publisher_enabled")]
    pub event_outbox_publisher_enabled: bool,
    #[serde(default = "default_event_outbox_publisher_interval_seconds")]
    pub event_outbox_publisher_interval_seconds: u64,
    #[serde(default = "default_unlock_scanner_enabled")]
    pub unlock_scanner_enabled: bool,
    #[serde(default = "default_unlock_scanner_interval_seconds")]
    pub unlock_scanner_interval_seconds: u64,
    #[serde(default = "default_unlock_scanner_batch_limit")]
    pub unlock_scanner_batch_limit: u32,
    #[serde(default = "default_kline_recovery_enabled")]
    pub kline_recovery_enabled: bool,
    #[serde(default = "default_kline_recovery_interval_seconds")]
    pub kline_recovery_interval_seconds: u64,
    #[serde(default = "default_kline_recovery_batch_limit")]
    pub kline_recovery_batch_limit: u32,
    #[serde(default = "default_seconds_contract_settlement_enabled")]
    pub seconds_contract_settlement_enabled: bool,
    #[serde(default = "default_seconds_contract_settlement_interval_seconds")]
    pub seconds_contract_settlement_interval_seconds: u64,
    #[serde(default = "default_seconds_contract_settlement_batch_limit")]
    pub seconds_contract_settlement_batch_limit: u32,
    #[serde(default = "default_earn_auto_redemption_enabled")]
    pub earn_auto_redemption_enabled: bool,
    #[serde(default = "default_earn_auto_redemption_interval_seconds")]
    pub earn_auto_redemption_interval_seconds: u64,
    #[serde(default = "default_earn_auto_redemption_batch_limit")]
    pub earn_auto_redemption_batch_limit: u32,
    #[serde(default = "default_margin_liquidation_enabled")]
    pub margin_liquidation_enabled: bool,
    #[serde(default = "default_margin_liquidation_interval_seconds")]
    pub margin_liquidation_interval_seconds: u64,
    #[serde(default = "default_margin_liquidation_batch_limit")]
    pub margin_liquidation_batch_limit: u32,
    #[serde(default = "default_margin_interest_enabled")]
    pub margin_interest_enabled: bool,
    #[serde(default = "default_margin_interest_interval_seconds")]
    pub margin_interest_interval_seconds: u64,
    #[serde(default = "default_margin_interest_batch_limit")]
    pub margin_interest_batch_limit: u32,
}

impl Settings {
    pub fn from_env() -> Result<Self, ConfigError> {
        dotenvy::dotenv().ok();

        Config::builder()
            .add_source(
                Environment::default()
                    .list_separator(",")
                    .with_list_parse_key("market_feed_symbols")
                    .with_list_parse_key("market_feed_intervals")
                    .with_list_parse_key("market_feed_providers")
                    .try_parsing(true),
            )
            .build()?
            .try_deserialize()
    }

    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.app_host, self.app_port)
    }

    pub fn exposed_database_url(&self) -> &str {
        self.database_url.expose_secret()
    }

    pub fn exposed_mongodb_uri(&self) -> &str {
        self.mongodb_uri.expose_secret()
    }

    pub fn exposed_redis_url(&self) -> &str {
        self.redis_url.expose_secret()
    }

    pub fn exposed_rabbitmq_url(&self) -> &str {
        self.rabbitmq_url.expose_secret()
    }

    pub fn exposed_credential_encryption_key(&self) -> Option<&str> {
        self.credential_encryption_key
            .as_ref()
            .map(SecretString::expose_secret)
            .map(String::as_str)
    }
}

fn deserialize_env_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let values = Vec::<String>::deserialize(deserializer)?;
    Ok(values
        .into_iter()
        .flat_map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .collect())
}

fn default_app_env() -> String {
    "local".to_owned()
}

fn default_host() -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))
}

fn default_port() -> u16 {
    8080
}

fn default_access_ttl() -> u64 {
    900
}

fn default_refresh_ttl() -> u64 {
    2_592_000
}

fn default_market_feed_reconnect_seconds() -> u64 {
    5
}

fn default_market_feed_rest_fallback_timeout_seconds() -> u64 {
    3
}

fn default_coinbase_rest_base_url() -> String {
    "https://api.coinbase.com".to_owned()
}

fn default_coinbase_ws_url() -> String {
    "wss://advanced-trade-ws.coinbase.com".to_owned()
}

fn default_event_inbox_retry_scan_seconds() -> u64 {
    10
}

fn default_event_outbox_publisher_enabled() -> bool {
    true
}

fn default_event_outbox_publisher_interval_seconds() -> u64 {
    5
}

fn default_unlock_scanner_enabled() -> bool {
    true
}

fn default_unlock_scanner_interval_seconds() -> u64 {
    10
}

fn default_unlock_scanner_batch_limit() -> u32 {
    100
}

fn default_kline_recovery_enabled() -> bool {
    true
}

fn default_kline_recovery_interval_seconds() -> u64 {
    30
}

fn default_kline_recovery_batch_limit() -> u32 {
    100
}

fn default_seconds_contract_settlement_enabled() -> bool {
    true
}

fn default_seconds_contract_settlement_interval_seconds() -> u64 {
    5
}

fn default_seconds_contract_settlement_batch_limit() -> u32 {
    100
}

fn default_earn_auto_redemption_enabled() -> bool {
    true
}

fn default_earn_auto_redemption_interval_seconds() -> u64 {
    60
}

fn default_earn_auto_redemption_batch_limit() -> u32 {
    100
}

fn default_margin_liquidation_enabled() -> bool {
    true
}

fn default_margin_liquidation_interval_seconds() -> u64 {
    5
}

fn default_margin_liquidation_batch_limit() -> u32 {
    100
}

fn default_margin_interest_enabled() -> bool {
    true
}

fn default_margin_interest_interval_seconds() -> u64 {
    60
}

fn default_margin_interest_batch_limit() -> u32 {
    100
}

#[cfg(test)]
#[path = "../tests/unit_src/src_config_tests.rs"]
mod tests;
