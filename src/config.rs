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
    IpAddr::V4(Ipv4Addr::LOCALHOST)
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
mod tests {
    use super::*;
    use std::{env, sync::Mutex};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn settings_from_env_parses_market_feed_lists() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        set_test_env("DATABASE_URL", "mysql://test:test@localhost/test");
        set_test_env("MONGODB_URI", "mongodb://localhost:27017");
        set_test_env("MONGODB_DATABASE", "exchange_test");
        set_test_env("REDIS_URL", "redis://localhost:6379");
        set_test_env("RABBITMQ_URL", "amqp://guest:guest@localhost:5672/%2f");
        set_test_env("JWT_SECRET", "test-secret");
        set_test_env(
            "CREDENTIAL_ENCRYPTION_KEY",
            "0123456789abcdef0123456789abcdef",
        );
        set_test_env("BITGET_REST_BASE_URL", "https://bitget.test");
        set_test_env("BITGET_WS_URL", "wss://bitget.test/ws");
        set_test_env("HTX_REST_BASE_URL", "https://htx.test");
        set_test_env("HTX_WS_URL", "wss://htx.test/ws");
        set_test_env("MARKET_FEED_SYMBOLS", "BTC-USDT,ETHUSDT");
        set_test_env("MARKET_FEED_INTERVALS", "1m,5m");
        set_test_env("MARKET_FEED_PROVIDERS", "bitget,htx");
        set_test_env("MARKET_FEED_RECONNECT_SECONDS", "9");
        set_test_env("MARKET_FEED_REST_FALLBACK_TIMEOUT_SECONDS", "7");
        set_test_env("EVENT_INBOX_RETRY_SCAN_SECONDS", "11");
        set_test_env("EVENT_OUTBOX_PUBLISHER_ENABLED", "false");
        set_test_env("EVENT_OUTBOX_PUBLISHER_INTERVAL_SECONDS", "12");
        set_test_env("UNLOCK_SCANNER_ENABLED", "false");
        set_test_env("UNLOCK_SCANNER_INTERVAL_SECONDS", "13");
        set_test_env("UNLOCK_SCANNER_BATCH_LIMIT", "77");
        set_test_env("KLINE_RECOVERY_ENABLED", "false");
        set_test_env("KLINE_RECOVERY_INTERVAL_SECONDS", "17");
        set_test_env("KLINE_RECOVERY_BATCH_LIMIT", "55");
        set_test_env("SECONDS_CONTRACT_SETTLEMENT_ENABLED", "false");
        set_test_env("SECONDS_CONTRACT_SETTLEMENT_INTERVAL_SECONDS", "19");
        set_test_env("SECONDS_CONTRACT_SETTLEMENT_BATCH_LIMIT", "66");
        set_test_env("EARN_AUTO_REDEMPTION_ENABLED", "false");
        set_test_env("EARN_AUTO_REDEMPTION_INTERVAL_SECONDS", "23");
        set_test_env("EARN_AUTO_REDEMPTION_BATCH_LIMIT", "44");
        set_test_env("MARGIN_LIQUIDATION_ENABLED", "false");
        set_test_env("MARGIN_LIQUIDATION_INTERVAL_SECONDS", "29");
        set_test_env("MARGIN_LIQUIDATION_BATCH_LIMIT", "33");
        set_test_env("MARGIN_INTEREST_ENABLED", "false");
        set_test_env("MARGIN_INTEREST_INTERVAL_SECONDS", "31");
        set_test_env("MARGIN_INTEREST_BATCH_LIMIT", "37");

        let settings = Settings::from_env().unwrap();

