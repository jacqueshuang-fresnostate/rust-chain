use super::*;
use crate::modules::spot::presentation::CreateSpotOrderRequest;
use std::str::FromStr;

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}

#[test]
fn spot_numeric_source_and_generated_amount_boundaries() {
    assert!(ensure_spot_asset_amount(&decimal("1.000000010000"), 8, "quantity").is_ok());
    for value in ["1.000000001", "1e-19", "1e20", "1e4294967296"] {
        assert!(ensure_spot_asset_amount(&decimal(value), 8, "quantity").is_err());
    }
    assert!(ensure_spot_asset_amount(&decimal("1"), -1, "quantity").is_err());
    assert!(ensure_spot_asset_amount(&decimal("1"), 19, "quantity").is_err());
    assert_eq!(
        spot_quote_amount(&decimal("1.00000001"), &decimal("1.00000001"), 8).unwrap(),
        decimal("1.00000002")
    );
    assert_eq!(
        spot_quote_amount(&decimal("1.0000000001"), &decimal("1.0000000001"), 18).unwrap(),
        decimal("1.0000000002")
    );
    assert_eq!(
        spot_quote_amount(&decimal("0.000000001"), &decimal("0.000000001"), 18).unwrap(),
        decimal("1e-18")
    );
    assert!(spot_quote_amount(&decimal("1e-18"), &decimal("1e-18"), 18).is_err());
    assert!(spot_quote_amount(&decimal("1e19"), &decimal("10"), 8).is_err());
    assert!(spot_quote_amount(&decimal("1e-19"), &decimal("10"), 18).is_err());
}

#[test]
fn spot_numeric_partial_fills_conserve_reservation_and_return_dust() {
    let price = decimal("1.00000001");
    let quantities = [
        decimal("0.33333333"),
        decimal("0.33333333"),
        decimal("0.33333334"),
    ];
    let reservation = spot_quote_amount(&price, &decimal("1"), 8).unwrap();
    let mut remaining = reservation.clone();
    let mut spent = decimal("0");
    for quantity in quantities {
        let quote = spot_quote_amount(&price, &quantity, 8).unwrap();
        remaining -= &quote;
        spent += &quote;
        assert!(remaining >= decimal("0"));
        assert_eq!(&spent + &remaining, reservation);
    }
    assert_eq!(spent, decimal("1"));
    assert_eq!(remaining, decimal("0.00000001"));
    // 全成释放尾差，部分成交取消同样释放原预留减实际扣款，而非再乘剩余量。
    let first_fill = spot_quote_amount(&price, &decimal("0.33333333"), 8).unwrap();
    let refund = &reservation - &first_fill;
    assert_eq!(refund, decimal("0.66666668"));
    assert_eq!(first_fill + refund, reservation);
}

#[test]
fn spot_numeric_ids_never_narrow_through_javascript_integer_range() {
    assert_eq!(
        parse_spot_order_request_id("9007199254740993").unwrap(),
        9_007_199_254_740_993
    );
    assert_eq!(
        parse_spot_order_request_id("18446744073709551615").unwrap(),
        u64::MAX
    );
    for value in ["0", "-1", "1.1", "1e3", "18446744073709551616"] {
        assert!(parse_spot_order_request_id(value).is_err(), "{value}");
    }
}

#[test]
fn spot_numeric_pair_config_create_and_update_reject_out_of_storage_bounds() {
    use crate::modules::admin::{
        presentation::{CreateTradingPairRequest, UpdateTradingPairRequest},
        service::{validate_create_trading_pair_request, validate_update_trading_pair_request},
    };
    for (price_precision, qty_precision, minimum, valid) in [
        (18, 18, "0.000000000000000001", true),
        (0, 0, "99999999999999999999", true),
        (-1, 8, "1", false),
        (19, 8, "1", false),
        (8, i32::MAX, "1", false),
        (8, 8, "1e-19", false),
        (8, 8, "1e20", false),
        (8, 8, "0", false),
    ] {
        let create = CreateTradingPairRequest {
            base_asset_id: 1,
            quote_asset_id: 2,
            symbol: "BTC-USDT".to_owned(),
            logo_url: None,
            price_precision,
            qty_precision,
            min_order_value: decimal(minimum),
            status: Some("active".to_owned()),
            market_type: Some("external".to_owned()),
            reason: None,
        };
        assert_eq!(validate_create_trading_pair_request(&create).is_ok(), valid);
        let update = UpdateTradingPairRequest {
            logo_url: None,
            price_precision,
            qty_precision,
            min_order_value: decimal(minimum),
            status: "active".to_owned(),
            market_type: "external".to_owned(),
            reason: None,
        };
        assert_eq!(validate_update_trading_pair_request(&update).is_ok(), valid);
    }
}

