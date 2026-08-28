use super::{publish_margin_position_close_event_if_needed, publish_margin_position_closed_event};
use crate::modules::{
    events::{EventBroadcastHub, WebSocketChannel},
    margin::presentation::{
        CloseMarginPositionResponse, MarginPositionCloseExecutionResponse, MarginPositionResponse,
    },
};
use bigdecimal::BigDecimal;
use chrono::{TimeZone, Utc};
use serde_json::Value;
use std::{str::FromStr, time::Duration};
use tokio::time::timeout;

fn decimal(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).expect("valid decimal")
}

#[tokio::test]
async fn closed_event_keeps_interest_adjusted_payout_json() {
    let hub = EventBroadcastHub::new(1);
    let mut events = hub.subscribe(&WebSocketChannel::private_user(7));
    let position = MarginPositionResponse {
        id: 11,
        user_id: 7,
        product_id: 13,
        pair_id: 17,
        margin_asset: 19,
        wallet_scope: "spot".to_owned(),
        margin_mode: "isolated".to_owned(),
        direction: "long".to_owned(),
        order_type: "market".to_owned(),
        margin_amount: decimal("20"),
        leverage: decimal("5"),
        notional_amount: decimal("100"),
        borrowed_amount: decimal("80"),
        interest_amount: decimal("1.25"),
        entry_price: Some(decimal("100")),
        limit_price: None,
        exit_price: Some(decimal("110")),
        realized_pnl: Some(decimal("10")),
        closed_at: Some(
            Utc.timestamp_millis_opt(1_700_000_000_000)
                .single()
                .expect("valid timestamp"),
        ),
        status: "closed".to_owned(),
        idempotency_key: "margin-close-test".to_owned(),
    };

    publish_margin_position_closed_event(&hub, 7, &position);

    let message = timeout(Duration::from_millis(100), events.recv())
        .await
        .expect("event timeout")
        .expect("closed event");
    let payload: Value = serde_json::from_str(message.payload()).expect("valid event JSON");
    assert_eq!(payload["type"], "margin.position.closed");
    assert_eq!(payload["interest_amount"], "1.250000000000000000");
    assert_eq!(payload["payout_amount"], "28.750000000000000000");
    assert_eq!(payload["closed_at"], 1_700_000_000_000_i64);
}

#[tokio::test]
async fn partial_close_event_is_post_commit_refresh_context_and_replay_is_silent() {
    let hub = EventBroadcastHub::new(2);
    let mut events = hub.subscribe(&WebSocketChannel::private_user(7));
    let created_at = Utc
        .timestamp_millis_opt(1_700_000_000_000)
        .single()
        .expect("valid timestamp");
    let response = CloseMarginPositionResponse {
        position: MarginPositionResponse {
            id: 11,
            user_id: 7,
            product_id: 13,
            pair_id: 17,
            margin_asset: 19,
            wallet_scope: "spot".to_owned(),
            margin_mode: "isolated".to_owned(),
            direction: "long".to_owned(),
            order_type: "market".to_owned(),
            margin_amount: decimal("10"),
            leverage: decimal("5"),
            notional_amount: decimal("50"),
            borrowed_amount: decimal("40"),
            interest_amount: decimal("0.625"),
            entry_price: Some(decimal("100")),
            limit_price: None,
            exit_price: None,
            realized_pnl: Some(decimal("5")),
            closed_at: None,
            status: "opened".to_owned(),
            idempotency_key: "margin-open-test".to_owned(),
        },
        execution: Some(MarginPositionCloseExecutionResponse {
            id: 23,
            position_id: 11,
            idempotency_key: "margin-partial-close-test".to_owned(),
            close_percentage: 50,
            close_margin_amount: decimal("10"),
            close_notional_amount: decimal("50"),
            close_borrowed_amount: decimal("40"),
            close_interest_amount: decimal("0.625"),
            exit_price: decimal("110"),
            realized_pnl: decimal("5"),
            settlement_amount: decimal("14.375"),
            fully_closed: false,
            created_at,
        }),
        settlement_amount: Some(decimal("14.375")),
        replayed: false,
    };

    publish_margin_position_close_event_if_needed(Some(&hub), 7, &response, true);
    let message = timeout(Duration::from_millis(100), events.recv())
        .await
        .expect("event timeout")
        .expect("partial close event");
    let payload: Value = serde_json::from_str(message.payload()).expect("valid event JSON");
    assert_eq!(payload["type"], "margin.position.partially_closed");
    assert_eq!(payload["execution_id"], 23);
    assert_eq!(payload["close_percentage"], 50);
    assert_eq!(payload["settlement_amount"], "14.375000000000000000");
    assert_eq!(
        payload["remaining_notional_amount"],
        "50.000000000000000000"
    );

    publish_margin_position_close_event_if_needed(Some(&hub), 7, &response, false);
    assert!(
        timeout(Duration::from_millis(25), events.recv())
            .await
            .is_err()
    );
}
