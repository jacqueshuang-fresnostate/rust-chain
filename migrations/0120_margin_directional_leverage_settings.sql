ALTER TABLE margin_user_settings
    ADD COLUMN long_leverage DECIMAL(18,8) NULL COMMENT '后续做多开仓默认杠杆' AFTER leverage,
    ADD COLUMN short_leverage DECIMAL(18,8) NULL COMMENT '后续做空开仓默认杠杆' AFTER long_leverage;

UPDATE margin_user_settings
SET long_leverage = leverage,
    short_leverage = leverage
WHERE leverage IS NOT NULL;

ALTER TABLE margin_user_settings
    ADD CONSTRAINT chk_margin_user_settings_long_leverage
        CHECK (long_leverage IS NULL OR long_leverage > 0),
    ADD CONSTRAINT chk_margin_user_settings_short_leverage
        CHECK (short_leverage IS NULL OR short_leverage > 0);
