ALTER TABLE assets
    ADD COLUMN withdraw_enabled BOOLEAN NOT NULL DEFAULT TRUE COMMENT '是否支持用户提现' AFTER deposit_enabled;
