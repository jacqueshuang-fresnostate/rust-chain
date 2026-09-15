-- 逐仓强平同样可能穿仓：负权益此前被直接截零，缺口在库内无处可查，报表与全仓口径也对不上。
-- 新增列只登记本次强平的穿仓缺口（负权益绝对值），未穿仓为 0，不改动任何历史行的资金含义。
ALTER TABLE margin_liquidation_records
    ADD COLUMN bad_debt_amount DECIMAL(38,18) NOT NULL DEFAULT 0 COMMENT '穿仓缺口：负权益绝对值，未穿仓为 0' AFTER payout_amount;

ALTER TABLE margin_liquidation_records
    ADD CONSTRAINT chk_margin_liquidation_records_bad_debt CHECK (bad_debt_amount >= 0);
