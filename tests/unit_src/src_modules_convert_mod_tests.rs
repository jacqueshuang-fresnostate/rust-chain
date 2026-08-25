use super::*;
use bigdecimal::BigDecimal;
use chrono::{TimeDelta, TimeZone, Utc};
use uuid::Uuid;

#[test]
fn convert_pair_response_serializes_configured_and_null_asset_logos() {
    let response = presentation::ConvertPairsResponse {
        pairs: vec![
            presentation::ConvertPairResponse {
                id: 7,
                from_asset_id: 11,
                from_asset_symbol: "BTC".to_owned(),
                from_asset_logo_url: Some("https://cdn.example.test/assets/btc.png".to_owned()),
                to_asset_id: 12,
                to_asset_symbol: "USDT".to_owned(),
                to_asset_logo_url: Some("/uploads/assets/usdt.svg".to_owned()),
                pricing_mode: "fixed".to_owned(),
                spread_rate: BigDecimal::from(0),
                fee_rate: BigDecimal::from(0),
                min_amount: BigDecimal::from(1),
                max_amount: None,
                target_min_amount: BigDecimal::from(1),
                target_max_amount: None,
                enabled: true,
            },
            presentation::ConvertPairResponse {
                id: 8,
                from_asset_id: 13,
                from_asset_symbol: "ETH".to_owned(),
                from_asset_logo_url: None,
                to_asset_id: 14,
                to_asset_symbol: "USDC".to_owned(),
                to_asset_logo_url: None,
                pricing_mode: "market".to_owned(),
                spread_rate: BigDecimal::from(0),
                fee_rate: BigDecimal::from(0),
                min_amount: BigDecimal::from(1),
                max_amount: None,
                target_min_amount: BigDecimal::from(1),
                target_max_amount: None,
                enabled: true,
            },
        ],
    };

    let payload = serde_json::to_value(response).unwrap();
    let configured_pair = &payload["pairs"][0];
    let null_pair = &payload["pairs"][1];

    assert_eq!(
        configured_pair["from_asset_logo_url"],
        "https://cdn.example.test/assets/btc.png"
    );
    assert_eq!(
        configured_pair["to_asset_logo_url"],
        "/uploads/assets/usdt.svg"
    );
    assert_eq!(configured_pair["from_asset_symbol"], "BTC");
    assert_eq!(configured_pair["to_asset_symbol"], "USDT");

    assert!(null_pair.get("from_asset_logo_url").is_some());
    assert!(null_pair["from_asset_logo_url"].is_null());
    assert!(null_pair.get("to_asset_logo_url").is_some());
    assert!(null_pair["to_asset_logo_url"].is_null());
    assert_eq!(null_pair["from_asset_symbol"], "ETH");
    assert_eq!(null_pair["to_asset_symbol"], "USDC");
}

#[test]
fn quote_ttl_accepts_before_expiry_and_rejects_at_expiry() {
    let quote_id = QuoteId(Uuid::nil());
    let now = Utc.with_ymd_and_hms(2026, 5, 26, 9, 0, 0).unwrap();
    let quote = ConvertQuote::new(quote_id.clone(), now, 10).unwrap();

    assert_eq!(quote.quote_id(), &quote_id);
    assert_eq!(
        quote.idempotency_key(),
        "convert:quote:00000000-0000-0000-0000-000000000000"
    );
    assert_eq!(quote.ttl().expires_at, now + TimeDelta::seconds(10));
    assert_eq!(
        quote.ensure_not_expired(now + TimeDelta::seconds(9)),
        Ok(())
    );
    assert_eq!(
        quote.ensure_not_expired(now + TimeDelta::seconds(10)),
        Err(ConvertQuoteError::Expired)
    );
}

#[test]
fn quote_ttl_requires_positive_ttl() {
    let now = Utc.with_ymd_and_hms(2026, 5, 26, 9, 0, 0).unwrap();

    assert_eq!(
        ConvertQuote::new(QuoteId(Uuid::nil()), now, 0).unwrap_err(),
        ConvertQuoteError::InvalidTtl
    );
}

#[test]
fn authoritative_market_price_validation_fails_closed_on_bad_or_stale_ticks() {
    let now = Utc.with_ymd_and_hms(2026, 8, 24, 12, 0, 0).unwrap();
    let valid = repository::ConvertMarketPriceSnapshot {
        price: BigDecimal::from(100),
        source: "bitget".to_owned(),
        symbol: "BTCUSDT".to_owned(),
        observed_at: now - TimeDelta::seconds(60),
        source_version: "event-v1".to_owned(),
    };
    assert!(service::validate_convert_market_price_snapshot(&valid, "BTC-USDT", now, 60).is_ok());

    let mut invalid = valid.clone();
    invalid.observed_at = now + TimeDelta::microseconds(1);
    assert!(service::validate_convert_market_price_snapshot(&invalid, "BTCUSDT", now, 60).is_err());

    invalid = valid.clone();
    invalid.observed_at = now - TimeDelta::seconds(60) - TimeDelta::microseconds(1);
    assert!(service::validate_convert_market_price_snapshot(&invalid, "BTCUSDT", now, 60).is_err());

    invalid = valid.clone();
    invalid.price = BigDecimal::from(0);
    assert!(service::validate_convert_market_price_snapshot(&invalid, "BTCUSDT", now, 60).is_err());

    invalid = valid.clone();
    invalid.source_version.clear();
    assert!(service::validate_convert_market_price_snapshot(&invalid, "BTCUSDT", now, 60).is_err());

    invalid = valid.clone();
    invalid.source = "unknown".to_owned();
    assert!(service::validate_convert_market_price_snapshot(&invalid, "BTCUSDT", now, 60).is_err());

    invalid = valid;
    invalid.symbol = "ETHUSDT".to_owned();
    assert!(service::validate_convert_market_price_snapshot(&invalid, "BTCUSDT", now, 60).is_err());

    let underscored = repository::ConvertMarketPriceSnapshot {
        symbol: "BTC_USDT".to_owned(),
        ..repository::ConvertMarketPriceSnapshot {
            price: BigDecimal::from(100),
            source: "bitget".to_owned(),
            symbol: "BTCUSDT".to_owned(),
            observed_at: now,
            source_version: "event-v2".to_owned(),
        }
    };
    assert!(
        service::validate_convert_market_price_snapshot(&underscored, "BTC-USDT", now, 60).is_ok()
    );
}
