ALTER TABLE event_inbox
    ADD COLUMN next_retry_at TIMESTAMP(6) NULL AFTER retry_count,
    ADD INDEX idx_event_inbox_retry_due (status, next_retry_at);
