-- 全仓跳空坏账需要独立记录，不能通过负钱包或超额返还隐藏。
ALTER TABLE margin_cross_accounts
    ADD COLUMN last_bad_debt DECIMAL(38,18) NOT NULL DEFAULT 0 AFTER last_margin_ratio;

-- 清理历史固定占位密码；该内部账户仅允许由撮合服务使用。
UPDATE users
SET password_hash = CONCAT('!internal-', REPLACE(UUID(), '-', ''), REPLACE(UUID(), '-', ''))
WHERE email = '__system_spot_liquidity@internal.local'
  AND password_hash = 'system-liquidity';

-- 提现请求成为真实资金状态机：申请时冻结、确认时扣除、失败时解冻。
ALTER TABLE wallet_withdrawal_requests
    ADD COLUMN asset_id BIGINT UNSIGNED NULL AFTER user_id,
    ADD COLUMN total_reserved DECIMAL(38,18) NULL AFTER fee,
    ADD COLUMN idempotency_key VARCHAR(128) NULL AFTER security_method,
    ADD COLUMN gateway_request_id CHAR(36) NULL AFTER idempotency_key,
    ADD COLUMN tx_hash VARCHAR(255) NULL AFTER gateway_request_id,
    ADD COLUMN block_height BIGINT UNSIGNED NULL AFTER tx_hash,
    ADD COLUMN confirmations INT UNSIGNED NOT NULL DEFAULT 0 AFTER block_height,
    ADD COLUMN failure_reason VARCHAR(500) NULL AFTER confirmations,
    ADD COLUMN review_reason VARCHAR(500) NULL AFTER failure_reason,
    ADD COLUMN reviewed_by BIGINT UNSIGNED NULL AFTER review_reason,
    ADD COLUMN reviewed_at TIMESTAMP(6) NULL AFTER reviewed_by,
    ADD COLUMN broadcasting_at TIMESTAMP(6) NULL AFTER reviewed_at,
    ADD COLUMN broadcast_at TIMESTAMP(6) NULL AFTER broadcasting_at,
    ADD COLUMN broadcasted_by BIGINT UNSIGNED NULL AFTER broadcast_at,
    ADD COLUMN confirmed_at TIMESTAMP(6) NULL AFTER broadcasted_by,
    ADD COLUMN confirmed_by BIGINT UNSIGNED NULL AFTER confirmed_at,
    ADD COLUMN failed_at TIMESTAMP(6) NULL AFTER confirmed_by,
    ADD COLUMN failed_by BIGINT UNSIGNED NULL AFTER failed_at,
    ADD COLUMN released_at TIMESTAMP(6) NULL AFTER failed_by,
    ADD COLUMN retry_count INT UNSIGNED NOT NULL DEFAULT 0 AFTER released_at,
    ADD COLUMN next_attempt_at TIMESTAMP(6) NULL AFTER retry_count;

UPDATE wallet_withdrawal_requests requests
INNER JOIN assets ON assets.symbol = requests.asset_symbol
SET requests.asset_id = assets.id,
    requests.total_reserved = requests.amount + requests.fee,
    requests.idempotency_key = CONCAT('legacy-withdrawal-', requests.id),
    requests.gateway_request_id = UUID(),
    requests.status = CASE
        WHEN requests.status = 'pending' THEN 'pending_review'
        ELSE requests.status
    END
WHERE requests.asset_id IS NULL
   OR requests.total_reserved IS NULL
   OR requests.idempotency_key IS NULL
   OR requests.gateway_request_id IS NULL;

ALTER TABLE wallet_withdrawal_requests
    MODIFY COLUMN asset_id BIGINT UNSIGNED NOT NULL,
    MODIFY COLUMN total_reserved DECIMAL(38,18) NOT NULL,
    MODIFY COLUMN idempotency_key VARCHAR(128) NOT NULL,
    MODIFY COLUMN gateway_request_id CHAR(36) NOT NULL,
    ADD UNIQUE KEY uq_wallet_withdrawal_user_idempotency (user_id, idempotency_key),
    ADD UNIQUE KEY uq_wallet_withdrawal_gateway_request (gateway_request_id),
    ADD UNIQUE KEY uq_wallet_withdrawal_network_tx (network, tx_hash),
    ADD INDEX idx_wallet_withdrawal_broadcast (status, next_attempt_at, id),
    ADD CONSTRAINT fk_wallet_withdrawal_asset FOREIGN KEY (asset_id) REFERENCES assets(id),
    ADD CONSTRAINT fk_wallet_withdrawal_reviewer FOREIGN KEY (reviewed_by) REFERENCES admin_users(id),
    ADD CONSTRAINT fk_wallet_withdrawal_broadcaster FOREIGN KEY (broadcasted_by) REFERENCES admin_users(id),
    ADD CONSTRAINT fk_wallet_withdrawal_confirmer FOREIGN KEY (confirmed_by) REFERENCES admin_users(id),
    ADD CONSTRAINT fk_wallet_withdrawal_failure_admin FOREIGN KEY (failed_by) REFERENCES admin_users(id);

ALTER TABLE deposit_network_configs
    ADD COLUMN required_confirmations INT UNSIGNED NOT NULL DEFAULT 12 AFTER sort_order;

-- 网关使用统一 HTTP 契约接入具体公链适配器，业务服务不保存私钥。
CREATE TABLE wallet_chain_gateways (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    network VARCHAR(64) NOT NULL,
    broadcast_url VARCHAR(1000) NULL,
    event_poll_url VARCHAR(1000) NULL,
    auth_token_encrypted TEXT NULL,
    status VARCHAR(16) NOT NULL DEFAULT 'disabled',
    last_deposit_cursor VARCHAR(500) NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    UNIQUE KEY uq_wallet_chain_gateways_network (network),
    INDEX idx_wallet_chain_gateways_status (status)
);

CREATE TABLE wallet_deposit_events (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    user_id BIGINT UNSIGNED NOT NULL,
    asset_id BIGINT UNSIGNED NOT NULL,
    asset_symbol VARCHAR(32) NOT NULL,
    network VARCHAR(64) NOT NULL,
    address VARCHAR(255) NOT NULL,
    memo VARCHAR(255) NULL,
    tx_hash VARCHAR(255) NOT NULL,
    event_index INT UNSIGNED NOT NULL DEFAULT 0,
    amount DECIMAL(38,18) NOT NULL,
    block_height BIGINT UNSIGNED NULL,
    confirmations INT UNSIGNED NOT NULL DEFAULT 0,
    required_confirmations INT UNSIGNED NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'observed',
    credited_at TIMESTAMP(6) NULL,
    reversed_at TIMESTAMP(6) NULL,
    failure_reason VARCHAR(500) NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    UNIQUE KEY uq_wallet_deposit_external_event (network, tx_hash, event_index),
    INDEX idx_wallet_deposit_user_time (user_id, created_at),
    INDEX idx_wallet_deposit_status (status, confirmations, id),
    CONSTRAINT fk_wallet_deposit_user FOREIGN KEY (user_id) REFERENCES users(id),
    CONSTRAINT fk_wallet_deposit_asset FOREIGN KEY (asset_id) REFERENCES assets(id)
);
