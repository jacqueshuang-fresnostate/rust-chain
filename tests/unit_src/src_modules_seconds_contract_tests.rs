use super::{repository, service};
use bigdecimal::BigDecimal;
use chrono::{TimeDelta, TimeZone, Utc};

fn snapshot(expires_at: chrono::DateTime<Utc>) -> repository::SecondsContractSettlementPriceRow {
    repository::SecondsContractSettlementPriceRow {
        id: 1,
        symbol: "BTCUSDT".to_owned(),
        price: BigDecimal::from(100),
        source: "bitget".to_owned(),
        observed_at: expires_at,
        generation: 1,
        source_version: "event-v1".to_owned(),
    }
}

#[test]
fn settlement_snapshot_validates_all_provenance_and_window_boundaries() {
    let expires_at = Utc.with_ymd_and_hms(2026, 8, 24, 12, 0, 0).unwrap();
    let valid = snapshot(expires_at);
    assert!(service::validate_settlement_price_snapshot(&valid, "btc-usdt", expires_at).is_ok());

    let mut invalid = valid.clone();
    invalid.symbol = "ETHUSDT".to_owned();
    assert!(service::validate_settlement_price_snapshot(&invalid, "BTCUSDT", expires_at).is_err());

    invalid = valid.clone();
    invalid.source = "unknown".to_owned();
    assert!(service::validate_settlement_price_snapshot(&invalid, "BTCUSDT", expires_at).is_err());

    invalid = valid.clone();
    invalid.generation = 0;
    assert!(service::validate_settlement_price_snapshot(&invalid, "BTCUSDT", expires_at).is_err());

    invalid = valid.clone();
    invalid.source_version.clear();
    assert!(service::validate_settlement_price_snapshot(&invalid, "BTCUSDT", expires_at).is_err());

    invalid = valid.clone();
    invalid.observed_at = expires_at - TimeDelta::microseconds(1);
    assert!(service::validate_settlement_price_snapshot(&invalid, "BTCUSDT", expires_at).is_err());

    invalid = valid;
    invalid.observed_at = expires_at + TimeDelta::seconds(5);
    assert!(service::validate_settlement_price_snapshot(&invalid, "BTCUSDT", expires_at).is_err());
}
