ALTER TABLE spot_orders
    ADD COLUMN request_reference_price DECIMAL(38,18) NULL AFTER reserved_amount;
