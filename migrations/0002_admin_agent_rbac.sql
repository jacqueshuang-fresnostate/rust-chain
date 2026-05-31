CREATE TABLE admin_roles (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    name VARCHAR(64) NOT NULL UNIQUE,
    permissions JSON NOT NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6)
);

CREATE TABLE admin_users (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    username VARCHAR(64) NOT NULL UNIQUE,
    password_hash VARCHAR(255) NOT NULL,
    role_id BIGINT UNSIGNED NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    CONSTRAINT fk_admin_users_role FOREIGN KEY (role_id) REFERENCES admin_roles(id)
);

CREATE TABLE admin_audit_logs (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    admin_id BIGINT UNSIGNED NOT NULL,
    action VARCHAR(128) NOT NULL,
    target_type VARCHAR(64) NOT NULL,
    target_id VARCHAR(64) NOT NULL,
    before_json JSON NULL,
    after_json JSON NULL,
    reason VARCHAR(512) NULL,
    ip VARCHAR(64) NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    INDEX idx_admin_audit_logs_admin_time (admin_id, created_at),
    CONSTRAINT fk_admin_audit_logs_admin FOREIGN KEY (admin_id) REFERENCES admin_users(id)
);

CREATE TABLE agents (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    user_id BIGINT UNSIGNED NOT NULL UNIQUE,
    agent_code VARCHAR(64) NOT NULL UNIQUE,
    level INT NOT NULL DEFAULT 1,
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    CONSTRAINT fk_agents_user FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE TABLE agent_admin_users (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    agent_id BIGINT UNSIGNED NOT NULL,
    username VARCHAR(64) NOT NULL UNIQUE,
    password_hash VARCHAR(255) NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    last_login_at TIMESTAMP(6) NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    CONSTRAINT fk_agent_admin_users_agent FOREIGN KEY (agent_id) REFERENCES agents(id)
);

CREATE TABLE invite_codes (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    owner_type VARCHAR(32) NOT NULL,
    owner_id BIGINT UNSIGNED NOT NULL,
    code VARCHAR(64) NOT NULL UNIQUE,
    usage_limit INT NULL,
    used_count INT NOT NULL DEFAULT 0,
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    INDEX idx_invite_codes_owner (owner_type, owner_id)
);

CREATE TABLE user_referrals (
    user_id BIGINT UNSIGNED PRIMARY KEY,
    direct_inviter_id BIGINT UNSIGNED NULL,
    direct_inviter_type VARCHAR(32) NULL,
    root_agent_id BIGINT UNSIGNED NULL,
    depth INT NOT NULL DEFAULT 0,
    path VARCHAR(2048) NOT NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    INDEX idx_user_referrals_root_agent (root_agent_id),
    INDEX idx_user_referrals_direct (direct_inviter_type, direct_inviter_id),
    CONSTRAINT fk_user_referrals_user FOREIGN KEY (user_id) REFERENCES users(id),
    CONSTRAINT fk_user_referrals_agent FOREIGN KEY (root_agent_id) REFERENCES agents(id)
);

CREATE TABLE agent_audit_logs (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    agent_id BIGINT UNSIGNED NOT NULL,
    agent_admin_id BIGINT UNSIGNED NOT NULL,
    action VARCHAR(128) NOT NULL,
    target_type VARCHAR(64) NOT NULL,
    target_id VARCHAR(64) NOT NULL,
    ip VARCHAR(64) NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    INDEX idx_agent_audit_logs_agent_time (agent_id, created_at),
    CONSTRAINT fk_agent_audit_logs_agent FOREIGN KEY (agent_id) REFERENCES agents(id),
    CONSTRAINT fk_agent_audit_logs_admin FOREIGN KEY (agent_admin_id) REFERENCES agent_admin_users(id)
);

CREATE TABLE agent_commission_rules (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    agent_id BIGINT UNSIGNED NOT NULL,
    product_type VARCHAR(32) NOT NULL,
    commission_rate DECIMAL(18,8) NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    CONSTRAINT fk_agent_commission_rules_agent FOREIGN KEY (agent_id) REFERENCES agents(id)
);

CREATE TABLE agent_commission_records (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    agent_id BIGINT UNSIGNED NOT NULL,
    user_id BIGINT UNSIGNED NOT NULL,
    source_type VARCHAR(64) NOT NULL,
    source_amount DECIMAL(38,18) NOT NULL,
    commission_amount DECIMAL(38,18) NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'pending',
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    INDEX idx_agent_commission_records_agent (agent_id, status),
    CONSTRAINT fk_agent_commission_records_agent FOREIGN KEY (agent_id) REFERENCES agents(id),
    CONSTRAINT fk_agent_commission_records_user FOREIGN KEY (user_id) REFERENCES users(id)
);
