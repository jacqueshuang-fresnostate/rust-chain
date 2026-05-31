CREATE TABLE assets (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    symbol VARCHAR(32) NOT NULL UNIQUE,
    name VARCHAR(128) NOT NULL,
    precision_scale INT NOT NULL DEFAULT 8,
    asset_type VARCHAR(32) NOT NULL DEFAULT 'coin',
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6)
);

CREATE TABLE wallet_accounts (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    user_id BIGINT UNSIGNED NOT NULL,
    asset_id BIGINT UNSIGNED NOT NULL,
    available DECIMAL(38,18) NOT NULL DEFAULT 0,
    frozen DECIMAL(38,18) NOT NULL DEFAULT 0,
    locked DECIMAL(38,18) NOT NULL DEFAULT 0,
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    UNIQUE KEY uq_wallet_accounts_user_asset (user_id, asset_id),
    CONSTRAINT fk_wallet_accounts_user FOREIGN KEY (user_id) REFERENCES users(id),
    CONSTRAINT fk_wallet_accounts_asset FOREIGN KEY (asset_id) REFERENCES assets(id),
    CONSTRAINT chk_wallet_accounts_non_negative CHECK (available >= 0 AND frozen >= 0 AND locked >= 0)
);

CREATE TABLE wallet_ledger (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    user_id BIGINT UNSIGNED NOT NULL,
    asset_id BIGINT UNSIGNED NOT NULL,
    change_type VARCHAR(64) NOT NULL,
    amount DECIMAL(38,18) NOT NULL,
    balance_type VARCHAR(32) NOT NULL,
    balance_after DECIMAL(38,18) NOT NULL,
    available_after DECIMAL(38,18) NOT NULL,
    frozen_after DECIMAL(38,18) NOT NULL,
    locked_after DECIMAL(38,18) NOT NULL,
    ref_type VARCHAR(64) NOT NULL,
    ref_id VARCHAR(128) NOT NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    INDEX idx_wallet_ledger_user_asset_time (user_id, asset_id, created_at),
    INDEX idx_wallet_ledger_ref (ref_type, ref_id),
    CONSTRAINT fk_wallet_ledger_user FOREIGN KEY (user_id) REFERENCES users(id),
    CONSTRAINT fk_wallet_ledger_asset FOREIGN KEY (asset_id) REFERENCES assets(id)
);

CREATE TABLE asset_lock_positions (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    user_id BIGINT UNSIGNED NOT NULL,
    asset_id BIGINT UNSIGNED NOT NULL,
    unlock_type VARCHAR(32) NOT NULL,
    unlock_at TIMESTAMP(6) NOT NULL,
    locked_amount DECIMAL(38,18) NOT NULL,
    released_amount DECIMAL(38,18) NOT NULL DEFAULT 0,
    remaining_amount DECIMAL(38,18) NOT NULL,
    merge_key VARCHAR(255) NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    UNIQUE KEY uq_asset_lock_positions_merge_key (merge_key),
    INDEX idx_asset_lock_positions_due (status, unlock_at),
    INDEX idx_asset_lock_positions_user_asset (user_id, asset_id),
    CONSTRAINT fk_asset_lock_positions_user FOREIGN KEY (user_id) REFERENCES users(id),
    CONSTRAINT fk_asset_lock_positions_asset FOREIGN KEY (asset_id) REFERENCES assets(id),
    CONSTRAINT chk_asset_lock_positions_amounts CHECK (locked_amount >= 0 AND released_amount >= 0 AND remaining_amount >= 0)
);

CREATE TABLE asset_lock_position_sources (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    lock_position_id BIGINT UNSIGNED NOT NULL,
    source_type VARCHAR(64) NOT NULL,
    source_id VARCHAR(128) NOT NULL,
    source_amount DECIMAL(38,18) NOT NULL,
    source_time TIMESTAMP(6) NOT NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    UNIQUE KEY uq_asset_lock_position_sources_source (source_type, source_id),
    INDEX idx_asset_lock_position_sources_position (lock_position_id),
    CONSTRAINT fk_asset_lock_position_sources_position FOREIGN KEY (lock_position_id) REFERENCES asset_lock_positions(id),
    CONSTRAINT chk_asset_lock_position_sources_amount CHECK (source_amount > 0)
);

CREATE TABLE asset_unlock_records (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    user_id BIGINT UNSIGNED NOT NULL,
    asset_id BIGINT UNSIGNED NOT NULL,
    lock_position_id BIGINT UNSIGNED NOT NULL,
    unlock_quantity DECIMAL(38,18) NOT NULL,
    unlock_price DECIMAL(38,18) NULL,
    unlock_fee_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    unlock_fee_rate DECIMAL(18,8) NULL,
    unlock_fee_basis VARCHAR(32) NULL,
    unlock_fee_asset BIGINT UNSIGNED NULL,
    unlock_fee_amount DECIMAL(38,18) NULL,
    fee_paid_status VARCHAR(32) NOT NULL DEFAULT 'not_required',
    status VARCHAR(32) NOT NULL DEFAULT 'pending',
    idempotency_key VARCHAR(255) NOT NULL UNIQUE,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    INDEX idx_asset_unlock_records_user_asset (user_id, asset_id),
    CONSTRAINT fk_asset_unlock_records_user FOREIGN KEY (user_id) REFERENCES users(id),
    CONSTRAINT fk_asset_unlock_records_asset FOREIGN KEY (asset_id) REFERENCES assets(id),
    CONSTRAINT fk_asset_unlock_records_position FOREIGN KEY (lock_position_id) REFERENCES asset_lock_positions(id),
    CONSTRAINT fk_asset_unlock_records_fee_asset FOREIGN KEY (unlock_fee_asset) REFERENCES assets(id)
);

CREATE TABLE deposit_records (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    user_id BIGINT UNSIGNED NOT NULL,
    asset_id BIGINT UNSIGNED NOT NULL,
    amount DECIMAL(38,18) NOT NULL,
    tx_hash VARCHAR(255) NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'pending',
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    CONSTRAINT fk_deposit_records_user FOREIGN KEY (user_id) REFERENCES users(id),
    CONSTRAINT fk_deposit_records_asset FOREIGN KEY (asset_id) REFERENCES assets(id)
);

CREATE TABLE withdraw_records (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    user_id BIGINT UNSIGNED NOT NULL,
    asset_id BIGINT UNSIGNED NOT NULL,
    amount DECIMAL(38,18) NOT NULL,
    fee DECIMAL(38,18) NOT NULL DEFAULT 0,
    address VARCHAR(255) NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'pending',
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    CONSTRAINT fk_withdraw_records_user FOREIGN KEY (user_id) REFERENCES users(id),
    CONSTRAINT fk_withdraw_records_asset FOREIGN KEY (asset_id) REFERENCES assets(id)
);
