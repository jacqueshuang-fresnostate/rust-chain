ALTER TABLE spot_trades
    ADD COLUMN idempotency_key VARCHAR(255) NULL UNIQUE AFTER fee;
