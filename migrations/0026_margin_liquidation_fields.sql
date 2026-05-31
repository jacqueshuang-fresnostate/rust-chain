ALTER TABLE margin_positions
    ADD COLUMN entry_price DECIMAL(38,18) NULL AFTER notional_amount,
    ADD COLUMN exit_price DECIMAL(38,18) NULL AFTER closed_at,
    ADD COLUMN realized_pnl DECIMAL(38,18) NULL AFTER exit_price,
    ADD COLUMN liquidated_at TIMESTAMP(6) NULL AFTER realized_pnl,
    ADD COLUMN liquidation_reason VARCHAR(64) NULL AFTER liquidated_at,
    ADD COLUMN next_liquidation_attempt_at TIMESTAMP(6) NULL AFTER liquidation_reason,
    ADD INDEX idx_margin_positions_liquidation_attempt (status, next_liquidation_attempt_at, opened_at, id);
