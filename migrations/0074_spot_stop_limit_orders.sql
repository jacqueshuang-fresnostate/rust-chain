ALTER TABLE spot_orders
    ADD COLUMN trigger_price DECIMAL(38,18) NULL COMMENT '现货止盈止损触发价格' AFTER price,
    ADD INDEX idx_spot_orders_stop_limit_trigger (pair_id, order_type, status, side, trigger_price, price);
