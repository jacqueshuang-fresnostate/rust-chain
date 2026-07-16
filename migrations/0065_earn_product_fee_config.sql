ALTER TABLE earn_products
    ADD COLUMN redemption_fee_rate DECIMAL(18,8) NOT NULL DEFAULT 0 COMMENT '提现赎回手续费率' AFTER apr_rate,
    ADD COLUMN maturity_profit_fee_rate DECIMAL(18,8) NOT NULL DEFAULT 0 COMMENT '到期获利手续费率' AFTER redemption_fee_rate,
    ADD COLUMN early_redeem_fee_basis VARCHAR(32) NOT NULL DEFAULT 'none' COMMENT '提前赎回扣费基准：none不扣费，principal按本金，profit按收益' AFTER maturity_profit_fee_rate,
    ADD COLUMN early_redeem_fee_rate DECIMAL(18,8) NOT NULL DEFAULT 0 COMMENT '提前赎回扣费率' AFTER early_redeem_fee_basis,
    ADD CONSTRAINT chk_earn_products_redemption_fee_rate CHECK (redemption_fee_rate >= 0 AND redemption_fee_rate <= 1),
    ADD CONSTRAINT chk_earn_products_maturity_profit_fee_rate CHECK (maturity_profit_fee_rate >= 0 AND maturity_profit_fee_rate <= 1),
    ADD CONSTRAINT chk_earn_products_early_redeem_fee_basis CHECK (early_redeem_fee_basis IN ('none', 'principal', 'profit')),
    ADD CONSTRAINT chk_earn_products_early_redeem_fee_rate CHECK (early_redeem_fee_rate >= 0 AND early_redeem_fee_rate <= 1);

ALTER TABLE earn_subscriptions
    ADD COLUMN redemption_fee_rate DECIMAL(18,8) NOT NULL DEFAULT 0 COMMENT '申购时快照的提现赎回手续费率' AFTER apr_rate,
    ADD COLUMN maturity_profit_fee_rate DECIMAL(18,8) NOT NULL DEFAULT 0 COMMENT '申购时快照的到期获利手续费率' AFTER redemption_fee_rate,
    ADD COLUMN early_redeem_fee_basis VARCHAR(32) NOT NULL DEFAULT 'none' COMMENT '申购时快照的提前赎回扣费基准：none不扣费，principal按本金，profit按收益' AFTER maturity_profit_fee_rate,
    ADD COLUMN early_redeem_fee_rate DECIMAL(18,8) NOT NULL DEFAULT 0 COMMENT '申购时快照的提前赎回扣费率' AFTER early_redeem_fee_basis,
    ADD CONSTRAINT chk_earn_subscriptions_redemption_fee_rate CHECK (redemption_fee_rate >= 0 AND redemption_fee_rate <= 1),
    ADD CONSTRAINT chk_earn_subscriptions_maturity_profit_fee_rate CHECK (maturity_profit_fee_rate >= 0 AND maturity_profit_fee_rate <= 1),
    ADD CONSTRAINT chk_earn_subscriptions_early_redeem_fee_basis CHECK (early_redeem_fee_basis IN ('none', 'principal', 'profit')),
    ADD CONSTRAINT chk_earn_subscriptions_early_redeem_fee_rate CHECK (early_redeem_fee_rate >= 0 AND early_redeem_fee_rate <= 1);
