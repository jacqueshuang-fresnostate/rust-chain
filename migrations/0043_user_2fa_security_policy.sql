CREATE TABLE IF NOT EXISTS user_two_factor_settings (
    user_id BIGINT UNSIGNED NOT NULL PRIMARY KEY,
    totp_secret_encrypted TEXT NULL,
    totp_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    login_2fa_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    confirmed_at TIMESTAMP NULL,
    last_verified_at TIMESTAMP NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    CONSTRAINT fk_user_two_factor_settings_user FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS security_policy_configs (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    policy_key VARCHAR(64) NOT NULL UNIQUE,
    policy_value JSON NOT NULL,
    updated_by BIGINT UNSIGNED NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    INDEX idx_security_policy_configs_updated_by (updated_by)
);

INSERT INTO security_policy_configs (policy_key, policy_value)
VALUES (
    'user_security_policy',
    JSON_OBJECT(
        'login_2fa_mode', 'user_enabled',
        'payment_policies', JSON_OBJECT(
            'withdraw', JSON_OBJECT('enabled', TRUE, 'method', 'fund_password'),
            'spot_order', JSON_OBJECT('enabled', FALSE, 'method', 'fund_password'),
            'convert', JSON_OBJECT('enabled', FALSE, 'method', 'fund_password'),
            'earn_subscribe', JSON_OBJECT('enabled', FALSE, 'method', 'fund_password')
        )
    )
)
ON DUPLICATE KEY UPDATE policy_key = policy_key;

CREATE TABLE IF NOT EXISTS login_two_factor_challenges (
    challenge_id CHAR(36) NOT NULL PRIMARY KEY,
    user_id BIGINT UNSIGNED NOT NULL,
    challenge_type VARCHAR(32) NOT NULL,
    expires_at TIMESTAMP NOT NULL,
    consumed_at TIMESTAMP NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_login_two_factor_challenges_user (user_id),
    INDEX idx_login_two_factor_challenges_expires_at (expires_at),
    CONSTRAINT fk_login_two_factor_challenges_user FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS wallet_withdrawal_requests (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    user_id BIGINT UNSIGNED NOT NULL,
    asset_symbol VARCHAR(32) NOT NULL,
    network VARCHAR(64) NULL,
    address VARCHAR(255) NOT NULL,
    amount DECIMAL(36, 18) NOT NULL,
    fee DECIMAL(36, 18) NOT NULL DEFAULT 0,
    status VARCHAR(32) NOT NULL DEFAULT 'pending',
    security_method VARCHAR(64) NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    INDEX idx_wallet_withdrawal_requests_user (user_id),
    INDEX idx_wallet_withdrawal_requests_status (status),
    CONSTRAINT fk_wallet_withdrawal_requests_user FOREIGN KEY (user_id) REFERENCES users(id)
);
