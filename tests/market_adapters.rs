use bigdecimal::BigDecimal;
use chrono::{TimeZone, Utc};
use exchange_api::modules::market::{
    MarketDataProvider, MarketDepthLevel, MarketKlineCacheEntry, MarketTickerCacheEntry,
    MarketTradeSide,
    adapters::{BitgetMarketAdapter, HtxMarketAdapter, MarketKlineMongoWrite},
};
use mongodb::bson::{DateTime as BsonDateTime, doc};
use std::str::FromStr;

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}

#[test]
fn bitget_adapter_parses_ws_ticker_depth_kline_and_trade() {
    let ticker = BitgetMarketAdapter::ticker_from_ws(
        r#"{
            "arg":{"channel":"ticker","instId":"BTCUSDT"},
            "data":[{"instId":"BTCUSDT","lastPr":"70000.12","baseVolume":"125.50","ts":"1710000000000"}]
        }"#,
    )
    .unwrap();
    let depth = BitgetMarketAdapter::depth_from_ws(
        r#"{
            "arg":{"channel":"books","instId":"BTCUSDT"},
            "data":[{"bids":[["70000.00","0.50"]],"asks":[["70001.00","0.40"]],"ts":"1710000001000"}]
        }"#,
    )
    .unwrap();
    let kline = BitgetMarketAdapter::kline_from_ws(
        r#"{
            "arg":{"channel":"candle1m","instId":"BTCUSDT"},
            "data":[["1710000000000","70000.00","70010.00","69990.00","70005.00","12.30"]]
        }"#,
    )
    .unwrap();
    let trade = BitgetMarketAdapter::trade_from_ws(
        r#"{
            "arg":{"channel":"trade","instId":"BTCUSDT"},
            "data":[{"tradeId":"bt-1","side":"buy","price":"70000.00","size":"0.25","ts":"1710000002000"}]
        }"#,
    )
    .unwrap();

    assert_eq!(ticker.provider(), MarketDataProvider::Bitget);
    assert_eq!(ticker.symbol(), "BTCUSDT");
    assert_eq!(ticker.last_price(), &decimal("70000.12"));
    assert_eq!(ticker.volume_24h(), &decimal("125.50"));
    assert_eq!(
        ticker.observed_at(),
        Utc.timestamp_millis_opt(1_710_000_000_000).unwrap()
    );
    assert_eq!(depth.provider(), MarketDataProvider::Bitget);
    assert_eq!(
        depth.bids(),
        &[MarketDepthLevel::new(decimal("70000.00"), decimal("0.50"))]
    );
    assert_eq!(
        depth.asks(),
        &[MarketDepthLevel::new(decimal("70001.00"), decimal("0.40"))]
    );
    assert_eq!(kline.provider(), MarketDataProvider::Bitget);
    assert_eq!(kline.interval(), "1m");
    assert_eq!(
        kline.open_time(),
        Utc.timestamp_millis_opt(1_710_000_000_000).unwrap()
    );
    assert_eq!(kline.close(), &decimal("70005.00"));
    assert_eq!(trade.provider(), MarketDataProvider::Bitget);
    assert_eq!(trade.trade_id(), "bt-1");
    assert_eq!(trade.side(), MarketTradeSide::Buy);
    assert_eq!(trade.quantity(), &decimal("0.25"));
}

