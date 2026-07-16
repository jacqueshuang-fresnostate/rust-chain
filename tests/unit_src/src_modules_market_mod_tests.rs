use super::*;
use bigdecimal::BigDecimal;
use chrono::{TimeZone, Utc};
use mongodb::bson::{DateTime as BsonDateTime, doc};
use std::str::FromStr;

fn kline_upsert_filter_for_test(key: &KlineUpsertKey) -> mongodb::bson::Document {
    doc! {
        "interval": key.interval(),
        "open_time": BsonDateTime::from_millis(key.open_time().timestamp_millis()),
    }
}

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}

#[test]
fn validated_symbol_accepts_only_sanitized_whitelisted_values() {
    let allowed = ["BTC/USDT", "NEW-USDT"];

    let symbol = ValidatedMarketSymbol::from_allowed("btc_usdt", allowed).unwrap();

    assert_eq!(symbol.as_str(), "BTCUSDT");
    assert_eq!(kline_collection_name(&symbol), "market_klines_BTCUSDT");
    assert!(ValidatedMarketSymbol::from_allowed("ETHUSDT", allowed).is_err());
    assert!(ValidatedMarketSymbol::from_raw("BTC.USDT").is_err());
    assert!(ValidatedMarketSymbol::from_raw("***").is_err());
}

#[test]
fn kline_upsert_key_uses_interval_and_open_time_only() {
    let open_time = Utc.with_ymd_and_hms(2026, 5, 26, 9, 0, 0).unwrap();

    let key = KlineUpsertKey::new("1m", open_time).unwrap();

    assert_eq!(key.interval(), "1m");
    assert_eq!(key.open_time(), open_time);
    assert_eq!(
        kline_upsert_filter_for_test(&key),
        doc! { "interval": "1m", "open_time": BsonDateTime::from_millis(open_time.timestamp_millis()) }
    );
    assert!(KlineUpsertKey::new("2m", open_time).is_err());
}

#[test]
fn market_ticker_cache_entry_normalizes_symbol_and_redis_key() {
    let observed_at = Utc.with_ymd_and_hms(2026, 5, 26, 16, 0, 0).unwrap();

    let entry = MarketTickerCacheEntry::new(
        "btc-usdt",
        decimal("70000.120000000000000000"),
        decimal("125.500000000000000000"),
        observed_at,
    )
    .unwrap();

    assert_eq!(entry.symbol(), "BTCUSDT");
    assert_eq!(entry.redis_key(), "market:ticker:BTCUSDT");
    assert_eq!(entry.last_price(), &decimal("70000.120000000000000000"));
    assert_eq!(entry.high_24h(), &decimal("70000.120000000000000000"));
    assert_eq!(entry.low_24h(), &decimal("70000.120000000000000000"));
    assert_eq!(entry.volume_24h(), &decimal("125.500000000000000000"));
    assert_eq!(entry.price_change_24h(), &decimal("0"));
    assert_eq!(entry.price_change_percent_24h(), &decimal("0"));
    assert_eq!(entry.observed_at(), observed_at);
    assert!(
        MarketTickerCacheEntry::new("BTC.USDT", decimal("1"), decimal("1"), observed_at).is_err()
    );
}

#[test]
fn market_depth_cache_entry_keeps_bid_ask_levels_and_key() {
    let observed_at = Utc.with_ymd_and_hms(2026, 5, 26, 16, 1, 0).unwrap();

    let entry = MarketDepthCacheEntry::new(
        "new_usdt",
        vec![MarketDepthLevel::new(decimal("1.10"), decimal("50"))],
        vec![MarketDepthLevel::new(decimal("1.11"), decimal("40"))],
        observed_at,
    )
    .unwrap();

    assert_eq!(entry.symbol(), "NEWUSDT");
    assert_eq!(entry.redis_key(), "market:depth:NEWUSDT");
    assert_eq!(entry.bids()[0].price, decimal("1.10"));
    assert_eq!(entry.asks()[0].quantity, decimal("40"));
    assert_eq!(entry.observed_at(), observed_at);
}

#[test]
fn market_kline_cache_entry_validates_interval_and_key() {
    let open_time = Utc.with_ymd_and_hms(2026, 5, 26, 16, 2, 0).unwrap();

    let entry = MarketKlineCacheEntry::new(
        "new-usdt",
        "1m",
        open_time,
        MarketKlineValues {
            open: decimal("1.00"),
            high: decimal("1.20"),
            low: decimal("0.95"),
            close: decimal("1.10"),
            volume: decimal("1000"),
        },
    )
    .unwrap();

    assert_eq!(entry.symbol(), "NEWUSDT");
    assert_eq!(entry.interval(), "1m");
    assert_eq!(entry.open_time(), open_time);
    assert_eq!(entry.redis_key(), "market:kline:NEWUSDT:1m");
    assert_eq!(entry.close(), &decimal("1.10"));
    assert!(
        MarketKlineCacheEntry::new(
            "new-usdt",
            "2m",
            open_time,
            MarketKlineValues {
                open: decimal("1.00"),
                high: decimal("1.20"),
                low: decimal("0.95"),
                close: decimal("1.10"),
                volume: decimal("1000"),
            },
        )
        .is_err()
    );
}

