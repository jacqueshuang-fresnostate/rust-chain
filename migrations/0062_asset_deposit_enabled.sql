ALTER TABLE assets
    ADD COLUMN deposit_enabled BOOLEAN NOT NULL DEFAULT TRUE COMMENT '是否支持用户充值' AFTER status;
