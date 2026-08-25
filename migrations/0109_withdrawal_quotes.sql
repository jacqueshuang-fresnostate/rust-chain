-- 提现冻结必须消费同一条服务端权威报价，配置变化或过期报价不得动账。
CREATE TABLE wallet_withdrawal_quotes (
    id CHAR(36) PRIMARY KEY,
    user_id BIGINT UNSIGNED NOT NULL,
    asset_id BIGINT UNSIGNED NOT NULL,
    asset_symbol VARCHAR(32) NOT NULL,
    network VARCHAR(64) NOT NULL,
    amount DECIMAL(38,18) NOT NULL,
    fee DECIMAL(38,18) NOT NULL,
    net DECIMAL(38,18) NOT NULL,
    total_reserved DECIMAL(38,18) NOT NULL,
    fee_config_version CHAR(64) NOT NULL,
    request_fingerprint CHAR(64) NOT NULL,
    expires_at TIMESTAMP(6) NOT NULL,
    consumed_at TIMESTAMP(6) NULL,
    withdrawal_id BIGINT UNSIGNED NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    INDEX idx_wallet_withdrawal_quotes_owner_expiry (user_id, expires_at),
    UNIQUE KEY uq_wallet_withdrawal_quotes_withdrawal (withdrawal_id),
    CONSTRAINT fk_wallet_withdrawal_quotes_user FOREIGN KEY (user_id) REFERENCES users(id),
    CONSTRAINT fk_wallet_withdrawal_quotes_asset FOREIGN KEY (asset_id) REFERENCES assets(id),
    CONSTRAINT fk_wallet_withdrawal_quotes_withdrawal
        FOREIGN KEY (withdrawal_id) REFERENCES wallet_withdrawal_requests(id)
        ON DELETE RESTRICT,
    CONSTRAINT chk_wallet_withdrawal_quote_amounts CHECK (
        amount > 0 AND fee >= 0 AND net = amount AND total_reserved = amount + fee
    ),
    CONSTRAINT chk_wallet_withdrawal_quote_versions CHECK (
        fee_config_version REGEXP '^[0-9a-f]{64}$'
        AND request_fingerprint REGEXP '^[0-9a-f]{64}$'
    ),
    CONSTRAINT chk_wallet_withdrawal_quote_expiry CHECK (expires_at > created_at),
    CONSTRAINT chk_wallet_withdrawal_quote_consumption CHECK (
        withdrawal_id IS NULL OR consumed_at IS NOT NULL
    )
) DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

ALTER TABLE wallet_withdrawal_requests
    MODIFY COLUMN amount DECIMAL(38,18) NOT NULL,
    MODIFY COLUMN fee DECIMAL(38,18) NOT NULL DEFAULT 0,
    ADD COLUMN withdrawal_quote_id CHAR(36) NULL AFTER gateway_request_id,
    ADD UNIQUE KEY uq_wallet_withdrawal_quote (withdrawal_quote_id),
    ADD CONSTRAINT fk_wallet_withdrawal_quote
        FOREIGN KEY (withdrawal_quote_id) REFERENCES wallet_withdrawal_quotes(id);
