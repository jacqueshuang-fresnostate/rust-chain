ALTER TABLE margin_positions
    ADD COLUMN wallet_scope VARCHAR(16) NOT NULL DEFAULT 'spot' AFTER margin_asset,
    ADD CONSTRAINT chk_margin_positions_wallet_scope CHECK (wallet_scope IN ('spot', 'margin'));

CREATE TABLE margin_wallet_accounts (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    user_id BIGINT UNSIGNED NOT NULL,
    asset_id BIGINT UNSIGNED NOT NULL,
    available DECIMAL(38,18) NOT NULL DEFAULT 0,
    frozen DECIMAL(38,18) NOT NULL DEFAULT 0,
    locked DECIMAL(38,18) NOT NULL DEFAULT 0,
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    UNIQUE KEY uq_margin_wallet_accounts_user_asset (user_id, asset_id),
    CONSTRAINT fk_margin_wallet_accounts_user FOREIGN KEY (user_id) REFERENCES users(id),
    CONSTRAINT fk_margin_wallet_accounts_asset FOREIGN KEY (asset_id) REFERENCES assets(id),
    CONSTRAINT chk_margin_wallet_accounts_non_negative CHECK (available >= 0 AND frozen >= 0 AND locked >= 0)
);

CREATE TABLE margin_wallet_ledger (
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
    INDEX idx_margin_wallet_ledger_user_asset_time (user_id, asset_id, created_at),
    INDEX idx_margin_wallet_ledger_ref (ref_type, ref_id),
    CONSTRAINT fk_margin_wallet_ledger_user FOREIGN KEY (user_id) REFERENCES users(id),
    CONSTRAINT fk_margin_wallet_ledger_asset FOREIGN KEY (asset_id) REFERENCES assets(id)
);

CREATE TABLE margin_user_settings (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    user_id BIGINT UNSIGNED NOT NULL,
    product_id BIGINT UNSIGNED NOT NULL,
    margin_mode VARCHAR(16) NULL,
    leverage DECIMAL(18,8) NULL,
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    UNIQUE KEY uq_margin_user_settings_user_product (user_id, product_id),
    CONSTRAINT fk_margin_user_settings_user FOREIGN KEY (user_id) REFERENCES users(id),
    CONSTRAINT fk_margin_user_settings_product FOREIGN KEY (product_id) REFERENCES margin_products(id),
    CONSTRAINT chk_margin_user_settings_margin_mode CHECK (margin_mode IS NULL OR margin_mode IN ('isolated', 'cross')),
    CONSTRAINT chk_margin_user_settings_leverage CHECK (leverage IS NULL OR leverage > 0)
);
