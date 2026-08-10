use super::{
    calculate_return_history, calculate_today_return, normalize_asset_symbol, utc_day_start,
    validate_return_history_days,
};
use crate::modules::wallet::{
    infrastructure::{ReturnHistoryAssetActivityRow, TodayReturnAssetActivityRow},
    presentation::TodayReturnStatus,
};
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, TimeDelta, TimeZone, Utc};
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

fn history_activity(
    activity_day: NaiveDate,
    asset_symbol: &str,
    amount: &str,
    basis_amount: &str,
) -> ReturnHistoryAssetActivityRow {
    ReturnHistoryAssetActivityRow {
        activity_day,
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

#[test]
fn return_history_days_are_a_strict_whitelist() {
    for days in [1, 7, 30, 180] {
        assert_eq!(validate_return_history_days(Some(days)).unwrap(), days);
    }
    for days in [None, Some(0), Some(2), Some(181)] {
        assert!(validate_return_history_days(days).is_err());
    }
}

#[test]
fn return_history_fills_exact_utc_days_and_accumulates_quantized_daily_returns() {
    let calculated_at = Utc.with_ymd_and_hms(2026, 8, 9, 18, 0, 0).unwrap();
    let period_start_at = utc_day_start(&calculated_at) - TimeDelta::days(6);
    let response = calculate_return_history(
        vec![
            history_activity(
                period_start_at.date_naive() + TimeDelta::days(1),
                "USDT",
                "1",
                "4",
            ),
            history_activity(
                period_start_at.date_naive() + TimeDelta::days(3),
                "USDC",
                "-2",
                "10",
            ),
            history_activity(
                period_start_at.date_naive() + TimeDelta::days(6),
                "USD",
                "3",
                "6",
            ),
        ],
        &BTreeMap::new(),
        &BTreeMap::new(),
        7,
        period_start_at,
        calculated_at,
    );

    assert_eq!(response.status, TodayReturnStatus::Complete);
    assert_eq!(response.points.len(), 7);
    for (index, point) in response.points.iter().enumerate() {
        assert_eq!(
            point.day_start_at,
            period_start_at + TimeDelta::days(index as i64)
        );
        assert_eq!(
            point.valued_at,
            if index == 6 {
                calculated_at
            } else {
                point.day_start_at + TimeDelta::days(1)
            }
        );
    }
    assert_eq!(
        response
            .points
            .iter()
            .map(|point| point.cumulative_amount.clone().unwrap())
            .collect::<Vec<_>>(),
        ["0", "1", "1", "-1", "-1", "-1", "2"]
            .into_iter()
            .map(decimal)
            .collect::<Vec<_>>()
    );
    assert_eq!(response.summary.amount, Some(decimal("2")));
    assert_eq!(response.summary.basis_amount, Some(decimal("20")));
    assert_eq!(response.summary.rate, Some(decimal("0.1")));
    assert!(response.missing_prices.is_empty());
}

#[test]
fn return_history_uses_past_close_and_current_ticker_then_propagates_unknown_cumulative() {
    let calculated_at = Utc.with_ymd_and_hms(2026, 8, 9, 18, 0, 0).unwrap();
    let period_start_at = utc_day_start(&calculated_at) - TimeDelta::days(2);
    let first_day = period_start_at.date_naive();
    let missing_day = first_day + TimeDelta::days(1);
    let today = first_day + TimeDelta::days(2);
    let response = calculate_return_history(
        vec![
            history_activity(first_day, "BTC", "0.1", "1"),
            history_activity(missing_day, "ETH", "1", "2"),
            history_activity(today, "BTC", "0.2", "1"),
        ],
        &BTreeMap::from([((first_day, "BTC".to_owned()), decimal("50000"))]),
        &BTreeMap::from([("BTC".to_owned(), decimal("60000"))]),
        3,
        period_start_at,
        calculated_at,
    );

    assert_eq!(response.status, TodayReturnStatus::Partial);
    assert_eq!(response.summary.amount, None);
    assert_eq!(response.summary.basis_amount, None);
    assert_eq!(response.summary.rate, None);
    assert_eq!(response.points[0].amount, Some(decimal("5000")));
    assert_eq!(response.points[0].cumulative_amount, Some(decimal("5000")));
    assert_eq!(response.points[1].status, TodayReturnStatus::Partial);
    assert_eq!(response.points[1].amount, None);
    assert_eq!(response.points[1].basis_amount, None);
    assert_eq!(response.points[1].rate, None);
    assert_eq!(response.points[1].cumulative_amount, None);
    assert_eq!(response.points[1].missing_price_assets, vec!["ETH"]);
    assert_eq!(response.points[2].status, TodayReturnStatus::Complete);
    assert_eq!(response.points[2].amount, Some(decimal("12000")));
    assert_eq!(response.points[2].cumulative_amount, None);
    assert_eq!(response.missing_prices.len(), 1);
    assert_eq!(
        response.missing_prices[0].day_start_at.date_naive(),
        missing_day
    );
    assert_eq!(response.missing_prices[0].asset_symbol, "ETH");
}

#[test]
fn return_history_no_activity_returns_exact_complete_zero_points_and_strings() {
    let calculated_at = Utc.with_ymd_and_hms(2026, 8, 9, 18, 0, 0).unwrap();
    let period_start_at = utc_day_start(&calculated_at);
    let response = calculate_return_history(
        Vec::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        1,
        period_start_at,
        calculated_at,
    );
    let payload = serde_json::to_value(response).unwrap();

    assert_eq!(payload["status"], "complete");
    assert_eq!(payload["period_days"], 1);
    assert_eq!(payload["points"].as_array().unwrap().len(), 1);
    assert_eq!(payload["points"][0]["amount"], "0.000000000000000000");
    assert_eq!(payload["points"][0]["basis_amount"], "0.000000000000000000");
    assert_eq!(payload["points"][0]["rate"], "0.000000000000000000");
    assert_eq!(
        payload["points"][0]["cumulative_amount"],
        "0.000000000000000000"
    );
    assert_eq!(payload["summary"]["amount"], "0.000000000000000000");
    assert_eq!(payload["summary"]["basis_amount"], "0.000000000000000000");
    assert_eq!(payload["summary"]["rate"], "0.000000000000000000");
}
