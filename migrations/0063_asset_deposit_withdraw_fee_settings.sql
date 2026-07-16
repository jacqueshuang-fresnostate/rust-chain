ALTER TABLE assets
    ADD COLUMN min_deposit_amount DECIMAL(38,18) NOT NULL DEFAULT 0 COMMENT '最小充值数量' AFTER deposit_enabled,
    ADD COLUMN deposit_fee DECIMAL(38,18) NOT NULL DEFAULT 0 COMMENT '充值手续费' AFTER min_deposit_amount,
    ADD COLUMN withdraw_fee DECIMAL(38,18) NOT NULL DEFAULT 0 COMMENT '提现手续费' AFTER deposit_fee;
