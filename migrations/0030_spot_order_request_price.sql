ALTER TABLE spot_orders
    ADD COLUMN request_price DECIMAL(38,18) NULL AFTER request_reference_price;
