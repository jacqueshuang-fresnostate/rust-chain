ALTER TABLE seconds_contract_orders
    ADD COLUMN entry_price DECIMAL(38,18) NULL AFTER payout_rate,
    ADD INDEX idx_seconds_contract_orders_status_expires (status, expires_at, id);
