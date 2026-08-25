use super::service::{
    calculate_interest_amount, calculate_loan_ltv, ensure_loan_ltv_within_initial,
    loan_order_request_fingerprint, loan_risk_state, validate_loan_ltv_thresholds,
};
use super::{
    domain::{INTEREST_MODE_ACTUAL_DAYS, INTEREST_MODE_FULL_TERM},
    *,
};
use bigdecimal::BigDecimal;
use chrono::{TimeDelta, Utc};
use serde_json::json;
use std::str::FromStr;

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).expect("valid decimal")
}

#[test]
fn full_term_interest_is_truncated_to_asset_precision() {
    let interest = calculate_interest_amount(
        &decimal("100"),
        &decimal("0.12345678"),
        INTEREST_MODE_FULL_TERM,
        30,
        Utc::now(),
        Utc::now(),
        4,
    )
    .expect("interest amount");

    assert_eq!(interest, decimal("12.3456"));
}

#[test]
fn actual_days_interest_charges_at_least_one_day_and_clamps_to_term() {
    let disbursed_at = Utc::now();
    let one_day_interest = calculate_interest_amount(
        &decimal("100"),
        &decimal("0.30"),
        INTEREST_MODE_ACTUAL_DAYS,
        30,
        disbursed_at,
        disbursed_at,
        2,
    )
    .expect("one day interest amount");
    let full_term_interest = calculate_interest_amount(
        &decimal("100"),
        &decimal("0.30"),
        INTEREST_MODE_ACTUAL_DAYS,
        30,
        disbursed_at,
        disbursed_at + TimeDelta::days(45),
        2,
    )
    .expect("full term interest amount");

    assert_eq!(one_day_interest, decimal("1.00"));
    assert_eq!(full_term_interest, decimal("30.00"));
}

#[test]
fn interest_fails_closed_for_invalid_term_or_asset_precision_metadata() {
    let now = Utc::now();
    calculate_interest_amount(
        &decimal("100"),
        &decimal("0.1"),
        INTEREST_MODE_ACTUAL_DAYS,
        0,
        now,
        now,
        18,
    )
    .expect_err("zero term must not reach division");
    let precision_error = calculate_interest_amount(
        &decimal("100"),
        &decimal("0.1"),
        INTEREST_MODE_FULL_TERM,
        30,
        now,
        now,
        19,
    )
    .expect_err("unsupported database precision must fail closed");
    assert!(format!("{precision_error:?}").contains("between 0 and 18"));
    calculate_interest_amount(
        &decimal("100"),
        &decimal("0.123456789"),
        INTEREST_MODE_FULL_TERM,
        30,
        now,
        now,
        18,
    )
    .expect_err("interest rate must fit its DECIMAL(18,8) snapshot");
    super::service::ensure_amount_precision(&decimal("1"), -1, "amount")
        .expect_err("negative asset precision metadata must fail closed");
}

#[test]
fn default_product_name_json_uses_chinese_locale() {
    let name_json = normalized_product_name_json(None, "信用贷").expect("default name json");

    assert_eq!(name_json["version"], json!(1));
    assert_eq!(name_json["default_locale"], json!("zh-CN"));
    assert_eq!(name_json["items"][0]["country"], json!("CN"));
    assert_eq!(name_json["items"][0]["title"], json!("信用贷"));
    assert_eq!(product_default_name(&name_json), Some("信用贷".to_owned()));
}

#[test]
fn product_name_json_requires_default_locale_item() {
    let name_json = json!({
        "version": 1,
        "default_locale": "zh-CN",
        "items": [
            { "locale": "en-US", "country": "US", "title": "Loan" }
        ]
    });

    let error =
        super::service::validate_product_name_json(&name_json).expect_err("missing default locale");
    assert!(format!("{error:?}").contains("default_locale must exist"));
}

#[test]
fn loan_product_reason_is_required_trimmed_and_bounded() {
    assert_eq!(
        required_product_reason(Some("  调整贷款额度  ".to_owned())).expect("trimmed reason"),
        "调整贷款额度"
    );
    for reason in [None, Some(String::new()), Some("   ".to_owned())] {
        let error = required_product_reason(reason).expect_err("blank reason must fail");
        assert!(format!("{error:?}").contains("reason is required"));
    }

    let error = required_product_reason(Some("变".repeat(513)))
        .expect_err("reason longer than audit column must fail");
    assert!(format!("{error:?}").contains("reason is too long"));
}

#[test]
fn loan_product_revision_must_be_present_and_positive() {
    assert_eq!(required_product_revision(Some(7)).expect("revision"), 7);
    for revision in [None, Some(0)] {
        let error = required_product_revision(revision).expect_err("missing revision must fail");
        assert!(format!("{error:?}").contains("revision is required"));
    }
}

#[test]
fn collateralized_loan_ltv_thresholds_are_strictly_ordered() {
    let thresholds = validate_loan_ltv_thresholds(
        domain::LOAN_TYPE_COLLATERALIZED,
        Some(decimal("0.50")),
        Some(decimal("0.70")),
        Some(decimal("0.85")),
    )
    .expect("valid thresholds")
    .expect("collateralized thresholds");
    assert_eq!(thresholds.0, decimal("0.50"));

    for values in [
        ("0", "0.70", "0.85"),
        ("0.70", "0.70", "0.85"),
        ("0.50", "0.90", "0.85"),
        ("0.50", "0.70", "1.01"),
    ] {
        validate_loan_ltv_thresholds(
            domain::LOAN_TYPE_COLLATERALIZED,
            Some(decimal(values.0)),
            Some(decimal(values.1)),
            Some(decimal(values.2)),
        )
        .expect_err("invalid LTV ordering must fail");
    }
    validate_loan_ltv_thresholds(domain::LOAN_TYPE_CREDIT, Some(decimal("0.50")), None, None)
        .expect_err("credit products cannot configure LTV");
    validate_loan_ltv_thresholds(
        domain::LOAN_TYPE_COLLATERALIZED,
        Some(decimal("0.500000001")),
        Some(decimal("0.70")),
        Some(decimal("0.85")),
    )
    .expect_err("LTV thresholds must fit DECIMAL(18,8) exactly");
}

