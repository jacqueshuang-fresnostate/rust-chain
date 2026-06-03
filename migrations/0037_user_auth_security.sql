ALTER TABLE users
    ADD COLUMN email_verified_at TIMESTAMP(6) NULL AFTER email;

CREATE TABLE user_email_verifications (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    user_id BIGINT UNSIGNED NOT NULL,
    email VARCHAR(255) NOT NULL,
    purpose VARCHAR(32) NOT NULL,
    code_hash VARCHAR(255) NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'pending',
    attempt_count INT NOT NULL DEFAULT 0,
    expires_at TIMESTAMP(6) NOT NULL,
    sent_at TIMESTAMP(6) NOT NULL,
    verified_at TIMESTAMP(6) NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    INDEX idx_user_email_verifications_user_status (user_id, purpose, status, created_at),
    INDEX idx_user_email_verifications_email_status (email, purpose, status),
    INDEX idx_user_email_verifications_expires (status, expires_at),
    CONSTRAINT fk_user_email_verifications_user FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE TABLE smtp_configs (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    name VARCHAR(64) NOT NULL UNIQUE,
    host VARCHAR(255) NOT NULL,
    port INT UNSIGNED NOT NULL,
    security VARCHAR(32) NOT NULL,
    username_ciphertext TEXT NULL,
    password_ciphertext TEXT NULL,
    username_mask VARCHAR(64) NULL,
    from_email VARCHAR(255) NOT NULL,
    from_name VARCHAR(128) NULL,
    enabled BOOLEAN NOT NULL DEFAULT FALSE,
    updated_by BIGINT UNSIGNED NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    INDEX idx_smtp_configs_enabled (enabled),
    CONSTRAINT fk_smtp_configs_updated_by FOREIGN KEY (updated_by) REFERENCES admin_users(id)
);
