UPDATE event_inbox
SET status = 'dead_letter',
    error_message = 'event inbox payload_json is missing and cannot be replayed',
    next_retry_at = NULL,
    updated_at = CURRENT_TIMESTAMP(6)
WHERE status = 'processing'
  AND JSON_EXTRACT(payload_json, '$.legacy_missing_payload') = TRUE;
