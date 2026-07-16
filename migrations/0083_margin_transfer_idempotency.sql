CREATE TABLE margin_transfers (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    transfer_id VARCHAR(128) NOT NULL,
    user_id BIGINT UNSIGNED NOT NULL,
    asset_id BIGINT UNSIGNED NOT NULL,
    from_account VARCHAR(16) NOT NULL,
    to_account VARCHAR(16) NOT NULL,
    amount DECIMAL(38,18) NOT NULL,
    idempotency_key VARCHAR(128) NOT NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    UNIQUE KEY uq_margin_transfers_transfer_id (transfer_id),
    UNIQUE KEY uq_margin_transfers_user_key (user_id, idempotency_key),
    INDEX idx_margin_transfers_user_time (user_id, created_at),
    CONSTRAINT fk_margin_transfers_user FOREIGN KEY (user_id) REFERENCES users(id),
    CONSTRAINT fk_margin_transfers_asset FOREIGN KEY (asset_id) REFERENCES assets(id),
    CONSTRAINT chk_margin_transfers_accounts
        CHECK (from_account IN ('spot', 'margin')
           AND to_account IN ('spot', 'margin')
           AND from_account <> to_account),
    CONSTRAINT chk_margin_transfers_amount CHECK (amount > 0)
);
