ALTER TABLE spot_orders
    ADD COLUMN reserved_asset BIGINT UNSIGNED NULL AFTER idempotency_key,
    ADD COLUMN reserved_amount DECIMAL(38,18) NULL AFTER reserved_asset,
    ADD CONSTRAINT fk_spot_orders_reserved_asset FOREIGN KEY (reserved_asset) REFERENCES assets(id);
