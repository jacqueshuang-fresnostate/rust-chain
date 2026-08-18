-- 管理端 P0 治理底座：完整审计关联、配置乐观并发与高风险变更复核。

ALTER TABLE admin_audit_logs
    ADD COLUMN request_id VARCHAR(64) NULL COMMENT '请求关联ID' AFTER ip,
    ADD INDEX idx_admin_audit_logs_request_id (request_id);

-- 首个管理员的默认角色必须显式持有全权限；空集合不再被应用解释为隐式超级管理员。
UPDATE admin_roles
SET permissions = JSON_ARRAY('*')
WHERE name = 'super_admin'
  AND JSON_LENGTH(permissions) = 0;

ALTER TABLE prediction_settings
    ADD COLUMN revision BIGINT UNSIGNED NOT NULL DEFAULT 1 COMMENT '配置乐观并发版本' AFTER quote_ttl_seconds;

ALTER TABLE prediction_asset_configs
    ADD COLUMN revision BIGINT UNSIGNED NOT NULL DEFAULT 1 COMMENT '配置乐观并发版本' AFTER max_payout_amount;

ALTER TABLE loan_products
    ADD COLUMN revision BIGINT UNSIGNED NOT NULL DEFAULT 1 COMMENT '配置乐观并发版本' AFTER status;

CREATE TABLE admin_config_change_requests (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT COMMENT '变更申请ID',
    request_no VARCHAR(64) NOT NULL COMMENT '对外稳定申请号',
    config_domain VARCHAR(64) NOT NULL COMMENT '配置业务域',
    target_type VARCHAR(64) NOT NULL COMMENT '目标资源类型',
    target_id VARCHAR(64) NOT NULL COMMENT '目标资源标识',
    action VARCHAR(64) NOT NULL COMMENT '请求动作',
    base_revision BIGINT UNSIGNED NULL COMMENT '制作时的基线版本',
    before_json JSON NULL COMMENT '变更前脱敏快照',
    proposed_json JSON NOT NULL COMMENT '待复核的脱敏配置',
    reason VARCHAR(512) NOT NULL COMMENT '制作原因',
    risk_level VARCHAR(16) NOT NULL DEFAULT 'high' COMMENT '风险等级',
    status VARCHAR(32) NOT NULL DEFAULT 'pending' COMMENT '待复核、通过、拒绝、已应用或失效',
    created_by BIGINT UNSIGNED NOT NULL COMMENT '制作管理员ID',
    reviewed_by BIGINT UNSIGNED NULL COMMENT '复核管理员ID',
    review_reason VARCHAR(512) NULL COMMENT '复核意见',
    applied_by BIGINT UNSIGNED NULL COMMENT '执行管理员ID',
    reviewed_at TIMESTAMP(6) NULL COMMENT '复核时间',
    applied_at TIMESTAMP(6) NULL COMMENT '应用时间',
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) COMMENT '创建时间',
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6) COMMENT '更新时间',
    UNIQUE KEY uk_admin_config_change_requests_no (request_no),
    INDEX idx_admin_config_change_requests_status_time (status, created_at),
    INDEX idx_admin_config_change_requests_target (target_type, target_id, created_at),
    INDEX idx_admin_config_change_requests_creator (created_by, created_at),
    CONSTRAINT fk_admin_config_change_requests_creator FOREIGN KEY (created_by) REFERENCES admin_users(id),
    CONSTRAINT fk_admin_config_change_requests_reviewer FOREIGN KEY (reviewed_by) REFERENCES admin_users(id),
    CONSTRAINT fk_admin_config_change_requests_applier FOREIGN KEY (applied_by) REFERENCES admin_users(id),
    CONSTRAINT chk_admin_config_change_requests_risk CHECK (risk_level IN ('low', 'medium', 'high', 'critical')),
    CONSTRAINT chk_admin_config_change_requests_status CHECK (status IN ('pending', 'approved', 'rejected', 'applied', 'expired'))
) COMMENT='高风险后台配置变更双人复核';
