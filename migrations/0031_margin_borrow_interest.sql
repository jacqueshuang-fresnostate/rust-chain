ALTER TABLE margin_products
    ADD COLUMN hourly_interest_rate DECIMAL(18,8) NOT NULL DEFAULT 0.00000000 AFTER maintenance_margin_rate,
    ADD CONSTRAINT chk_margin_products_hourly_interest CHECK (hourly_interest_rate >= 0);

ALTER TABLE margin_positions
    ADD COLUMN borrowed_amount DECIMAL(38,18) NOT NULL DEFAULT 0.000000000000000000 AFTER notional_amount,
    ADD COLUMN interest_amount DECIMAL(38,18) NOT NULL DEFAULT 0.000000000000000000 AFTER borrowed_amount,
    ADD COLUMN interest_accrued_at TIMESTAMP(6) NULL AFTER interest_amount,
    ADD INDEX idx_margin_positions_interest_accrual (status, interest_accrued_at, opened_at, id),
    ADD CONSTRAINT chk_margin_positions_borrowed CHECK (borrowed_amount >= 0),
    ADD CONSTRAINT chk_margin_positions_interest CHECK (interest_amount >= 0);
