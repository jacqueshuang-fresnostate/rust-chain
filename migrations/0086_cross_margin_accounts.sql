-- 全仓账户按用户和保证金资产隔离，仓位共享同一账户权益和组合风险。
CREATE TABLE margin_cross_accounts (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    user_id BIGINT UNSIGNED NOT NULL,
    margin_asset BIGINT UNSIGNED NOT NULL,
    status VARCHAR(16) NOT NULL DEFAULT 'active',
    last_equity DECIMAL(38,18) NOT NULL DEFAULT 0,
    last_unrealized_pnl DECIMAL(38,18) NOT NULL DEFAULT 0,
    last_interest_amount DECIMAL(38,18) NOT NULL DEFAULT 0,
    last_maintenance_margin DECIMAL(38,18) NOT NULL DEFAULT 0,
    last_margin_ratio DECIMAL(38,18) NULL,
    last_risk_at TIMESTAMP(6) NULL,
    version BIGINT UNSIGNED NOT NULL DEFAULT 0,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    UNIQUE KEY uq_margin_cross_accounts_user_asset (user_id, margin_asset),
    INDEX idx_margin_cross_accounts_status (status),
    CONSTRAINT fk_margin_cross_accounts_user FOREIGN KEY (user_id) REFERENCES users(id),
    CONSTRAINT fk_margin_cross_accounts_asset FOREIGN KEY (margin_asset) REFERENCES assets(id),
    CONSTRAINT chk_margin_cross_accounts_status CHECK (status IN ('active', 'liquidating', 'liquidated'))
);

ALTER TABLE margin_positions
    ADD INDEX idx_margin_positions_cross_account (user_id, margin_asset, margin_mode, status, id);
