use super::*;
use crate::modules::events::infrastructure::{ExistingInboxMessage, decide_existing_inbox_claim};
use chrono::{TimeDelta, TimeZone, Utc};
use serde_json::json;

#[test]
fn domain_event_constructor_sets_routing_and_idempotency_fields() {
    let created_at = Utc.with_ymd_and_hms(2026, 5, 26, 9, 0, 0).unwrap();

    let event = DomainEvent::new(
        EventRoute::new("convert.events", "convert.completed"),
        EventIdempotency::new("convert_order", "42", "completed"),
        json!({ "quote_id": "quote-1" }),
        created_at,
    );

    assert_eq!(event.exchange, "convert.events");
    assert_eq!(event.routing_key, "convert.completed");
    assert_eq!(event.idempotency_key, "convert_order:42:completed");
    assert_eq!(event.payload, json!({ "quote_id": "quote-1" }));
    assert_eq!(event.created_at, created_at);
}

#[test]
fn inbox_idempotency_key_scopes_message_by_consumer() {
    let key = InboxIdempotency::new("wallet-balance", "message-1", "convert_order:42:completed");

    assert_eq!(key.consumer_name, "wallet-balance");
    assert_eq!(key.message_id, "message-1");
    assert_eq!(key.idempotency_key, "convert_order:42:completed");
    assert_eq!(key.consumer_message_key(), "wallet-balance:message-1");
}

#[test]
fn retry_metadata_tracks_next_attempt_and_dead_letter_threshold() {
    let now = Utc.with_ymd_and_hms(2026, 5, 26, 9, 0, 0).unwrap();
    let metadata = RetryMetadata::new(3, TimeDelta::seconds(10)).unwrap();

    let first_failure = metadata.record_failure(now).unwrap();
    assert_eq!(first_failure.attempt_count(), 1);
    assert_eq!(
        first_failure.next_attempt_at(),
        now + TimeDelta::seconds(10)
    );
    assert!(!first_failure.should_dead_letter());

    let third_failure = first_failure
        .record_failure(now + TimeDelta::seconds(10))
        .unwrap()
        .record_failure(now + TimeDelta::seconds(20))
        .unwrap();

    assert_eq!(third_failure.attempt_count(), 3);
    assert!(third_failure.should_dead_letter());
    assert_eq!(
        RetryMetadata::new(0, TimeDelta::seconds(10)).unwrap_err(),
        RetryMetadataError::InvalidMaxAttempts
    );
    assert_eq!(
        RetryMetadata::new(3, TimeDelta::zero()).unwrap_err(),
        RetryMetadataError::InvalidBackoff
    );
}

#[test]
fn existing_processing_inbox_row_returns_error_for_requeue_after_insert_race() {
    let message = NewInboxMessage::new(
        "wallet-balance",
        "message-1",
        "convert_order:42:completed",
        "payload-hash",
        json!({ "idempotency_key": "convert_order:42:completed" }),
    );
    let existing = ExistingInboxMessage {
        status: INBOX_PROCESSING.to_owned(),
        retry_count: 0,
        message_id: message.message_id.clone(),
        next_retry_at: None,
        updated_at: Utc::now().naive_utc(),
    };

    let error =
        decide_existing_inbox_claim(&message, existing, "token-fresh".to_owned()).unwrap_err();

    assert_eq!(
        error.to_string(),
        "internal error: event inbox message is already processing"
    );
}

#[test]
fn stale_processing_inbox_row_can_be_reclaimed_by_retry_scanner() {
    let message = NewInboxMessage::new(
        "wallet-balance",
        "message-1",
        "convert_order:42:completed",
        "payload-hash",
        json!({ "idempotency_key": "convert_order:42:completed" }),
    );
    let existing = ExistingInboxMessage {
        status: INBOX_PROCESSING.to_owned(),
        retry_count: 2,
        message_id: message.message_id.clone(),
        next_retry_at: None,
        updated_at: (Utc::now() - TimeDelta::seconds(301)).naive_utc(),
    };

    let claim =
        decide_existing_inbox_claim(&message, existing, "token-reclaimed".to_owned()).unwrap();

    assert_eq!(
        claim,
        InboxClaim::Claimed {
            attempt_count: 2,
            processing_token: "token-reclaimed".to_owned(),
        }
    );
}

#[test]
fn stale_processing_inbox_row_with_different_message_id_stays_duplicate() {
    let message = NewInboxMessage::new(
        "wallet-balance",
        "message-2",
        "convert_order:42:completed",
        "payload-hash",
        json!({ "idempotency_key": "convert_order:42:completed" }),
    );
    let existing = ExistingInboxMessage {
        status: INBOX_PROCESSING.to_owned(),
        retry_count: 2,
        message_id: "message-1".to_owned(),
        next_retry_at: None,
        updated_at: (Utc::now() - TimeDelta::seconds(301)).naive_utc(),
    };

    let claim =
        decide_existing_inbox_claim(&message, existing, "token-duplicate".to_owned()).unwrap();

    assert_eq!(claim, InboxClaim::Duplicate);
}
