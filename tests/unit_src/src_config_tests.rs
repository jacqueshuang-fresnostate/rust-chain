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
    set_test_env("COINBASE_REST_BASE_URL", "https://coinbase.test");
    set_test_env("COINBASE_WS_URL", "wss://coinbase.test/ws");
    set_test_env("MARKET_FEED_SYMBOLS", "BTC-USDT,ETHUSDT");
    set_test_env("MARKET_FEED_INTERVALS", "1m,5m");
    set_test_env("MARKET_FEED_PROVIDERS", "bitget,htx,coinbase");
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
    assert_eq!(
        settings.market_feed_providers,
        ["bitget", "htx", "coinbase"]
    );
    assert_eq!(settings.coinbase_rest_base_url, "https://coinbase.test");
    assert_eq!(settings.coinbase_ws_url, "wss://coinbase.test/ws");
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
    assert_eq!(settings.coinbase_rest_base_url, "https://api.coinbase.com");
    assert_eq!(
        settings.coinbase_ws_url,
        "wss://advanced-trade-ws.coinbase.com"
    );
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
        env::remove_var("COINBASE_REST_BASE_URL");
        env::remove_var("COINBASE_WS_URL");
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
