ALTER TABLE margin_positions
    ADD COLUMN order_type VARCHAR(16) NULL COMMENT '杠杆订单类型' AFTER direction,
    ADD COLUMN limit_price DECIMAL(38,18) NULL COMMENT '杠杆限价委托价格' AFTER entry_price;

UPDATE margin_positions
SET order_type = 'market'
WHERE order_type IS NULL;

ALTER TABLE margin_positions
    MODIFY COLUMN order_type VARCHAR(16)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'market'
        COMMENT '杠杆订单类型',
    ADD CONSTRAINT chk_margin_positions_order_type
        CHECK (order_type IN ('market', 'limit')),
    ADD CONSTRAINT chk_margin_positions_order_price
        CHECK (
            (order_type = 'market' AND limit_price IS NULL)
            OR (order_type = 'limit' AND limit_price > 0)
        ),
    ADD INDEX idx_margin_positions_limit_trigger
        (pair_id, status, order_type, entry_price, direction, limit_price, id);
