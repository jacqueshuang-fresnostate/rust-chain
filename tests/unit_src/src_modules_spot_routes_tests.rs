use super::*;
use crate::{
    config::Settings,
    error::AppError,
    modules::{
        auth::{TokenScope, issue_token},
        spot::{
            NewOrder, OrderSide, OrderStatus, OrderType, SpotOrder,
            application::build_create_spot_order as build_create_spot_order_use_case,
            application::route_limit as spot_route_limit,
            service::{
                ensure_market_price_within_reference, is_triggerable_stop_limit_buy_order,
                is_triggerable_stop_limit_sell_order, market_buy_reservation_price,
                spot_fill_order_lock_keys, spot_fill_wallet_lock_keys, spot_order_reservation,
            },
        },
    },
};
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use bigdecimal::BigDecimal;
use secrecy::SecretString;
use serde_json::Value;
use std::str::FromStr;
use tower::ServiceExt;

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}

fn test_state() -> AppState {
    AppState::new(Settings {
        app_env: "test".to_owned(),
        app_host: "127.0.0.1".parse().unwrap(),
        app_port: 0,
        database_url: SecretString::new("mysql://test:test@localhost/test".to_owned()),
        mongodb_uri: SecretString::new("mongodb://localhost:27017".to_owned()),
        mongodb_database: "exchange_test".to_owned(),
        redis_url: SecretString::new("redis://localhost:6379".to_owned()),
        rabbitmq_url: SecretString::new("amqp://guest:guest@localhost:5672/%2f".to_owned()),
        jwt_secret: SecretString::new("test-secret".to_owned()),
        credential_encryption_key: Some(SecretString::new(
            "0123456789abcdef0123456789abcdef".to_owned(),
        )),
        jwt_access_ttl_seconds: 900,
        jwt_refresh_ttl_seconds: 2_592_000,
        bitget_rest_base_url: "https://bitget.test".to_owned(),
        bitget_ws_url: "wss://bitget.test/ws".to_owned(),
        htx_rest_base_url: "https://htx.test".to_owned(),
        htx_ws_url: "wss://htx.test/ws".to_owned(),
        coinbase_rest_base_url: "https://coinbase.test".to_owned(),
        coinbase_ws_url: "wss://coinbase.test/ws".to_owned(),
        market_feed_symbols: Vec::new(),
        market_feed_intervals: Vec::new(),
        market_feed_providers: Vec::new(),
        market_feed_reconnect_seconds: 5,
        market_feed_rest_fallback_timeout_seconds: 3,
        event_inbox_retry_scan_seconds: 10,
        event_outbox_publisher_enabled: true,
        event_outbox_publisher_interval_seconds: 5,
        unlock_scanner_enabled: true,
        unlock_scanner_interval_seconds: 10,
        unlock_scanner_batch_limit: 100,
        kline_recovery_enabled: true,
        kline_recovery_interval_seconds: 30,
        kline_recovery_batch_limit: 100,
        seconds_contract_settlement_enabled: true,
        seconds_contract_settlement_interval_seconds: 5,
        seconds_contract_settlement_batch_limit: 100,
        earn_auto_redemption_enabled: true,
        earn_auto_redemption_interval_seconds: 60,
        earn_auto_redemption_batch_limit: 100,
        margin_liquidation_enabled: true,
        margin_liquidation_interval_seconds: 5,
        margin_liquidation_batch_limit: 100,
        margin_interest_enabled: true,
        margin_interest_interval_seconds: 60,
        margin_interest_batch_limit: 100,
    })
}

fn bearer_token(state: &AppState) -> String {
    issue_token(&state.settings, "user:42", TokenScope::User, 900).unwrap()
}

#[tokio::test]
async fn spot_orders_route_requires_user_auth() {
    let app = routes().with_state(test_state());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/spot/orders")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn spot_orders_route_returns_clear_error_without_mysql() {
    let state = test_state();
    let token = bearer_token(&state);
    let app = routes().with_state(state);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/spot/orders")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["code"], "INTERNAL_ERROR");
    assert_eq!(
        payload["message"],
        "internal error: mysql pool is not configured for spot routes"
    );
}

#[test]
fn route_limit_is_clamped() {
    assert_eq!(spot_route_limit(None), 50);
    assert_eq!(spot_route_limit(Some(0)), 1);
    assert_eq!(spot_route_limit(Some(500)), 100);
}

#[test]
fn spot_fill_wallet_lock_keys_are_sorted_and_unique() {
    assert_eq!(
        spot_fill_wallet_lock_keys(20, 10, 2, 1),
        vec![(10, 1), (10, 2), (20, 1), (20, 2)]
    );
    assert_eq!(spot_fill_wallet_lock_keys(7, 7, 2, 1), vec![(7, 1), (7, 2)]);
}

#[test]
fn spot_fill_order_lock_keys_are_canonical_sorted_and_unique() {
    assert_eq!(spot_fill_order_lock_keys("20", "10").unwrap(), vec![10, 20]);
    assert_eq!(spot_fill_order_lock_keys("0010", "10").unwrap(), vec![10]);
}

