-- 每条返佣记录保存本次实际分配的差额比例，便于后台审计多级分润结果。
ALTER TABLE agent_commission_records
    ADD COLUMN commission_rate DECIMAL(18,8) NULL AFTER payout_asset_id;

-- 历史记录只有来源金额和返佣金额，按可恢复的实际比例回填；零来源记录按零处理。
UPDATE agent_commission_records
SET commission_rate = CASE
    WHEN source_amount > 0
        THEN LEAST(1, GREATEST(0, TRUNCATE(commission_amount / source_amount, 8)))
    ELSE 0
END
WHERE commission_rate IS NULL;

ALTER TABLE agent_commission_records
    MODIFY COLUMN commission_rate DECIMAL(18,8) NOT NULL DEFAULT 0;
