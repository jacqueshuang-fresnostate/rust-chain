use super::{
    WalletLedgerCategory, WalletLedgerEntryRow, WalletLedgerFilter,
    classify_wallet_ledger_change_type, push_wallet_ledger_filters,
    return_history_historical_close_if_valid, return_history_kline_document_close_if_valid,
    today_return_ticker_price_if_current, wallet_ledger_entry_response,
};
use bigdecimal::BigDecimal;
use chrono::{TimeDelta, TimeZone, Utc};
use mongodb::bson::{DateTime as BsonDateTime, doc};
use serde_json::json;
use sqlx::{MySql, QueryBuilder};
use std::{collections::BTreeSet, str::FromStr};

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}

fn ticker_payload(symbol: &str, price: &str, observed_at: chrono::DateTime<Utc>) -> String {
    json!({
        "symbol": symbol,
        "last_price": price,
        "observed_at": observed_at.timestamp_millis(),
    })
    .to_string()
}

fn wallet_ledger_filter(category: WalletLedgerCategory) -> WalletLedgerFilter {
    WalletLedgerFilter {
        asset_id: Some(42),
        asset_symbol: Some("USDT".to_owned()),
        change_type: Some("spot_trade_settlement".to_owned()),
        category: Some(category),
        ref_type: Some("spot_trade".to_owned()),
        ref_id: Some("100:200".to_owned()),
        start_time: Some("2026-08-01T00:00:00Z".to_owned()),
        end_time: Some("2026-08-10T00:00:00Z".to_owned()),
        limit: 50,
        offset: 0,
    }
}

#[test]
fn wallet_ledger_classifier_covers_every_category_and_defaults_unknown_to_other() {
    for (change_type, expected) in [
        ("deposit", WalletLedgerCategory::Funding),
        ("deposit_confirm", WalletLedgerCategory::Funding),
        ("withdrawal_release", WalletLedgerCategory::Funding),
        ("admin_recharge", WalletLedgerCategory::Funding),
        ("quick_recharge", WalletLedgerCategory::Funding),
        ("spot_trade_settlement", WalletLedgerCategory::Spot),
        ("margin_position_close", WalletLedgerCategory::Margin),
        ("seconds_contract_settle_win", WalletLedgerCategory::Seconds),
        ("convert_settlement", WalletLedgerCategory::Convert),
        ("earn_redeem", WalletLedgerCategory::Earn),
        ("new_coin_purchase_payment", WalletLedgerCategory::NewCoin),
        ("loan_repayment", WalletLedgerCategory::Loan),
        ("prediction_payout", WalletLedgerCategory::Prediction),
        ("agent_commission_payout", WalletLedgerCategory::Other),
        ("future_wallet_adjustment", WalletLedgerCategory::Other),
        ("spot", WalletLedgerCategory::Other),
        ("SPOT_trade", WalletLedgerCategory::Other),
    ] {
        assert_eq!(
            classify_wallet_ledger_change_type(change_type),
            expected,
            "change_type: {change_type}"
        );
    }
}

#[test]
fn wallet_ledger_filter_sql_combines_category_with_existing_exact_predicates() {
    let filter = wallet_ledger_filter(WalletLedgerCategory::Spot);
    let mut rows = QueryBuilder::<MySql>::new("WHERE wl.user_id = ?");
    push_wallet_ledger_filters(&mut rows, &filter);
    let mut count = QueryBuilder::<MySql>::new("WHERE wl.user_id = ?");
    push_wallet_ledger_filters(&mut count, &filter);

    assert_eq!(rows.sql(), count.sql());
    for predicate in [
        "wl.asset_id = ?",
        "UPPER(a.symbol) = ?",
        " AND wl.change_type = ?",
        "LEFT(BINARY wl.change_type, ?) = ?",
        "wl.ref_type = ?",
        "wl.ref_id = ?",
        "wl.created_at >= ?",
        "wl.created_at <= ?",
    ] {
        assert!(rows.sql().contains(predicate), "missing: {predicate}");
    }

    let other_filter = wallet_ledger_filter(WalletLedgerCategory::Other);
    let mut other = QueryBuilder::<MySql>::new("WHERE wl.user_id = ?");
    push_wallet_ledger_filters(&mut other, &other_filter);
    assert!(other.sql().contains(" AND NOT ("));
    assert!(other.sql().contains("BINARY wl.change_type = ?"));
    assert!(other.sql().contains("LEFT(BINARY wl.change_type, ?) = ?"));
}

