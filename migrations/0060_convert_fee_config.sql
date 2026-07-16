ALTER TABLE convert_pairs
    ADD COLUMN fee_rate DECIMAL(18,8) NOT NULL DEFAULT 0 COMMENT '闪兑手续费率；按支付资产数量计费' AFTER spread_rate,
    ADD CONSTRAINT chk_convert_pairs_fee_rate CHECK (fee_rate >= 0 AND fee_rate < 1);

ALTER TABLE convert_quotes
    ADD COLUMN fee_rate DECIMAL(18,8) NOT NULL DEFAULT 0 COMMENT '报价时锁定的闪兑手续费率' AFTER spread_rate,
    ADD COLUMN fee_amount DECIMAL(38,18) NOT NULL DEFAULT 0 COMMENT '报价时支付资产手续费金额' AFTER fee_rate,
    ADD CONSTRAINT chk_convert_quotes_fee CHECK (fee_rate >= 0 AND fee_rate < 1 AND fee_amount >= 0);

ALTER TABLE convert_orders
    ADD COLUMN fee_rate DECIMAL(18,8) NOT NULL DEFAULT 0 COMMENT '成交时锁定的闪兑手续费率' AFTER rate,
    ADD COLUMN fee_amount DECIMAL(38,18) NOT NULL DEFAULT 0 COMMENT '成交时支付资产手续费金额' AFTER fee_rate,
    ADD CONSTRAINT chk_convert_orders_fee CHECK (fee_rate >= 0 AND fee_rate < 1 AND fee_amount >= 0);
