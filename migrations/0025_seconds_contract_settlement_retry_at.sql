ALTER TABLE seconds_contract_orders
    ADD COLUMN next_settlement_attempt_at TIMESTAMP(6) NULL AFTER settled_at,
    ADD INDEX idx_seconds_contract_orders_settlement_attempt (status, next_settlement_attempt_at, expires_at, id);
