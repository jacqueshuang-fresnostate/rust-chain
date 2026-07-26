CREATE TABLE IF NOT EXISTS login_failure_counters (
    actor_type VARCHAR(16) NOT NULL,
    identifier VARCHAR(191) NOT NULL,
    failure_count INT UNSIGNED NOT NULL DEFAULT 0,
    window_expires_at TIMESTAMP(6) NOT NULL,
    locked_until TIMESTAMP(6) NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    PRIMARY KEY (actor_type, identifier),
    INDEX idx_login_failure_counters_window (window_expires_at)
);

CREATE TABLE IF NOT EXISTS admin_two_factor_settings (
    admin_id BIGINT UNSIGNED NOT NULL PRIMARY KEY,
    totp_secret_encrypted TEXT NULL,
    totp_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    confirmed_at TIMESTAMP(6) NULL,
    last_verified_at TIMESTAMP(6) NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    CONSTRAINT fk_admin_two_factor_settings_admin FOREIGN KEY (admin_id) REFERENCES admin_users(id)
);

CREATE TABLE IF NOT EXISTS admin_login_two_factor_challenges (
    challenge_id CHAR(36) NOT NULL PRIMARY KEY,
    admin_id BIGINT UNSIGNED NOT NULL,
    attempt_count INT UNSIGNED NOT NULL DEFAULT 0,
    expires_at TIMESTAMP(6) NOT NULL,
    consumed_at TIMESTAMP(6) NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    INDEX idx_admin_login_two_factor_challenges_admin (admin_id),
    INDEX idx_admin_login_two_factor_challenges_expires_at (expires_at),
    CONSTRAINT fk_admin_login_two_factor_challenges_admin FOREIGN KEY (admin_id) REFERENCES admin_users(id)
);
