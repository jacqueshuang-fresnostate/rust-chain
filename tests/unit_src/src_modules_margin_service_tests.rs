use super::publish_margin_position_closed_event;
use crate::modules::{
    events::{EventBroadcastHub, WebSocketChannel},
    margin::presentation::MarginPositionResponse,
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
        margin_amount: decimal("20"),
        leverage: decimal("5"),
        notional_amount: decimal("100"),
        borrowed_amount: decimal("80"),
        interest_amount: decimal("1.25"),
        entry_price: Some(decimal("100")),
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