fn request(pair_id: &str, price: &str, quantity: &str) -> CreateSpotOrderRequest {
    CreateSpotOrderRequest {
        pair_id: pair_id.to_owned(),
        side: OrderSide::Buy,
        order_type: OrderType::Limit,
        price: Some(decimal(price)),
        trigger_price: None,
        trigger_direction: None,
        quantity: decimal(quantity),
        reference_price: None,
        idempotency_key: "client-key".to_owned(),
    }
}

#[test]
fn spot_order_fingerprint_normalizes_pair_and_decimal_text() {
    let first = spot_order_request_fingerprint(7, &request(" btc-usdt ", "10.0", "2.000"));
    let equivalent = spot_order_request_fingerprint(7, &request("BTC-USDT", "10", "2"));
    let changed = spot_order_request_fingerprint(7, &request("BTC-USDT", "10", "3"));

    assert_eq!(first, equivalent);
    assert_ne!(first, changed);
    assert_eq!(first.len(), 64);
}

#[test]
fn spot_order_idempotency_key_is_required_and_bounded() {
    assert!(normalize_idempotency_key("   ").is_err());
    assert!(normalize_idempotency_key(&"x".repeat(129)).is_err());
    assert_eq!(normalize_idempotency_key(" key ").unwrap(), "key");
}

#[test]
fn spot_explicit_trigger_direction_preserves_legacy_fingerprint_and_changes_intent() {
    let mut intent = request("BTC-USDT", "10", "2");
    // Frozen v1 bytes for the no-direction request, independent of the production encoder.
    let mut digest = sha2::Sha256::default();
    for field in [
        "spot_order_create_v1",
        "7",
        "BTC-USDT",
        "buy",
        "limit",
        "10",
        "none",
        "2",
        "none",
    ] {
        sha2::Digest::update(&mut digest, (field.len() as u64).to_be_bytes());
        sha2::Digest::update(&mut digest, field.as_bytes());
    }
    assert_eq!(
        spot_order_request_fingerprint(7, &intent),
        hex::encode(sha2::Digest::finalize(digest))
    );
    intent.order_type = OrderType::StopLimit;
    intent.trigger_price = Some(decimal("12"));
    let legacy = spot_order_request_fingerprint(7, &intent);
    intent.trigger_direction = Some(crate::modules::spot::TriggerDirection::Rising);
    let rising = spot_order_request_fingerprint(7, &intent);
    intent.trigger_direction = Some(crate::modules::spot::TriggerDirection::Falling);
    let falling = spot_order_request_fingerprint(7, &intent);
    assert_ne!(legacy, rising);
    assert_ne!(legacy, falling);
    assert_ne!(rising, falling);
}

#[test]
fn spot_explicit_trigger_activation_is_directional_durable_and_terminal_safe() {
    use crate::modules::spot::TriggerDirection::{Falling, Rising};
    for side in [OrderSide::Buy, OrderSide::Sell] {
        for direction in [Rising, Falling] {
            let mut order = SpotOrder {
                id: "1".into(),
                user_id: "7".into(),
                pair_id: "BTC-USDT".into(),
                side,
                order_type: OrderType::StopLimit,
                price: Some(decimal("10")),
                trigger_price: Some(decimal("12")),
                trigger_direction: Some(direction),
                triggered_at: None,
                quantity: decimal("2"),
                filled_quantity: decimal("0"),
                status: OrderStatus::Open,
            };
            assert!(should_activate_stop_limit_order(&order, &decimal("12")));
            let not_reached = if direction == Rising { "11" } else { "13" };
            assert!(!should_activate_stop_limit_order(
                &order,
                &decimal(not_reached)
            ));
            assert!(!is_triggerable_stop_limit_buy_order(&order, &decimal("10")));
            assert!(!is_triggerable_stop_limit_sell_order(
                &order,
                &decimal("10")
            ));
            order.triggered_at = Some(chrono::Utc::now());
            let predicate = if side == OrderSide::Buy {
                is_triggerable_stop_limit_buy_order
            } else {
                is_triggerable_stop_limit_sell_order
            };
            assert!(predicate(&order, &decimal("10")));
            assert!(!should_activate_stop_limit_order(&order, &decimal("12")));
            for status in [
                OrderStatus::Cancelled,
                OrderStatus::Filled,
                OrderStatus::Rejected,
            ] {
                order.status = status;
                assert!(!predicate(&order, &decimal("10")));
                order.triggered_at = None;
                assert!(!should_activate_stop_limit_order(&order, &decimal("12")));
            }
        }
    }
}
