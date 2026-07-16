ALTER TABLE assets
    ADD COLUMN withdraw_fee_tiers_json JSON NULL COMMENT '资产：提现手续费梯度规则，按提现金额区间配置百分比手续费' AFTER withdraw_fee;
