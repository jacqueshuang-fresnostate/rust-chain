-- 保留旧订单结算事实；只有新写入明确选择 manual_distribution 的订单才具有冻结义务。
ALTER TABLE new_coin_subscriptions
    ADD COLUMN settlement_mode VARCHAR(32) NULL,
    ADD COLUMN frozen_quote_amount DECIMAL(38,18) NULL,
    ADD COLUMN settled_quote_amount DECIMAL(38,18) NULL,
    ADD COLUMN refunded_quote_amount DECIMAL(38,18) NULL;

UPDATE new_coin_subscriptions
SET settlement_mode = 'legacy_instant', frozen_quote_amount = 0;

ALTER TABLE new_coin_subscriptions
    MODIFY COLUMN settlement_mode VARCHAR(32) NOT NULL DEFAULT 'legacy_instant',
    MODIFY COLUMN frozen_quote_amount DECIMAL(38,18) NOT NULL DEFAULT 0,
    ADD INDEX idx_new_coin_subscription_settlement (project_id, settlement_mode, status),
    ADD CONSTRAINT chk_new_coin_subscription_settlement CHECK (
        settlement_mode = 'legacy_instant'
        OR (
            settlement_mode = 'manual_distribution'
            AND frozen_quote_amount >= 0
            AND settled_quote_amount IS NOT NULL AND settled_quote_amount >= 0
            AND refunded_quote_amount IS NOT NULL AND refunded_quote_amount >= 0
            AND frozen_quote_amount + settled_quote_amount + refunded_quote_amount = quote_amount
            AND allocated_quantity >= 0 AND allocated_quantity <= requested_quantity
            AND ((status = 'pending' AND allocated_quantity = 0 AND frozen_quote_amount = quote_amount)
                OR (status IN ('allocated', 'partial_allocated', 'refunded') AND frozen_quote_amount = 0))
        )
    );
