CREATE TABLE event_outbox (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    aggregate_type VARCHAR(64) NOT NULL,
    aggregate_id VARCHAR(128) NOT NULL,
    event_type VARCHAR(128) NOT NULL,
    routing_key VARCHAR(128) NOT NULL,
    idempotency_key VARCHAR(255) NOT NULL UNIQUE,
    payload_json JSON NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'pending',
    retry_count INT NOT NULL DEFAULT 0,
    next_retry_at TIMESTAMP(6) NULL,
    published_at TIMESTAMP(6) NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    INDEX idx_event_outbox_status_retry (status, next_retry_at),
    INDEX idx_event_outbox_aggregate (aggregate_type, aggregate_id)
);

CREATE TABLE event_inbox (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    consumer_name VARCHAR(128) NOT NULL,
    message_id VARCHAR(255) NOT NULL,
    idempotency_key VARCHAR(255) NOT NULL,
    payload_hash VARCHAR(128) NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'processing',
    error_message TEXT NULL,
    consumed_at TIMESTAMP(6) NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    UNIQUE KEY uq_event_inbox_consumer_message (consumer_name, message_id),
    UNIQUE KEY uq_event_inbox_consumer_idempotency (consumer_name, idempotency_key),
    INDEX idx_event_inbox_status (status, created_at)
);

CREATE TABLE risk_rules (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    rule_type VARCHAR(64) NOT NULL,
    target_type VARCHAR(64) NOT NULL,
    target_id VARCHAR(128) NULL,
    config_json JSON NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_by BIGINT UNSIGNED NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    INDEX idx_risk_rules_target (rule_type, target_type, target_id, enabled),
    CONSTRAINT fk_risk_rules_admin FOREIGN KEY (created_by) REFERENCES admin_users(id)
);

CREATE TABLE risk_events (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    user_id BIGINT UNSIGNED NULL,
    actor_type VARCHAR(32) NOT NULL,
    actor_id BIGINT UNSIGNED NULL,
    event_type VARCHAR(64) NOT NULL,
    risk_level VARCHAR(32) NOT NULL,
    decision VARCHAR(32) NOT NULL,
    reason VARCHAR(512) NULL,
    payload_json JSON NOT NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    INDEX idx_risk_events_user_time (user_id, created_at),
    INDEX idx_risk_events_decision_time (decision, created_at),
    CONSTRAINT fk_risk_events_user FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE TABLE sensitive_operation_confirmations (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    actor_type VARCHAR(32) NOT NULL,
    actor_id BIGINT UNSIGNED NOT NULL,
    operation_type VARCHAR(128) NOT NULL,
    operation_id VARCHAR(128) NOT NULL,
    confirmation_type VARCHAR(32) NOT NULL,
    confirmed_at TIMESTAMP(6) NULL,
    expires_at TIMESTAMP(6) NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'pending',
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    UNIQUE KEY uq_sensitive_operation_confirmations_operation (actor_type, actor_id, operation_type, operation_id),
    INDEX idx_sensitive_operation_confirmations_expires (status, expires_at)
);

CREATE TABLE audit_events (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    actor_type VARCHAR(32) NOT NULL,
    actor_id BIGINT UNSIGNED NULL,
    action VARCHAR(128) NOT NULL,
    target_type VARCHAR(64) NOT NULL,
    target_id VARCHAR(128) NOT NULL,
    before_json JSON NULL,
    after_json JSON NULL,
    reason VARCHAR(512) NULL,
    ip VARCHAR(64) NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    INDEX idx_audit_events_actor_time (actor_type, actor_id, created_at),
    INDEX idx_audit_events_target_time (target_type, target_id, created_at)
);
