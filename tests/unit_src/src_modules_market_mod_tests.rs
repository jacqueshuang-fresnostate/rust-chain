use super::adapters::MarketFeedEvent;
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
fn market_provenance_ticker_and_candle_cache_rest_ws_agree() {
    use super::presentation::{KlineResponse, TickerResponse};
    let open = Utc.with_ymd_and_hms(2026, 9, 18, 12, 0, 0).unwrap();
    let observed = open + chrono::Duration::seconds(30);
    for (provider, source) in [
        (MarketDataProvider::Bitget, "external"),
        (MarketDataProvider::Htx, "external"),
        (MarketDataProvider::Coinbase, "external"),
        (MarketDataProvider::Strategy, "generated"),
    ] {
        let ticker =
            MarketTickerSnapshot::new(provider, "BTCUSDT", decimal("1.25"), decimal("2"), observed)
                .unwrap();
        let cached =
            serde_json::to_value(MarketTickerCacheEntry::from_snapshot(&ticker).unwrap()).unwrap();
        let rest: TickerResponse = serde_json::from_value(cached).unwrap();
        let rest = serde_json::to_value(rest).unwrap();
        let event = MarketFeedEvent::from_ticker_snapshot(&ticker).unwrap();
        assert_eq!(rest["source"], source);
        for field in ["source", "provider", "observed_at", "last_price"] {
            assert_eq!(event.payload()[field], rest[field]);
        }
        let candle = MarketKlineSnapshot::new(
            provider,
            "BTCUSDT",
            "1m",
            open,
            MarketKlineValues {
                open: decimal("1"),
                high: decimal("2"),
                low: decimal("0.5"),
                close: decimal("1.25"),
                volume: decimal("2"),
            },
            observed,
        )
        .unwrap();
        let cached =
            serde_json::to_value(MarketKlineCacheEntry::from_snapshot(&candle).unwrap()).unwrap();
        let rest: KlineResponse = serde_json::from_value(cached).unwrap();
        let rest = serde_json::to_value(rest).unwrap();
        let event = MarketFeedEvent::from_kline_snapshot(&candle).unwrap();
        assert_eq!(rest["source"], source);
        assert_ne!(rest["observed_at"], rest["open_time"]);
        for field in ["source", "provider", "observed_at", "open_time", "close"] {
            assert_eq!(event.payload()[field], rest[field]);
        }
    }
}

#[test]
fn market_provenance_legacy_ticker_candle_and_mongo_remain_unknown() {
    use super::presentation::{KlineResponse, TickerResponse};
    let time = 1_790_000_000_000_i64;
    let ticker: TickerResponse = serde_json::from_value(serde_json::json!({
        "symbol": "BTCUSDT", "last_price": "1.25", "volume_24h": "2", "observed_at": time,
    }))
    .unwrap();
    assert_eq!(ticker.provenance.source, "unknown");
    assert!(ticker.provenance.provider.is_none());
    let candle: KlineResponse = serde_json::from_value(serde_json::json!({
        "symbol": "BTCUSDT", "interval": "1m", "open_time": time,
        "open": "1", "high": "2", "low": "0.5", "close": "1.25", "volume": "2",
    }))
    .unwrap();
    assert_eq!(candle.provenance.source, "unknown");
    assert!(candle.observed_at.is_none());
    for (stored, expected) in [
        (None, "unknown"),
        (Some("future"), "unknown"),
        (Some("strategy"), "generated"),
        (Some("htx"), "external"),
    ] {
        let mut doc = doc! {
            "interval": "1m", "open_time": BsonDateTime::from_millis(time),
            "open": "1", "high": "2", "low": "0.5", "close": "1.25", "volume": "2",
        };
        if let Some(source) = stored {
            doc.insert("source", source);
            doc.insert("updated_at", BsonDateTime::from_millis(time + 30_000));
        }
        let row = mongodb::bson::from_document(doc).unwrap();
        let response = KlineResponse::from_document("BTCUSDT", row);
        assert_eq!(response.provenance.source, expected);
        assert_eq!(
            response.observed_at.map(|at| at.timestamp_millis()),
            stored.map(|_| time + 30_000)
        );
    }
}

#[test]
fn market_provenance_depth_cache_rest_and_ws_preserve_provider() {
    use super::presentation::{DepthCachePayload, DepthResponse};
    for (provider, source, name) in [
        (MarketDataProvider::Bitget, "external", "bitget"),
        (MarketDataProvider::Htx, "external", "htx"),
        (MarketDataProvider::Coinbase, "external", "coinbase"),
        (MarketDataProvider::Strategy, "generated", "strategy"),
    ] {
        let snapshot =
            MarketDepthSnapshot::new(provider, "BTCUSDT", vec![], vec![], Utc::now()).unwrap();
        let cache =
            serde_json::to_value(MarketDepthCacheEntry::from_snapshot(&snapshot).unwrap()).unwrap();
        let rest = serde_json::to_value(DepthResponse::from_cache(
            serde_json::from_value::<DepthCachePayload>(cache).unwrap(),
        ))
        .unwrap();
        let event = MarketFeedEvent::from_depth_snapshot(&snapshot).unwrap();
        assert_eq!(rest["source"], source);
        assert_eq!(rest["provider"], name);
        assert_eq!(event.payload()["source"], rest["source"]);
        assert_eq!(event.payload()["provider"], rest["provider"]);
    }
}

#[test]
fn market_provenance_legacy_depth_never_invents_a_provider() {
    let cache =
        serde_json::json!({"symbol":"BTCUSDT","bids":[],"asks":[],"observed_at":1790000000000_i64});
    let response = presentation::DepthResponse::from_cache(serde_json::from_value(cache).unwrap());
    let body = serde_json::to_value(response).unwrap();
    assert_eq!(body["source"], "unknown");
    assert!(body["provider"].is_null());
}

