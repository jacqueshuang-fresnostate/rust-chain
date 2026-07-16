ALTER TABLE margin_products
    ADD COLUMN margin_modes JSON NULL COMMENT '杠杆产品：支持的保证金模式 JSON 列表' AFTER margin_mode;

UPDATE margin_products
SET margin_modes = JSON_ARRAY(margin_mode)
WHERE margin_modes IS NULL;

ALTER TABLE margin_products
    MODIFY COLUMN margin_modes JSON NOT NULL COMMENT '杠杆产品：支持的保证金模式 JSON 列表';
