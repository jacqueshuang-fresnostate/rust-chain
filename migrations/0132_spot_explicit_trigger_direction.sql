-- NULL preserves the legacy buy <= / sell >= conjunction. Never backfill old intent.
ALTER TABLE spot_orders
    ADD COLUMN trigger_direction VARCHAR(16) CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci NULL,
    ADD COLUMN triggered_at TIMESTAMP(6) NULL,
    ADD CONSTRAINT chk_spot_trigger_direction CHECK (
        trigger_direction IS NULL OR
        (order_type = 'stop_limit' AND trigger_direction IN ('rising', 'falling'))
    ),
    ADD CONSTRAINT chk_spot_triggered_at CHECK (
        triggered_at IS NULL OR trigger_direction IS NOT NULL
    );
