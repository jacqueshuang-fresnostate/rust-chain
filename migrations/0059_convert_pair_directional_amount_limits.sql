ALTER TABLE convert_pairs
    ADD COLUMN target_min_amount DECIMAL(38,18) NOT NULL DEFAULT 0 COMMENT '目标资产作为支付资产时的最小兑换数量' AFTER max_amount,
    ADD COLUMN target_max_amount DECIMAL(38,18) NULL COMMENT '目标资产作为支付资产时的最大兑换数量；为空表示无上限' AFTER target_min_amount;

UPDATE convert_pairs
SET target_min_amount = min_amount,
    target_max_amount = max_amount;

ALTER TABLE convert_pairs
    ADD CONSTRAINT chk_convert_pairs_target_amounts
    CHECK (target_min_amount >= 0 AND (target_max_amount IS NULL OR target_max_amount >= target_min_amount));
