CREATE TABLE admin_wallet_recharges (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    recharge_id VARCHAR(128) NOT NULL,
    admin_id BIGINT UNSIGNED NOT NULL,
    user_id BIGINT UNSIGNED NOT NULL,
    asset_id BIGINT UNSIGNED NOT NULL,
    amount DECIMAL(38,18) NOT NULL,
    reason VARCHAR(512) NOT NULL,
    idempotency_key VARCHAR(128) NOT NULL,
    request_fingerprint CHAR(64) NOT NULL,
    response_snapshot_json JSON NOT NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    UNIQUE KEY uq_admin_wallet_recharges_recharge_id (recharge_id),
    UNIQUE KEY uq_admin_wallet_recharges_admin_key (admin_id, idempotency_key),
    INDEX idx_admin_wallet_recharges_user_time (user_id, created_at),
    INDEX idx_admin_wallet_recharges_asset_time (asset_id, created_at),
    CONSTRAINT fk_admin_wallet_recharges_admin FOREIGN KEY (admin_id) REFERENCES admin_users(id),
    CONSTRAINT fk_admin_wallet_recharges_user FOREIGN KEY (user_id) REFERENCES users(id),
    CONSTRAINT fk_admin_wallet_recharges_asset FOREIGN KEY (asset_id) REFERENCES assets(id),
    CONSTRAINT chk_admin_wallet_recharges_amount CHECK (amount > 0)
);

ALTER TABLE spot_orders
    ADD COLUMN request_fingerprint CHAR(64) NULL AFTER idempotency_key,
    ADD COLUMN idempotency_attempt_token CHAR(36) NULL AFTER request_fingerprint,
    ADD COLUMN idempotency_response_json JSON NULL AFTER idempotency_attempt_token,
    DROP INDEX idempotency_key,
    ADD UNIQUE KEY uq_spot_orders_user_idempotency (user_id, idempotency_key);

ALTER TABLE margin_transfers
    ADD COLUMN request_fingerprint CHAR(64) NULL AFTER idempotency_key;
