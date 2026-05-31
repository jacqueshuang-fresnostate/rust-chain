ALTER TABLE margin_liquidation_records
    ADD COLUMN interest_amount DECIMAL(38,18) NOT NULL DEFAULT 0.000000000000000000 AFTER notional_amount,
    ADD CONSTRAINT chk_margin_liquidation_records_interest CHECK (interest_amount >= 0);
