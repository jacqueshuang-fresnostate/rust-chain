ALTER TABLE seconds_contract_products
    ADD COLUMN open_payout_capacity DECIMAL(38,18) NULL,
    ADD CONSTRAINT chk_seconds_open_payout_capacity CHECK (open_payout_capacity >= 0);

CREATE INDEX idx_seconds_orders_payout_exposure
    ON seconds_contract_orders (product_id, stake_asset, status);
