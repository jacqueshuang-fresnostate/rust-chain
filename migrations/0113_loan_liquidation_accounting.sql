-- 没有外部卖出通道时，以平台抵押品清算与显式坏账腿完成可对账结算。
ALTER TABLE loan_orders
    ADD COLUMN liquidated_at TIMESTAMP(6) NULL COMMENT '强制清算完成时间' AFTER repaid_at;

ALTER TABLE loan_orders
    DROP CHECK chk_loan_orders_status,
    MODIFY COLUMN status VARCHAR(32) NOT NULL DEFAULT 'pending' COMMENT '订单状态：pending/disbursed/rejected/cancelled/repaid/overdue/liquidated';

ALTER TABLE loan_orders
    ADD CONSTRAINT chk_loan_orders_status CHECK (
        status IN ('pending', 'disbursed', 'rejected', 'cancelled', 'repaid', 'overdue', 'liquidated')
    );

CREATE TABLE loan_liquidations (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT COMMENT '借贷清算ID',
    order_id BIGINT UNSIGNED NOT NULL,
    transaction_key VARCHAR(255) NOT NULL,
    user_id BIGINT UNSIGNED NOT NULL,
    loan_asset_id BIGINT UNSIGNED NOT NULL,
    collateral_asset_id BIGINT UNSIGNED NOT NULL,
    oracle_symbol VARCHAR(64) NOT NULL,
    oracle_source VARCHAR(64) NOT NULL,
    oracle_price DECIMAL(38,18) NOT NULL,
    oracle_observed_at TIMESTAMP(6) NOT NULL,
    ltv DECIMAL(38,18) NOT NULL,
    principal_amount DECIMAL(38,18) NOT NULL,
    interest_amount DECIMAL(38,18) NOT NULL,
    debt_amount DECIMAL(38,18) NOT NULL,
    collateral_seized DECIMAL(38,18) NOT NULL,
    collateral_returned DECIMAL(38,18) NOT NULL,
    recovered_amount DECIMAL(38,18) NOT NULL,
    bad_debt_amount DECIMAL(38,18) NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'completed',
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    UNIQUE KEY uq_loan_liquidations_order (order_id),
    UNIQUE KEY uq_loan_liquidations_transaction (transaction_key),
    INDEX idx_loan_liquidations_user_time (user_id, created_at),
    CONSTRAINT fk_loan_liquidations_order FOREIGN KEY (order_id) REFERENCES loan_orders(id),
    CONSTRAINT fk_loan_liquidations_user FOREIGN KEY (user_id) REFERENCES users(id),
    CONSTRAINT fk_loan_liquidations_loan_asset FOREIGN KEY (loan_asset_id) REFERENCES assets(id),
    CONSTRAINT fk_loan_liquidations_collateral_asset FOREIGN KEY (collateral_asset_id) REFERENCES assets(id),
    CONSTRAINT chk_loan_liquidations_amounts CHECK (
        oracle_price > 0 AND ltv > 0 AND principal_amount > 0 AND interest_amount >= 0
        AND debt_amount = principal_amount + interest_amount
        AND collateral_seized > 0 AND collateral_returned >= 0
        AND collateral_seized + collateral_returned > 0
        AND recovered_amount >= 0 AND bad_debt_amount >= 0
        AND recovered_amount + bad_debt_amount = debt_amount
    ),
    CONSTRAINT chk_loan_liquidations_oracle_source CHECK (
        oracle_source = 'market_ticker_redis'
    ),
    CONSTRAINT chk_loan_liquidations_status CHECK (status = 'completed')
) COMMENT='抵押贷平台清算及坏账快照';