#[test]
fn market_adapter_snapshots_normalize_provider_symbols() {
    let observed_at = Utc.with_ymd_and_hms(2026, 5, 26, 16, 3, 0).unwrap();

    let ticker = MarketTickerSnapshot::new(
        MarketDataProvider::Bitget,
        "btc_usdt",
        decimal("70000.12"),
        decimal("125.50"),
        observed_at,
    )
    .unwrap();
    let depth = MarketDepthSnapshot::new(
        MarketDataProvider::Htx,
        "eth-usdt",
        vec![MarketDepthLevel::new(decimal("3000.00"), decimal("2.50"))],
        vec![MarketDepthLevel::new(decimal("3001.00"), decimal("1.50"))],
        observed_at,
    )
    .unwrap();

    assert_eq!(ticker.provider(), MarketDataProvider::Bitget);
    assert_eq!(ticker.symbol(), "BTCUSDT");
    assert_eq!(ticker.last_price(), &decimal("70000.12"));
    assert_eq!(ticker.high_24h(), &decimal("70000.12"));
    assert_eq!(ticker.low_24h(), &decimal("70000.12"));
    assert_eq!(ticker.price_change_24h(), &decimal("0"));
    assert_eq!(depth.provider(), MarketDataProvider::Htx);
    assert_eq!(depth.symbol(), "ETHUSDT");
    assert_eq!(depth.bids()[0].quantity, decimal("2.50"));
    assert!(
        MarketTickerSnapshot::new(
            MarketDataProvider::Bitget,
            "BTC.USDT",
            decimal("1"),
            decimal("1"),
            observed_at,
        )
        .is_err()
    );
}

#[test]
fn market_kline_snapshot_reuses_interval_validation() {
    let open_time = Utc.with_ymd_and_hms(2026, 5, 26, 16, 4, 0).unwrap();
    let observed_at = Utc.with_ymd_and_hms(2026, 5, 26, 16, 4, 30).unwrap();
    let values = MarketKlineValues {
        open: decimal("1.00"),
        high: decimal("1.20"),
        low: decimal("0.95"),
        close: decimal("1.10"),
        volume: decimal("1000"),
    };

    let kline = MarketKlineSnapshot::new(
        MarketDataProvider::Strategy,
        "new-usdt",
        "1m",
        open_time,
        values.clone(),
        observed_at,
    )
    .unwrap();

    assert_eq!(kline.provider(), MarketDataProvider::Strategy);
    assert_eq!(kline.symbol(), "NEWUSDT");
    assert_eq!(kline.interval(), "1m");
    assert_eq!(kline.open_time(), open_time);
    assert_eq!(kline.close(), &decimal("1.10"));
    assert!(
        MarketKlineSnapshot::new(
            MarketDataProvider::Strategy,
            "new-usdt",
            "2m",
            open_time,
            values,
            observed_at,
        )
        .is_err()
    );
}

#[test]
fn market_trade_tick_carries_provider_side_and_trade_time() {
    let traded_at = Utc.with_ymd_and_hms(2026, 5, 26, 16, 5, 0).unwrap();

    let trade = MarketTradeTick::new(
        MarketDataProvider::Htx,
        "btc-usdt",
        "trade-1",
        MarketTradeSide::Buy,
        decimal("70000"),
        decimal("0.25"),
        traded_at,
    )
    .unwrap();

    assert_eq!(trade.provider(), MarketDataProvider::Htx);
    assert_eq!(trade.symbol(), "BTCUSDT");
    assert_eq!(trade.trade_id(), "trade-1");
    assert_eq!(trade.side(), MarketTradeSide::Buy);
    assert_eq!(trade.price(), &decimal("70000"));
    assert_eq!(trade.quantity(), &decimal("0.25"));
    assert_eq!(trade.traded_at(), traded_at);
}

#[test]
fn bitget_ticker_from_ws_accepts_snapshot_payload_shape() {
    let ticker = adapters::BitgetMarketAdapter::ticker_from_ws(
        r#"{
            "action": "snapshot",
            "arg": {
                "instType": "SPOT",
                "channel": "ticker",
                "instId": "ETHUSDT"
            },
            "data": [{
                "instId": "ETHUSDT",
                "lastPr": "2026.88",
                "open24h": "2026.73",
                "high24h": "2032.21",
                "low24h": "2001.94",
                "change24h": "-0.00188",
                "bidPr": "2026.99",
                "askPr": "2027",
                "bidSz": "48.9485",
                "askSz": "27.1205",
                "baseVolume": "58343.4208",
                "quoteVolume": "117761880.4947",
                "openUtc": "2014.43",
                "changeUtc24h": "0.00618",
                "ts": "1780163523579"
            }],
            "ts": 1780163523581
        }"#,
    )
    .unwrap();

    assert_eq!(ticker.provider(), MarketDataProvider::Bitget);
    assert_eq!(ticker.symbol(), "ETHUSDT");
    assert_eq!(ticker.last_price(), &decimal("2026.88"));
    assert_eq!(ticker.high_24h(), &decimal("2032.21"));
    assert_eq!(ticker.low_24h(), &decimal("2001.94"));
    assert_eq!(ticker.volume_24h(), &decimal("58343.4208"));
    assert_eq!(ticker.price_change_24h(), &decimal("0.15"));
    assert_eq!(ticker.price_change_percent_24h(), &decimal("-0.18800"));
    assert_eq!(ticker.observed_at().timestamp_millis(), 1780163523579);
}
