CREATE TABLE market_feed_configs (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    name VARCHAR(64) NOT NULL UNIQUE,
    symbols_json JSON NOT NULL,
    intervals_json JSON NOT NULL,
    providers_json JSON NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    version BIGINT UNSIGNED NOT NULL DEFAULT 1,
    applied_version BIGINT UNSIGNED NULL,
    last_reload_status VARCHAR(32) NULL,
    last_reload_error TEXT NULL,
    last_reloaded_at TIMESTAMP(6) NULL,
    updated_by BIGINT UNSIGNED NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    INDEX idx_market_feed_configs_enabled (enabled),
    CONSTRAINT fk_market_feed_configs_updated_by FOREIGN KEY (updated_by) REFERENCES admin_users(id)
);

CREATE TABLE market_source_credentials (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    provider VARCHAR(32) NOT NULL UNIQUE,
    auth_type VARCHAR(32) NOT NULL DEFAULT 'none',
    api_key_ciphertext TEXT NULL,
    api_secret_ciphertext TEXT NULL,
    passphrase_ciphertext TEXT NULL,
    api_key_mask VARCHAR(64) NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    updated_by BIGINT UNSIGNED NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    INDEX idx_market_source_credentials_enabled (enabled),
    CONSTRAINT fk_market_source_credentials_updated_by FOREIGN KEY (updated_by) REFERENCES admin_users(id)
);
