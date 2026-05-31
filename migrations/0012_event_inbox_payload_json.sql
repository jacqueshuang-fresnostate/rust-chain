ALTER TABLE event_inbox
    ADD COLUMN payload_json JSON NULL AFTER payload_hash;

UPDATE event_inbox AS inbox
LEFT JOIN event_outbox AS outbox ON outbox.idempotency_key = inbox.idempotency_key
SET inbox.payload_json = CASE
    WHEN outbox.id IS NOT NULL THEN JSON_OBJECT(
        'aggregate_type', outbox.aggregate_type,
        'aggregate_id', outbox.aggregate_id,
        'event_type', outbox.event_type,
        'routing_key', outbox.routing_key,
        'idempotency_key', outbox.idempotency_key,
        'payload', outbox.payload_json
    )
    ELSE JSON_OBJECT('legacy_missing_payload', TRUE, 'idempotency_key', inbox.idempotency_key)
END
WHERE inbox.payload_json IS NULL;

UPDATE event_inbox
SET status = 'dead_letter',
    error_message = 'event inbox payload_json is missing and cannot be replayed',
    next_retry_at = NULL,
    updated_at = CURRENT_TIMESTAMP(6)
WHERE status = 'retry'
  AND JSON_EXTRACT(payload_json, '$.legacy_missing_payload') = TRUE;

ALTER TABLE event_inbox
    MODIFY COLUMN payload_json JSON NOT NULL;
