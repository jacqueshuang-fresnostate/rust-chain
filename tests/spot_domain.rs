use bigdecimal::BigDecimal;
use exchange_api::modules::spot::{
    OrderSide, OrderStatus, OrderType, SpotDomainError, SpotOrder, TradingPairRule, apply_fill,
    cancel_order, create_limit_order, create_market_order, create_stop_limit_order,
    validate_order_request,
};
use std::str::FromStr;

fn dec(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).unwrap()
}

fn pair() -> TradingPairRule {
    TradingPairRule {
        pair_id: "BTC-USDT".to_owned(),
        price_precision: 2,
        quantity_precision: 4,
        min_order_value: dec("10"),
        enabled: true,
    }
}

fn open_limit_order(quantity: &str) -> SpotOrder {
    SpotOrder {
        id: "order-1".to_owned(),
        user_id: "user-1".to_owned(),
        pair_id: "BTC-USDT".to_owned(),
        side: OrderSide::Buy,
        order_type: OrderType::Limit,
        price: Some(dec("100")),
        trigger_price: None,
        quantity: dec(quantity),
        filled_quantity: dec("0"),
        status: OrderStatus::Open,
    }
}

#[test]
fn creates_valid_limit_order() {
    let order = create_limit_order(
        "user-1",
        OrderSide::Buy,
        dec("100.12"),
        dec("0.1000"),
        &pair(),
    )
    .unwrap();

    assert_eq!(order.user_id, "user-1");
    assert_eq!(order.pair_id, "BTC-USDT");
    assert_eq!(order.side, OrderSide::Buy);
    assert_eq!(order.order_type, OrderType::Limit);
    assert_eq!(order.price, Some(dec("100.12")));
    assert_eq!(order.quantity, dec("0.1000"));
    assert_eq!(order.filled_quantity, dec("0"));
    assert_eq!(order.status, OrderStatus::Pending);
}

#[test]
fn creates_valid_stop_limit_order() {
    let order = create_stop_limit_order(
        "user-1",
        OrderSide::Sell,
        dec("105.00"),
        dec("104.50"),
        dec("0.1000"),
        &pair(),
    )
    .unwrap();

    assert_eq!(order.side, OrderSide::Sell);
    assert_eq!(order.order_type, OrderType::StopLimit);
    assert_eq!(order.trigger_price, Some(dec("105.00")));
    assert_eq!(order.price, Some(dec("104.50")));
    assert_eq!(order.quantity, dec("0.1000"));
    assert_eq!(order.status, OrderStatus::Pending);
}

#[test]
fn creates_valid_market_order_without_stored_price() {
    let order = create_market_order(
        "user-1",
        OrderSide::Sell,
        dec("0.1000"),
        dec("100.12"),
        &pair(),
    )
    .unwrap();

    assert_eq!(order.side, OrderSide::Sell);
    assert_eq!(order.order_type, OrderType::Market);
    assert_eq!(order.price, None);
    assert_eq!(order.quantity, dec("0.1000"));
    assert_eq!(order.status, OrderStatus::Pending);
}

#[test]
fn rejects_orders_below_min_order_value() {
    let result = create_limit_order(
        "user-1",
        OrderSide::Buy,
        dec("100.00"),
        dec("0.0999"),
        &pair(),
    );

    assert_eq!(
        result,
        Err(SpotDomainError::MinOrderValueNotMet {
            actual: dec("9.990000"),
            minimum: dec("10"),
        })
    );
}

#[test]
fn rejects_price_and_quantity_precision_overflow() {
    assert_eq!(
        create_limit_order(
            "user-1",
            OrderSide::Buy,
            dec("100.123"),
            dec("0.1000"),
            &pair()
        ),
        Err(SpotDomainError::PricePrecisionExceeded { allowed: 2 })
    );
    assert_eq!(
        create_limit_order(
            "user-1",
            OrderSide::Buy,
            dec("100.12"),
            dec("0.10001"),
            &pair()
        ),
        Err(SpotDomainError::QuantityPrecisionExceeded { allowed: 4 })
    );
}

#[test]
fn market_order_request_rejects_explicit_price() {
    assert_eq!(
        validate_order_request(OrderType::Market, Some(dec("100")), dec("0.1"), &pair()),
        Err(SpotDomainError::MarketOrderRejectsPrice)
    );
}

#[test]
fn cancel_is_idempotent_for_already_cancelled_orders() {
    let mut order = open_limit_order("1");

    assert_eq!(cancel_order(&mut order), Ok(true));
    assert_eq!(order.status, OrderStatus::Cancelled);
    assert_eq!(cancel_order(&mut order), Ok(false));
    assert_eq!(order.status, OrderStatus::Cancelled);
}

#[test]
fn fill_accounting_transitions_partial_to_filled() {
    let mut order = open_limit_order("10");

    apply_fill(&mut order, dec("4")).unwrap();
    assert_eq!(order.filled_quantity, dec("4"));
    assert_eq!(order.status, OrderStatus::PartiallyFilled);

    apply_fill(&mut order, dec("6")).unwrap();
    assert_eq!(order.filled_quantity, dec("10"));
    assert_eq!(order.status, OrderStatus::Filled);
}
