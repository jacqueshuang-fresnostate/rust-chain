CREATE TABLE seconds_contract_product_cycles (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT COMMENT '周期配置ID',
    product_id BIGINT UNSIGNED NOT NULL COMMENT '秒合约产品ID',
    duration_seconds INT UNSIGNED NOT NULL COMMENT '周期秒数',
    payout_rate DECIMAL(18,8) NOT NULL COMMENT '赔付赔率',
    min_stake DECIMAL(38,18) NOT NULL COMMENT '最小押注金额',
    max_stake DECIMAL(38,18) NULL COMMENT '最大押注金额；为空表示无上限',
    sort_order INT UNSIGNED NOT NULL DEFAULT 0 COMMENT '排序值',
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) COMMENT '创建时间',
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6) COMMENT '更新时间',
    UNIQUE KEY uq_seconds_contract_product_cycles_duration (product_id, duration_seconds),
    INDEX idx_seconds_contract_product_cycles_product (product_id, sort_order, id),
    CONSTRAINT fk_seconds_contract_product_cycles_product FOREIGN KEY (product_id) REFERENCES seconds_contract_products(id) ON DELETE CASCADE,
    CONSTRAINT chk_seconds_contract_product_cycles_duration CHECK (duration_seconds > 0),
    CONSTRAINT chk_seconds_contract_product_cycles_stake CHECK (min_stake > 0 AND (max_stake IS NULL OR max_stake >= min_stake)),
    CONSTRAINT chk_seconds_contract_product_cycles_payout CHECK (payout_rate >= 0)
) COMMENT='秒合约产品周期配置';

INSERT INTO seconds_contract_product_cycles
    (product_id, duration_seconds, payout_rate, min_stake, max_stake, sort_order)
SELECT id, duration_seconds, payout_rate, min_stake, max_stake, 0
FROM seconds_contract_products;

ALTER TABLE seconds_contract_orders
    ADD COLUMN duration_seconds INT UNSIGNED NULL COMMENT '订单周期秒数' AFTER stake_amount;

UPDATE seconds_contract_orders orders
INNER JOIN seconds_contract_products products ON products.id = orders.product_id
SET orders.duration_seconds = products.duration_seconds
WHERE orders.duration_seconds IS NULL;

ALTER TABLE seconds_contract_orders
    MODIFY COLUMN duration_seconds INT UNSIGNED NOT NULL DEFAULT 60 COMMENT '订单周期秒数',
    ADD CONSTRAINT chk_seconds_contract_orders_duration CHECK (duration_seconds > 0);
