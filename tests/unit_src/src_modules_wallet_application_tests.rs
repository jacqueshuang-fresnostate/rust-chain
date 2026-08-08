use super::{calculate_today_return, normalize_asset_symbol, utc_day_start};
use crate::modules::wallet::{
    infrastructure::TodayReturnAssetActivityRow, presentation::TodayReturnStatus,
};
use bigdecimal::BigDecimal;
use chrono::{TimeZone, Utc};
use std::{collections::BTreeMap, str::FromStr};

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}

fn activity(asset_symbol: &str, amount: &str, basis_amount: &str) -> TodayReturnAssetActivityRow {
    TodayReturnAssetActivityRow {
        asset_symbol: asset_symbol.to_owned(),
        amount: decimal(amount),
        basis_amount: decimal(basis_amount),
    }
}

#[test]
fn normalize_asset_symbol_to_uppercase() {
    assert_eq!(normalize_asset_symbol(" usdt ").unwrap(), "USDT");
}

#[test]
fn normalize_asset_symbol_rejects_invalid_format() {
    assert!(normalize_asset_symbol("BTC-USDT").is_err());
}

#[test]
fn today_return_uses_utc_calendar_day_and_stablecoin_parity() {
    let calculated_at = Utc.with_ymd_and_hms(2026, 8, 9, 16, 30, 45).unwrap();
    let period_start_at = utc_day_start(&calculated_at);
    let response = calculate_today_return(
        vec![
            activity("USDT", "8", "100"),
            activity("usdc", "-3", "50"),
            activity(" usd ", "2", "25"),
        ],
        &BTreeMap::new(),
        period_start_at,
        calculated_at,
    );

    assert_eq!(
        response.period_start_at,
        Utc.with_ymd_and_hms(2026, 8, 9, 0, 0, 0).unwrap()
    );
    assert_eq!(response.scope, "realized");
    assert_eq!(response.reporting_asset, "USDT");
    assert_eq!(response.amount, decimal("7"));
    assert_eq!(response.basis_amount, decimal("175"));
    assert_eq!(response.rate, decimal("0.04"));
    assert_eq!(response.status, TodayReturnStatus::Complete);
    assert!(response.missing_price_assets.is_empty());
}

#[test]
fn today_return_values_non_stable_activity_with_current_ticker_price() {
    let calculated_at = Utc.with_ymd_and_hms(2026, 8, 9, 18, 0, 0).unwrap();
    let prices = BTreeMap::from([("BTC".to_owned(), decimal("50000"))]);
    let response = calculate_today_return(
        vec![activity("BTC", "0.1", "1")],
        &prices,
        utc_day_start(&calculated_at),
        calculated_at,
    );

    assert_eq!(response.amount, decimal("5000"));
    assert_eq!(response.basis_amount, decimal("50000"));
    assert_eq!(response.rate, decimal("0.1"));
    assert_eq!(response.status, TodayReturnStatus::Complete);
}

#[test]
fn today_return_preserves_negative_realized_amount_and_rate() {
    let calculated_at = Utc.with_ymd_and_hms(2026, 8, 9, 18, 0, 0).unwrap();
    let response = calculate_today_return(
        vec![activity("USDT", "-25", "100")],
        &BTreeMap::new(),
        utc_day_start(&calculated_at),
        calculated_at,
    );

    assert_eq!(response.amount, decimal("-25"));
    assert_eq!(response.basis_amount, decimal("100"));
    assert_eq!(response.rate, decimal("-0.25"));
    assert_eq!(response.status, TodayReturnStatus::Complete);
}

#[test]
fn today_return_marks_missing_non_stable_price_as_partial_without_calling_it_zero() {
    let calculated_at = Utc.with_ymd_and_hms(2026, 8, 9, 18, 0, 0).unwrap();
    let response = calculate_today_return(
        vec![activity("USDT", "2", "10"), activity("BTC", "1", "5")],
        &BTreeMap::new(),
        utc_day_start(&calculated_at),
        calculated_at,
    );

    assert_eq!(response.amount, decimal("2"));
    assert_eq!(response.basis_amount, decimal("10"));
    assert_eq!(response.rate, decimal("0.2"));
    assert_eq!(response.status, TodayReturnStatus::Partial);
    assert_eq!(response.missing_price_assets, vec!["BTC"]);
}

#[test]
fn today_return_without_realized_activity_is_complete_true_zero() {
    let calculated_at = Utc.with_ymd_and_hms(2026, 8, 9, 18, 0, 0).unwrap();
    let response = calculate_today_return(
        Vec::new(),
        &BTreeMap::new(),
        utc_day_start(&calculated_at),
        calculated_at,
    );

    assert_eq!(response.amount, decimal("0"));
    assert_eq!(response.basis_amount, decimal("0"));
    assert_eq!(response.rate, decimal("0"));
    assert_eq!(response.status, TodayReturnStatus::Complete);
    assert!(response.missing_price_assets.is_empty());
}

#[test]
fn today_return_serializes_decimal_amounts_and_rate_as_exact_strings() {
    let calculated_at = Utc.with_ymd_and_hms(2026, 8, 9, 18, 0, 0).unwrap();
    let response = calculate_today_return(
        vec![activity("USDT", "1", "3")],
        &BTreeMap::new(),
        utc_day_start(&calculated_at),
        calculated_at,
    );
    let payload = serde_json::to_value(response).unwrap();

    assert_eq!(payload["amount"], "1.000000000000000000");
    assert_eq!(payload["basis_amount"], "3.000000000000000000");
    assert_eq!(payload["rate"], "0.333333333333333333");
    assert_eq!(payload["period_start_at"], 1_786_233_600_000_i64);
    assert_eq!(payload["calculated_at"], 1_786_298_400_000_i64);
}