#[test]
fn htx_adapter_parses_ws_ticker_depth_kline_and_trade() {
    let ticker = HtxMarketAdapter::ticker_from_ws(
        r#"{
            "ch":"market.btcusdt.ticker",
            "ts":1710000000000,
            "tick":{"close":70000.12,"amount":125.50}
        }"#,
    )
    .unwrap();
    let depth = HtxMarketAdapter::depth_from_ws(
        r#"{
            "ch":"market.btcusdt.depth.step0",
            "ts":1710000001000,
            "tick":{"bids":[[70000.00,0.50]],"asks":[[70001.00,0.40]],"ts":1710000001000}
        }"#,
    )
    .unwrap();
    let kline = HtxMarketAdapter::kline_from_ws(
        r#"{
            "ch":"market.btcusdt.kline.1min",
            "ts":1710000002000,
            "tick":{"id":1710000000,"open":70000.00,"high":70010.00,"low":69990.00,"close":70005.00,"amount":12.30}
        }"#,
    )
    .unwrap();
    let trade = HtxMarketAdapter::trade_from_ws(
        r#"{
            "ch":"market.btcusdt.trade.detail",
            "ts":1710000003000,
            "tick":{"data":[{"id":1001,"ts":1710000003000,"direction":"sell","price":70000.00,"amount":0.25}]}
        }"#,
    )
    .unwrap();

    assert_eq!(ticker.provider(), MarketDataProvider::Htx);
    assert_eq!(ticker.symbol(), "BTCUSDT");
    assert_eq!(ticker.last_price(), &decimal("70000.12"));
    assert_eq!(ticker.volume_24h(), &decimal("125.50"));
    assert_eq!(depth.provider(), MarketDataProvider::Htx);
    assert_eq!(
        depth.bids(),
        &[MarketDepthLevel::new(decimal("70000.0"), decimal("0.5"))]
    );
    assert_eq!(
        depth.asks(),
        &[MarketDepthLevel::new(decimal("70001.0"), decimal("0.4"))]
    );
    assert_eq!(kline.provider(), MarketDataProvider::Htx);
    assert_eq!(kline.interval(), "1m");
    assert_eq!(
        kline.open_time(),
        Utc.timestamp_opt(1_710_000_000, 0).unwrap()
    );
    assert_eq!(kline.volume(), &decimal("12.30"));
    assert_eq!(trade.provider(), MarketDataProvider::Htx);
    assert_eq!(trade.trade_id(), "1001");
    assert_eq!(trade.side(), MarketTradeSide::Sell);
    assert_eq!(
        trade.traded_at(),
        Utc.timestamp_millis_opt(1_710_000_003_000).unwrap()
    );
}

#[test]
fn adapter_snapshots_build_cache_entries_and_mongo_kline_upsert() {
    let ticker = BitgetMarketAdapter::ticker_from_ws(
        r#"{"arg":{"channel":"ticker","instId":"ETHUSDT"},"data":[{"lastPr":"3000.00","baseVolume":"50.00","ts":"1710000000000"}]}"#,
    )
    .unwrap();
    let kline = HtxMarketAdapter::kline_from_ws(
        r#"{"ch":"market.ethusdt.kline.5min","ts":1710000300000,"tick":{"id":1710000300,"open":"3000","high":"3010","low":"2990","close":"3005","amount":"88"}}"#,
    )
    .unwrap();

    let ticker_cache = MarketTickerCacheEntry::from_snapshot(&ticker).unwrap();
    let kline_cache = MarketKlineCacheEntry::from_snapshot(&kline).unwrap();
    let mongo_write = MarketKlineMongoWrite::from_snapshot(&kline).unwrap();

    assert_eq!(ticker_cache.redis_key(), "market:ticker:ETHUSDT");
    assert_eq!(ticker_cache.last_price(), &decimal("3000.00"));
    assert_eq!(kline_cache.redis_key(), "market:kline:ETHUSDT:5m");
    assert_eq!(mongo_write.collection_name(), "market_klines_ETHUSDT");
    assert_eq!(
        mongo_write.upsert_filter(),
        doc! { "interval": "5m", "open_time": BsonDateTime::from_millis(kline.open_time().timestamp_millis()) }
    );
    assert_eq!(
        mongo_write.upsert_update(),
        doc! { "$set": {
            "interval": "5m",
            "open_time": BsonDateTime::from_millis(kline.open_time().timestamp_millis()),
            "open": "3000",
            "high": "3010",
            "low": "2990",
            "close": "3005",
            "volume": "88",
            "source": "htx",
            "updated_at": BsonDateTime::from_millis(kline.observed_at().timestamp_millis()),
        }}
    );
}

#[test]
fn adapters_reject_invalid_symbols_intervals_and_payloads() {
    assert!(
        BitgetMarketAdapter::ticker_from_ws(
            r#"{"arg":{"instId":"BTC.USDT"},"data":[{"lastPr":"1","baseVolume":"1","ts":"1710000000000"}]}"#
        )
        .is_err()
    );
    assert!(
        BitgetMarketAdapter::kline_from_ws(
            r#"{"arg":{"channel":"candle2m","instId":"BTCUSDT"},"data":[["1710000000000","1","1","1","1","1"]]}"#
        )
        .is_err()
    );
    assert!(
        HtxMarketAdapter::trade_from_ws(
            r#"{"ch":"market.btcusdt.trade.detail","tick":{"data":[]}}"#
        )
        .is_err()
    );
    assert!(
        BitgetMarketAdapter::trade_from_ws(
            r#"{"arg":{"instId":"BTCUSDT"},"data":[{"tradeId":"bt-1","side":"unknown","price":"1","size":"1","ts":"1710000000000"}]}"#
        )
        .is_err()
    );
}
