ALTER TABLE event_inbox
    ADD COLUMN retry_count INT NOT NULL DEFAULT 0 AFTER error_message;
