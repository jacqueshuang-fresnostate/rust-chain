ALTER TABLE wallet_deposit_events
    ADD COLUMN fee_amount DECIMAL(38,18) NOT NULL DEFAULT 0 COMMENT '入账时快照的充值手续费' AFTER amount;