#[test]
fn market_provenance_generated_depth_uses_the_same_evidence_in_cache_and_event() {
    let time = Utc.with_ymd_and_hms(2026, 9, 18, 12, 0, 0).unwrap();
    for source in ["strategy", "default"] {
        let tick = MarketTradeTick::new(
            MarketDataProvider::Strategy,
            "BTCUSDT",
            format!("{source}:42:v2:{}", time.timestamp()),
            MarketTradeSide::Buy,
            decimal("1"),
            decimal("2"),
            time,
        )
        .unwrap();
        let evidence = presentation::MarketProvenance::from_tick(&tick);
        let depth = MarketDepthSnapshot::new(
            MarketDataProvider::Strategy,
            "BTCUSDT",
            vec![],
            vec![],
            time,
        )
        .unwrap();
        let cache = MarketDepthCacheEntry::from_snapshot(&depth)
            .unwrap()
            .with_provenance(evidence.clone());
        let event = MarketFeedEvent::from_depth_snapshot(&depth)
            .unwrap()
            .with_depth_provenance(&evidence.source);
        let response = presentation::DepthResponse::from_cache(
            serde_json::from_value(serde_json::to_value(cache).unwrap()).unwrap(),
        );
        let body = serde_json::to_value(response).unwrap();
        assert_eq!(body["source"], source);
        assert_eq!(event.payload()["source"], body["source"]);
        assert_eq!(event.payload()["provider"], body["provider"]);
    }
}

#[test]
fn market_provenance_generated_trade_identity_is_evidence_not_pair_configuration() {
    let time = Utc.with_ymd_and_hms(2026, 9, 18, 12, 0, 0).unwrap();
    for (provider, id, source) in [
        (
            MarketDataProvider::Strategy,
            format!("default:42:v2:{}", time.timestamp()),
            "default",
        ),
        (
            MarketDataProvider::Strategy,
            format!("strategy:9:v1:{}", time.timestamp()),
            "strategy",
        ),
        (
            MarketDataProvider::Strategy,
            "default:unverified".into(),
            "generated",
        ),
        (
            MarketDataProvider::Strategy,
            "default:42:v2:0".into(),
            "generated",
        ),
        (
            MarketDataProvider::Htx,
            format!("default:42:v2:{}", time.timestamp()),
            "external",
        ),
    ] {
        let tick = MarketTradeTick::new(
            provider,
            "BTCUSDT",
            id,
            MarketTradeSide::Sell,
            decimal("1.20"),
            decimal("2"),
            time,
        )
        .unwrap();
        let event = MarketFeedEvent::from_trade_tick(&tick).unwrap();
        let rest =
            serde_json::to_value(presentation::TradeResponse::from_synthetic_tick(tick)).unwrap();
        assert_eq!(rest["source"], source);
        assert_eq!(event.payload()["source"], rest["source"]);
        assert_eq!(event.payload()["provider"], rest["provider"]);
        assert_eq!(rest["direction"], "SELL");
    }
}

#[test]
fn market_provenance_only_platform_record_conversion_labels_platform_trade() {
    let trade = presentation::TradeResponse::from_record(repository::SpotTradeRecord {
        id: 7,
        symbol: "BTC_USDT".into(),
        price: decimal("1.20"),
        quantity: decimal("2"),
        created_at: Utc::now(),
    });
    let body = serde_json::to_value(trade).unwrap();
    assert_eq!(body["source"], "platform");
    assert_eq!(body["provider"], "platform");
    assert_eq!(body["symbol"], "BTCUSDT");
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

#[test]
fn forming_read_model_merges_only_current_in_range_slot_and_keeps_latest_limit() {
    use super::presentation::KlineResponse;
    let open = Utc.with_ymd_and_hms(2026, 9, 6, 10, 0, 0).unwrap();
    let candle = |time, close: &str| KlineResponse {
        provenance: Default::default(),
        observed_at: None,
        symbol: "SIMUSDT".into(),
        interval: "5m".into(),
        open_time: time,
        open: "10".into(),
        high: "20".into(),
        low: "5".into(),
        close: close.into(),
        volume: "60".into(),
    };
    let query = KlineQuery::new("5m", None, None, Some(2)).unwrap();
    let now = open + chrono::TimeDelta::minutes(2);
    let mut rows = vec![
        candle(open - chrono::TimeDelta::minutes(10), "11"),
        candle(open - chrono::TimeDelta::minutes(5), "12"),
    ];
    service::merge_current_kline(&mut rows, candle(open, "13"), &query, now);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].close, "12");
    assert_eq!(rows[1].close, "13");
    service::merge_current_kline(&mut rows, candle(open, "99"), &query, now);
    assert_eq!(
        rows[1].close, "13",
        "existing root is not duplicated or overwritten"
    );
    let mut empty = vec![];
    service::merge_current_kline(
        &mut empty,
        candle(open - chrono::TimeDelta::minutes(5), "10"),
        &query,
        now,
    );
    service::merge_current_kline(
        &mut empty,
        candle(open + chrono::TimeDelta::minutes(5), "10"),
        &query,
        now,
    );
    assert!(empty.is_empty(), "expired or future cache is not history");
    let bounded = KlineQuery::new(
        "5m",
        Some(open + chrono::TimeDelta::minutes(1)),
        None,
        Some(2),
    )
    .unwrap();
    service::merge_current_kline(&mut empty, candle(open, "10"), &bounded, now);
    assert!(empty.is_empty());
}