        assert_eq!(
            settings.exposed_credential_encryption_key(),
            Some("0123456789abcdef0123456789abcdef")
        );
        assert_eq!(settings.market_feed_symbols, ["BTC-USDT", "ETHUSDT"]);
        assert_eq!(settings.market_feed_intervals, ["1m", "5m"]);
        assert_eq!(settings.market_feed_providers, ["bitget", "htx"]);
        assert_eq!(settings.market_feed_reconnect_seconds, 9);
        assert_eq!(settings.market_feed_rest_fallback_timeout_seconds, 7);
        assert_eq!(settings.event_inbox_retry_scan_seconds, 11);
        assert!(!settings.event_outbox_publisher_enabled);
        assert_eq!(settings.event_outbox_publisher_interval_seconds, 12);
        assert!(!settings.unlock_scanner_enabled);
        assert_eq!(settings.unlock_scanner_interval_seconds, 13);
        assert_eq!(settings.unlock_scanner_batch_limit, 77);
        assert!(!settings.kline_recovery_enabled);
        assert_eq!(settings.kline_recovery_interval_seconds, 17);
        assert_eq!(settings.kline_recovery_batch_limit, 55);
        assert!(!settings.seconds_contract_settlement_enabled);
        assert_eq!(settings.seconds_contract_settlement_interval_seconds, 19);
        assert_eq!(settings.seconds_contract_settlement_batch_limit, 66);
        assert!(!settings.earn_auto_redemption_enabled);
        assert_eq!(settings.earn_auto_redemption_interval_seconds, 23);
        assert_eq!(settings.earn_auto_redemption_batch_limit, 44);
        assert!(!settings.margin_liquidation_enabled);
        assert_eq!(settings.margin_liquidation_interval_seconds, 29);
        assert_eq!(settings.margin_liquidation_batch_limit, 33);
        assert!(!settings.margin_interest_enabled);
        assert_eq!(settings.margin_interest_interval_seconds, 31);
        assert_eq!(settings.margin_interest_batch_limit, 37);
        clear_market_feed_env();
    }

    #[test]
    fn settings_from_env_accepts_empty_market_feed_lists() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        set_test_env("DATABASE_URL", "mysql://test:test@localhost/test");
        set_test_env("MONGODB_URI", "mongodb://localhost:27017");
        set_test_env("MONGODB_DATABASE", "exchange_test");
        set_test_env("REDIS_URL", "redis://localhost:6379");
        set_test_env("RABBITMQ_URL", "amqp://guest:guest@localhost:5672/%2f");
        set_test_env("JWT_SECRET", "test-secret");
        set_test_env(
            "CREDENTIAL_ENCRYPTION_KEY",
            "0123456789abcdef0123456789abcdef",
        );
        set_test_env("BITGET_REST_BASE_URL", "https://bitget.test");
        set_test_env("BITGET_WS_URL", "wss://bitget.test/ws");
        set_test_env("HTX_REST_BASE_URL", "https://htx.test");
        set_test_env("HTX_WS_URL", "wss://htx.test/ws");
        set_test_env("MARKET_FEED_SYMBOLS", "");
        set_test_env("MARKET_FEED_INTERVALS", "");
        set_test_env("MARKET_FEED_PROVIDERS", "");
        unsafe {
            env::remove_var("MARKET_FEED_RECONNECT_SECONDS");
        }

        let settings = Settings::from_env().unwrap();

        assert!(settings.market_feed_symbols.is_empty());
        assert!(settings.market_feed_intervals.is_empty());
        assert!(settings.market_feed_providers.is_empty());
        assert_eq!(settings.market_feed_reconnect_seconds, 5);
        assert_eq!(settings.market_feed_rest_fallback_timeout_seconds, 3);
        assert_eq!(settings.event_inbox_retry_scan_seconds, 10);
        assert!(settings.event_outbox_publisher_enabled);
        assert_eq!(settings.event_outbox_publisher_interval_seconds, 5);
        assert!(settings.unlock_scanner_enabled);
        assert_eq!(settings.unlock_scanner_interval_seconds, 10);
        assert_eq!(settings.unlock_scanner_batch_limit, 100);
        assert!(settings.kline_recovery_enabled);
        assert_eq!(settings.kline_recovery_interval_seconds, 30);
        assert_eq!(settings.kline_recovery_batch_limit, 100);
        assert!(settings.seconds_contract_settlement_enabled);
        assert_eq!(settings.seconds_contract_settlement_interval_seconds, 5);
        assert_eq!(settings.seconds_contract_settlement_batch_limit, 100);
        assert!(settings.earn_auto_redemption_enabled);
        assert_eq!(settings.earn_auto_redemption_interval_seconds, 60);
        assert_eq!(settings.earn_auto_redemption_batch_limit, 100);
        assert!(settings.margin_liquidation_enabled);
        assert_eq!(settings.margin_liquidation_interval_seconds, 5);
        assert_eq!(settings.margin_liquidation_batch_limit, 100);
        assert!(settings.margin_interest_enabled);
        assert_eq!(settings.margin_interest_interval_seconds, 60);
        assert_eq!(settings.margin_interest_batch_limit, 100);
        clear_market_feed_env();
    }

    fn set_test_env(key: &str, value: &str) {
        unsafe {
            env::set_var(key, value);
        }
    }

    fn clear_market_feed_env() {
        unsafe {
            env::remove_var("CREDENTIAL_ENCRYPTION_KEY");
            env::remove_var("MARKET_FEED_SYMBOLS");
            env::remove_var("MARKET_FEED_INTERVALS");
            env::remove_var("MARKET_FEED_PROVIDERS");
            env::remove_var("MARKET_FEED_RECONNECT_SECONDS");
            env::remove_var("MARKET_FEED_REST_FALLBACK_TIMEOUT_SECONDS");
            env::remove_var("EVENT_INBOX_RETRY_SCAN_SECONDS");
            env::remove_var("EVENT_OUTBOX_PUBLISHER_ENABLED");
            env::remove_var("EVENT_OUTBOX_PUBLISHER_INTERVAL_SECONDS");
            env::remove_var("UNLOCK_SCANNER_ENABLED");
            env::remove_var("UNLOCK_SCANNER_INTERVAL_SECONDS");
            env::remove_var("UNLOCK_SCANNER_BATCH_LIMIT");
            env::remove_var("KLINE_RECOVERY_ENABLED");
            env::remove_var("KLINE_RECOVERY_INTERVAL_SECONDS");
            env::remove_var("KLINE_RECOVERY_BATCH_LIMIT");
            env::remove_var("SECONDS_CONTRACT_SETTLEMENT_ENABLED");
            env::remove_var("SECONDS_CONTRACT_SETTLEMENT_INTERVAL_SECONDS");
            env::remove_var("SECONDS_CONTRACT_SETTLEMENT_BATCH_LIMIT");
            env::remove_var("EARN_AUTO_REDEMPTION_ENABLED");
            env::remove_var("EARN_AUTO_REDEMPTION_INTERVAL_SECONDS");
            env::remove_var("EARN_AUTO_REDEMPTION_BATCH_LIMIT");
            env::remove_var("MARGIN_LIQUIDATION_ENABLED");
            env::remove_var("MARGIN_LIQUIDATION_INTERVAL_SECONDS");
            env::remove_var("MARGIN_LIQUIDATION_BATCH_LIMIT");
            env::remove_var("MARGIN_INTEREST_ENABLED");
            env::remove_var("MARGIN_INTEREST_INTERVAL_SECONDS");
            env::remove_var("MARGIN_INTEREST_BATCH_LIMIT");
        }
    }
}
