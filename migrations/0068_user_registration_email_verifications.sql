CREATE TABLE user_registration_email_verifications (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT COMMENT '注册邮箱验证码ID',
    email VARCHAR(255) NOT NULL COMMENT '注册邮箱',
    purpose VARCHAR(32) NOT NULL DEFAULT 'register' COMMENT '验证码用途',
    code_hash VARCHAR(255) NOT NULL COMMENT '验证码哈希',
    status VARCHAR(32) NOT NULL DEFAULT 'pending' COMMENT '验证码状态：pending待验证，verified已验证，superseded已失效',
    attempt_count INT NOT NULL DEFAULT 0 COMMENT '验证尝试次数',
    expires_at TIMESTAMP(6) NOT NULL COMMENT '过期时间',
    sent_at TIMESTAMP(6) NOT NULL COMMENT '发送时间',
    verified_at TIMESTAMP(6) NULL COMMENT '验证完成时间',
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) COMMENT '创建时间',
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6) COMMENT '更新时间',
    INDEX idx_user_registration_email_verifications_email_status (email, purpose, status, sent_at),
    INDEX idx_user_registration_email_verifications_expires (status, expires_at)
) COMMENT='用户注册邮箱验证码';
