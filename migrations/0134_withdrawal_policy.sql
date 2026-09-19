CREATE TABLE wallet_withdrawal_policies (
    asset_id BIGINT UNSIGNED PRIMARY KEY,
    revision BIGINT UNSIGNED NOT NULL,
    config_json JSON NOT NULL,
    updated_by BIGINT UNSIGNED NOT NULL,
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    CONSTRAINT fk_withdrawal_policy_asset FOREIGN KEY (asset_id) REFERENCES assets(id)
);

CREATE TABLE wallet_withdrawal_addresses (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    user_id BIGINT UNSIGNED NOT NULL,
    network VARCHAR(64) NOT NULL,
    address VARCHAR(255) NOT NULL,
    identity_hash CHAR(64) NOT NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    UNIQUE KEY uq_withdrawal_address_identity (user_id, identity_hash),
    CONSTRAINT fk_withdrawal_address_user FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE TABLE wallet_withdrawal_policy_receipts (
    withdrawal_id BIGINT UNSIGNED PRIMARY KEY,
    policy_revision BIGINT UNSIGNED NOT NULL,
    config_json JSON NOT NULL,
    required_approvals INT UNSIGNED NOT NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    CONSTRAINT fk_withdrawal_policy_receipt FOREIGN KEY (withdrawal_id) REFERENCES wallet_withdrawal_requests(id),
    CONSTRAINT chk_withdrawal_review_count CHECK (required_approvals BETWEEN 1 AND 20)
);

CREATE TABLE wallet_withdrawal_reviews (
    withdrawal_id BIGINT UNSIGNED NOT NULL,
    admin_id BIGINT UNSIGNED NOT NULL,
    reason VARCHAR(512) NOT NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    PRIMARY KEY (withdrawal_id, admin_id),
    CONSTRAINT fk_withdrawal_review_request FOREIGN KEY (withdrawal_id) REFERENCES wallet_withdrawal_requests(id)
);

CREATE INDEX idx_withdrawal_allowance ON wallet_withdrawal_requests(user_id, asset_id, created_at);

-- Existing row modification times are conservative upper bounds, not invented
-- historical credential-change times. New credential changes have exact clocks.
ALTER TABLE users ADD COLUMN withdrawal_security_changed_at TIMESTAMP(6) NULL;
UPDATE users SET withdrawal_security_changed_at = updated_at;
ALTER TABLE user_security ADD COLUMN withdrawal_security_changed_at TIMESTAMP(6) NULL;
UPDATE user_security SET withdrawal_security_changed_at = updated_at;
ALTER TABLE user_two_factor_settings ADD COLUMN withdrawal_security_changed_at TIMESTAMP(6) NULL;
UPDATE user_two_factor_settings SET withdrawal_security_changed_at = updated_at;

CREATE TRIGGER withdrawal_users_security_clock BEFORE UPDATE ON users FOR EACH ROW
SET NEW.withdrawal_security_changed_at =
    IF(NOT (BINARY OLD.password_hash <=> BINARY NEW.password_hash)
       OR NOT (BINARY OLD.email <=> BINARY NEW.email)
       OR NOT (BINARY OLD.phone <=> BINARY NEW.phone),
       CURRENT_TIMESTAMP(6), OLD.withdrawal_security_changed_at);

CREATE TRIGGER withdrawal_fund_security_clock BEFORE UPDATE ON user_security FOR EACH ROW
SET NEW.withdrawal_security_changed_at =
    IF(NOT (BINARY OLD.fund_password_hash <=> BINARY NEW.fund_password_hash),
       CURRENT_TIMESTAMP(6), OLD.withdrawal_security_changed_at);

CREATE TRIGGER withdrawal_totp_security_clock BEFORE UPDATE ON user_two_factor_settings FOR EACH ROW
SET NEW.withdrawal_security_changed_at =
    IF(NOT (BINARY OLD.totp_secret_encrypted <=> BINARY NEW.totp_secret_encrypted)
       OR OLD.totp_enabled <> NEW.totp_enabled
       OR OLD.login_2fa_enabled <> NEW.login_2fa_enabled,
       CURRENT_TIMESTAMP(6), OLD.withdrawal_security_changed_at);
