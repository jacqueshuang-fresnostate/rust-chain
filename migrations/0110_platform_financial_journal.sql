-- 平台级资金分录：承载用户钱包之外的收入、应收、清算与坏账腿。
CREATE TABLE platform_financial_journal (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT COMMENT '平台资金分录ID',
    transaction_key VARCHAR(255) NOT NULL COMMENT '同一业务结算的稳定幂等键',
    context VARCHAR(64) NOT NULL COMMENT '业务上下文：new_coin_unlock_fee/loan_disbursement/loan_repayment/loan_liquidation',
    account_code VARCHAR(96) NOT NULL COMMENT '平台会计科目或对账腿',
    asset_id BIGINT UNSIGNED NOT NULL COMMENT '记账资产',
    amount DECIMAL(38,18) NOT NULL COMMENT '带符号变动：借方为负、贷方为正',
    ref_type VARCHAR(64) NOT NULL COMMENT '关联业务类型',
    ref_id VARCHAR(128) NOT NULL COMMENT '关联业务编号',
    metadata_json JSON NULL COMMENT '脱敏对账元数据',
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    UNIQUE KEY uq_platform_financial_journal_leg (transaction_key, account_code, asset_id),
    INDEX idx_platform_financial_journal_ref (ref_type, ref_id),
    INDEX idx_platform_financial_journal_context_time (context, created_at),
    CONSTRAINT fk_platform_financial_journal_asset FOREIGN KEY (asset_id) REFERENCES assets(id),
    CONSTRAINT chk_platform_financial_journal_non_zero CHECK (amount <> 0)
) COMMENT='用户钱包之外的平台资金与坏账分录';