#[test]
fn stop_limit_buy_requires_trigger_and_limit_prices() {
    let order = SpotOrder {
        id: "7".to_owned(),
        user_id: "42".to_owned(),
        pair_id: "BTC-USDT".to_owned(),
        side: OrderSide::Buy,
        order_type: OrderType::StopLimit,
        price: Some(decimal("95")),
        trigger_price: Some(decimal("100")),
        quantity: decimal("1"),
        filled_quantity: decimal("0"),
        status: OrderStatus::Pending,
    };

    assert!(is_triggerable_stop_limit_buy_order(&order, &decimal("94")));
    assert!(!is_triggerable_stop_limit_buy_order(&order, &decimal("98")));
    assert!(!is_triggerable_stop_limit_buy_order(
        &order,
        &decimal("101")
    ));
}

#[test]
fn stop_limit_sell_requires_trigger_and_limit_prices() {
    let order = SpotOrder {
        id: "8".to_owned(),
        user_id: "42".to_owned(),
        pair_id: "BTC-USDT".to_owned(),
        side: OrderSide::Sell,
        order_type: OrderType::StopLimit,
        price: Some(decimal("105")),
        trigger_price: Some(decimal("100")),
        quantity: decimal("1"),
        filled_quantity: decimal("0"),
        status: OrderStatus::Pending,
    };

    assert!(is_triggerable_stop_limit_sell_order(
        &order,
        &decimal("106")
    ));
    assert!(!is_triggerable_stop_limit_sell_order(
        &order,
        &decimal("102")
    ));
    assert!(!is_triggerable_stop_limit_sell_order(
        &order,
        &decimal("99")
    ));
}

#[test]
fn route_new_order_requires_market_reference_price() {
    let pair = crate::modules::spot::TradingPairRule {
        pair_id: "BTC-USDT".to_owned(),
        price_precision: 2,
        quantity_precision: 4,
        min_order_value: decimal("10"),
        enabled: true,
    };
    let request = CreateSpotOrderRequest {
        pair_id: "BTC-USDT".to_owned(),
        side: OrderSide::Buy,
        order_type: OrderType::Market,
        price: None,
        trigger_price: None,
        quantity: decimal("0.1"),
        reference_price: None,
        idempotency_key: None,
    };
    let result = build_create_spot_order_use_case(42, &request, &pair);

    assert!(matches!(result, Err(AppError::Validation(_))));
}

#[test]
fn market_reference_price_allows_small_buy_uptick() {
    assert!(
        ensure_market_price_within_reference(
            OrderSide::Buy,
            &decimal("100.090000000000000000"),
            &decimal("100.000000000000000000"),
        )
        .is_ok()
    );
}

#[test]
fn market_reference_price_rejects_buy_above_tolerance() {
    let result = ensure_market_price_within_reference(
        OrderSide::Buy,
        &decimal("100.110000000000000000"),
        &decimal("100.000000000000000000"),
    );

    assert!(
        matches!(result, Err(AppError::Validation(message)) if message == "market price exceeds submitted reference price; please retry")
    );
}

#[test]
fn market_reference_price_allows_small_sell_downtick() {
    assert!(
        ensure_market_price_within_reference(
            OrderSide::Sell,
            &decimal("99.910000000000000000"),
            &decimal("100.000000000000000000"),
        )
        .is_ok()
    );
}

#[test]
fn market_reference_price_rejects_sell_below_tolerance() {
    let result = ensure_market_price_within_reference(
        OrderSide::Sell,
        &decimal("99.890000000000000000"),
        &decimal("100.000000000000000000"),
    );

    assert!(
        matches!(result, Err(AppError::Validation(message)) if message == "market price is below submitted reference price; please retry")
    );
}

#[test]
fn market_buy_reserves_execution_price_when_it_is_above_reference() {
    let reference_price = decimal("100.000000000000000000");
    let execution_price = decimal("100.090000000000000000");

    let reservation_price =
        market_buy_reservation_price(Some(&reference_price), &execution_price).unwrap();

    assert_eq!(reservation_price, &execution_price);
}

#[test]
fn market_buy_reserves_reference_price_when_it_is_above_execution() {
    let reference_price = decimal("100.000000000000000000");
    let execution_price = decimal("99.990000000000000000");

    let reservation_price =
        market_buy_reservation_price(Some(&reference_price), &execution_price).unwrap();

    assert_eq!(reservation_price, &reference_price);
}

#[test]
fn spot_order_reservation_uses_quote_asset_for_buy_limit() {
    let order = NewOrder {
        user_id: "42".to_owned(),
        pair_id: "BTC-USDT".to_owned(),
        side: OrderSide::Buy,
        order_type: OrderType::Limit,
        price: Some(decimal("10")),
        trigger_price: None,
        quantity: decimal("2"),
        filled_quantity: decimal("0"),
        status: OrderStatus::Pending,
    };

    let reservation = spot_order_reservation(&order, None, 1, 2).unwrap();

    assert_eq!(reservation.asset_id, 2);
    assert_eq!(reservation.amount, decimal("20"));
}

#[test]
fn spot_order_reservation_requires_reference_for_market_order() {
    let order = NewOrder {
        user_id: "42".to_owned(),
        pair_id: "BTC-USDT".to_owned(),
        side: OrderSide::Buy,
        order_type: OrderType::Market,
        price: None,
        trigger_price: None,
        quantity: decimal("2"),
        filled_quantity: decimal("0"),
        status: OrderStatus::Pending,
    };

    let result = spot_order_reservation(&order, None, 1, 2);

    assert!(
        matches!(result, Err(AppError::Validation(message)) if message == "reference_price is required for market orders")
    );
}