#[test]
fn loan_ltv_uses_conservative_rounding_and_exact_initial_boundary() {
    let ltv =
        calculate_loan_ltv(&decimal("1"), &decimal("3"), &decimal("1")).expect("LTV calculation");
    assert_eq!(ltv, decimal("0.333333333333333334"));
    ensure_loan_ltv_within_initial(
        &decimal("50"),
        &decimal("100"),
        &decimal("1"),
        &decimal("0.5"),
    )
    .expect("exact initial LTV boundary is allowed");
    ensure_loan_ltv_within_initial(
        &decimal("50.000000000000000001"),
        &decimal("100"),
        &decimal("1"),
        &decimal("0.5"),
    )
    .expect_err("any amount above the initial boundary must fail");
    assert_eq!(
        loan_risk_state(&decimal("0.85"), &decimal("0.70"), &decimal("0.85")),
        domain::LOAN_RISK_STATE_LIQUIDATABLE
    );
}

#[test]
fn loan_order_fingerprint_is_decimal_canonical_and_parameter_bound() {
    let first =
        loan_order_request_fingerprint(1, 2, &decimal("50.00"), Some(3), Some(&decimal("100.0")));
    let same = loan_order_request_fingerprint(1, 2, &decimal("50"), Some(3), Some(&decimal("100")));
    let changed =
        loan_order_request_fingerprint(1, 2, &decimal("51"), Some(3), Some(&decimal("100")));
    assert_eq!(first, same);
    assert_ne!(first, changed);
}

#[test]
fn liquidation_rounds_collateral_up_without_creating_false_bad_debt() {
    let settlement = super::liquidation::calculate_loan_collateral_settlement_amounts(
        &decimal("100"),
        &decimal("0.55"),
        &decimal("51.00"),
        2,
        2,
    )
    .expect("balanced collateral settlement");

    assert_eq!(settlement.collateral_seized, decimal("92.73"));
    assert_eq!(settlement.collateral_returned, decimal("7.27"));
    assert_eq!(settlement.recovered_amount, decimal("51.00"));
    assert_eq!(settlement.bad_debt_amount, decimal("0.00"));

    let dust_surplus = super::liquidation::calculate_loan_collateral_settlement_amounts(
        &decimal("100"),
        &decimal("0.51009"),
        &decimal("51.00"),
        2,
        2,
    )
    .expect("exact collateral value above debt must return representable surplus");
    assert_eq!(dust_surplus.collateral_seized, decimal("99.99"));
    assert_eq!(dust_surplus.collateral_returned, decimal("0.01"));
    assert_eq!(dust_surplus.recovered_amount, decimal("51.00"));
    assert_eq!(dust_surplus.bad_debt_amount, decimal("0.00"));
}

#[test]
fn loan_oracle_rejects_stale_future_and_wrong_symbol_payloads() {
    let now = Utc::now();
    let payload = |symbol: &str, observed_at: chrono::DateTime<Utc>| {
        json!({
            "symbol": symbol,
            "last_price": "1.25",
            "observed_at": observed_at.timestamp_millis(),
        })
        .to_string()
    };

    let fresh = super::oracle::validate_loan_ticker_payload(
        &payload("BTCUSDT", now - TimeDelta::seconds(5)),
        "BTCUSDT",
        domain::LOAN_ORACLE_SOURCE_MARKET_TICKER_REDIS,
        30,
        now,
    )
    .expect("fresh ticker");
    assert_eq!(fresh.price, decimal("1.25"));

    let high_precision = json!({
        "symbol": "BTCUSDT",
        "last_price": "1.1234567890123456789",
        "observed_at": now.timestamp_millis(),
    })
    .to_string();
    let quantized = super::oracle::validate_loan_ticker_payload(
        &high_precision,
        "BTCUSDT",
        domain::LOAN_ORACLE_SOURCE_MARKET_TICKER_REDIS,
        30,
        now,
    )
    .expect("oracle price is conservatively quantized to its persisted precision");
    assert_eq!(quantized.price, decimal("1.123456789012345678"));

    let sub_precision = json!({
        "symbol": "BTCUSDT",
        "last_price": "0.0000000000000000009",
        "observed_at": now.timestamp_millis(),
    })
    .to_string();
    super::oracle::validate_loan_ticker_payload(
        &sub_precision,
        "BTCUSDT",
        domain::LOAN_ORACLE_SOURCE_MARKET_TICKER_REDIS,
        30,
        now,
    )
    .expect_err("a positive ticker that cannot be persisted must fail closed");

    for invalid in [
        payload("BTCUSDT", now - TimeDelta::seconds(31)),
        payload("BTCUSDT", now + TimeDelta::seconds(6)),
        payload("ETHUSDT", now),
    ] {
        super::oracle::validate_loan_ticker_payload(
            &invalid,
            "BTCUSDT",
            domain::LOAN_ORACLE_SOURCE_MARKET_TICKER_REDIS,
            30,
            now,
        )
        .expect_err("invalid ticker must fail closed");
    }
}
