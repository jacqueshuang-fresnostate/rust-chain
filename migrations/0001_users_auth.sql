CREATE TABLE users (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    email VARCHAR(255) NULL UNIQUE,
    phone VARCHAR(32) NULL UNIQUE,
    password_hash VARCHAR(255) NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    kyc_level INT NOT NULL DEFAULT 0,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6)
);

CREATE TABLE user_security (
    user_id BIGINT UNSIGNED PRIMARY KEY,
    fund_password_hash VARCHAR(255) NULL,
    totp_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    anti_phishing_code VARCHAR(64) NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    CONSTRAINT fk_user_security_user FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE TABLE refresh_tokens (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    user_id BIGINT UNSIGNED NULL,
    actor_type VARCHAR(32) NOT NULL,
    actor_id BIGINT UNSIGNED NOT NULL,
    token_hash VARCHAR(255) NOT NULL UNIQUE,
    expires_at TIMESTAMP(6) NOT NULL,
    revoked_at TIMESTAMP(6) NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    INDEX idx_refresh_tokens_actor (actor_type, actor_id),
    INDEX idx_refresh_tokens_expires_at (expires_at),
    CONSTRAINT fk_refresh_tokens_user FOREIGN KEY (user_id) REFERENCES users(id)
);