#[test]
fn wallet_ledger_entry_response_includes_authoritative_category_and_exact_decimals() {
    let response = wallet_ledger_entry_response(WalletLedgerEntryRow {
        id: 1,
        user_id: 2,
        asset_id: 3,
        symbol: "USDT".to_owned(),
        change_type: "future_wallet_adjustment".to_owned(),
        amount: decimal("0.123456789012345678"),
        balance_type: "available".to_owned(),
        balance_after: decimal("10.123456789012345678"),
        available_after: decimal("10.123456789012345678"),
        frozen_after: decimal("0.000000000000000000"),
        locked_after: decimal("0.000000000000000000"),
        fee: decimal("0.010000000000000000"),
        ref_type: "wallet_route_fixture".to_owned(),
        ref_id: "fixture-1".to_owned(),
        created_at: Utc.with_ymd_and_hms(2026, 8, 10, 1, 2, 3).unwrap(),
    });
    let payload = serde_json::to_value(response).unwrap();

    assert_eq!(payload["change_type"], "future_wallet_adjustment");
    assert_eq!(payload["category"], "other");
    assert_eq!(payload["amount"], "0.123456789012345678");
    assert_eq!(payload["balance_after"], "10.123456789012345678");
    assert_eq!(payload["fee"], "0.010000000000000000");
}

#[test]
fn today_return_ticker_requires_matching_positive_fresh_payload() {
    let calculated_at = Utc.with_ymd_and_hms(2026, 8, 9, 18, 0, 0).unwrap();
    let fresh = ticker_payload(
        "BTCUSDT",
        "50000.125000000000000000",
        calculated_at - TimeDelta::seconds(60),
    );

    assert_eq!(
        today_return_ticker_price_if_current("btc", &fresh, calculated_at),
        Some(decimal("50000.125000000000000000"))
    );

    for payload in [
        ticker_payload("BTCUSDT", "50000", calculated_at - TimeDelta::seconds(61)),
        ticker_payload(
            "BTCUSDT",
            "50000",
            calculated_at + TimeDelta::milliseconds(1),
        ),
        ticker_payload("ETHUSDT", "50000", calculated_at),
        ticker_payload("BTCUSDT", "0", calculated_at),
        ticker_payload("BTCUSDT", "-1", calculated_at),
        r#"{"symbol":"BTCUSDT","last_price":"broken","observed_at":0}"#.to_owned(),
        r#"{"symbol":"BTCUSDT","last_price":"50000"}"#.to_owned(),
    ] {
        assert_eq!(
            today_return_ticker_price_if_current("BTC", &payload, calculated_at),
            None,
            "payload must not be called current: {payload}"
        );
    }
}

#[test]
fn return_history_close_requires_requested_utc_day_and_positive_decimal() {
    let requested_day = Utc
        .with_ymd_and_hms(2026, 8, 8, 0, 0, 0)
        .unwrap()
        .date_naive();
    let requested_days = BTreeSet::from([requested_day]);
    let open_time = requested_day.and_hms_opt(0, 0, 0).unwrap().and_utc();

    assert_eq!(
        return_history_historical_close_if_valid(
            open_time.timestamp_millis(),
            "50000.125000000000000000",
            &requested_days,
        ),
        Some((requested_day, decimal("50000.125000000000000000")))
    );

    for (millis, close) in [
        (open_time.timestamp_millis() + 1, "50000"),
        ((open_time + TimeDelta::days(1)).timestamp_millis(), "50000"),
        (open_time.timestamp_millis(), "0"),
        (open_time.timestamp_millis(), "-1"),
        (open_time.timestamp_millis(), "broken"),
    ] {
        assert_eq!(
            return_history_historical_close_if_valid(millis, close, &requested_days),
            None
        );
    }
}

#[test]
fn return_history_malformed_kline_document_becomes_missing_price() {
    let requested_day = Utc
        .with_ymd_and_hms(2026, 8, 8, 0, 0, 0)
        .unwrap()
        .date_naive();
    let requested_days = BTreeSet::from([requested_day]);
    let open_time = BsonDateTime::from_millis(
        requested_day
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp_millis(),
    );

    assert_eq!(
        return_history_kline_document_close_if_valid(
            &doc! { "open_time": open_time, "close": "50000" },
            &requested_days,
        ),
        Some((requested_day, decimal("50000")))
    );
    for document in [
        doc! { "open_time": open_time, "close": 50000_i64 },
        doc! { "open_time": "broken", "close": "50000" },
        doc! { "open_time": open_time },
    ] {
        assert_eq!(
            return_history_kline_document_close_if_valid(&document, &requested_days),
            None
        );
    }
}
