use super::{repository, service};
use bigdecimal::BigDecimal;
use chrono::{TimeDelta, TimeZone, Utc};
use std::str::FromStr;

#[test]
fn numeric_safety_seconds_stake_storage_ignores_only_trailing_zeros() {
    for value in [
        "1e-18",
        "99999999999999999999.999999999999999999",
        "1.000000000000000000000",
    ] {
        service::validate_stake_amount(&value.parse().unwrap()).unwrap();
    }
    for value in ["1e20", "1e-19", "1e100000", "1e-4294967297"] {
        assert!(service::validate_stake_amount(&value.parse().unwrap()).is_err());
    }
}

#[test]
fn gross_payout_budget_preserves_net_rate_and_capacity_precision() {
    let decimal = |value: &str| BigDecimal::from_str(value).unwrap();
    assert_eq!(
        service::maximum_gross_payout(&decimal("100"), &decimal("0.4")),
        decimal("140")
    );
    assert_eq!(
        service::maximum_gross_payout(&decimal("100"), &decimal("1.4")),
        decimal("240")
    );
    assert_eq!(
        service::maximum_gross_payout(&decimal("0.000000000000000001"), &decimal("0.4")),
        decimal("0.000000000000000002")
    );
    for value in [None, Some(decimal("0")), Some(decimal("1.2300"))] {
        service::validate_payout_capacity(value.as_ref(), 2).unwrap();
    }
    for value in ["-1", "1.001", "100000000000000000000"] {
        assert!(service::validate_payout_capacity(Some(&decimal(value)), 2).is_err());
    }
}

#[test]
fn manual_review_requires_evidence_based_settlement_mode() {
    assert!(service::ensure_manual_settlement_allowed("manual_review", true).is_ok());
    assert!(service::ensure_manual_settlement_allowed("manual_review", false).is_err());
    assert!(service::ensure_manual_settlement_allowed("opened", false).is_ok());
    assert!(service::ensure_manual_settlement_allowed("settled", true).is_ok());
    assert!(service::ensure_manual_settlement_allowed("refunded", true).is_err());
}

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

    let mut default_source = valid.clone();
    default_source.source = "default".to_owned();
    assert!(
        service::validate_settlement_price_snapshot(&default_source, "BTCUSDT", expires_at).is_ok(),
        "default-generator ticks are archival settlement evidence"
    );

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

#[test]
fn payout_rate_is_net_profit_while_historical_order_snapshots_keep_their_value() {
    let stake = BigDecimal::from(100);
    let configured_net_rate = BigDecimal::from_str("0.4").unwrap();
    let historical_snapshot_rate = BigDecimal::from_str("1.4").unwrap();

    assert_eq!(
        service::seconds_contract_payout_amount(&stake, &configured_net_rate, "win", 18),
        BigDecimal::from(140)
    );
    assert_eq!(
        service::seconds_contract_payout_amount(&stake, &historical_snapshot_rate, "win", 18),
        BigDecimal::from(240)
    );
    assert_eq!(
        service::seconds_contract_payout_amount(&stake, &configured_net_rate, "loss", 18),
        BigDecimal::from(0)
    );
}
