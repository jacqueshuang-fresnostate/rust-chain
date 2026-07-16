CREATE TABLE IF NOT EXISTS kyc_configs (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    name VARCHAR(64) NOT NULL UNIQUE,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    target_kyc_level INT NOT NULL DEFAULT 1,
    required_documents_json JSON NOT NULL,
    allowed_countries_json JSON NOT NULL,
    max_document_size_bytes BIGINT UNSIGNED NOT NULL DEFAULT 5242880,
    updated_by BIGINT UNSIGNED NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    INDEX idx_kyc_configs_updated_by (updated_by),
    CONSTRAINT fk_kyc_configs_updated_by FOREIGN KEY (updated_by) REFERENCES admin_users(id)
);

INSERT INTO kyc_configs (
    name,
    enabled,
    target_kyc_level,
    required_documents_json,
    allowed_countries_json,
    max_document_size_bytes
)
VALUES (
    'default',
    TRUE,
    1,
    JSON_ARRAY('identity_front', 'identity_back'),
    JSON_ARRAY(),
    5242880
)
ON DUPLICATE KEY UPDATE name = name;

CREATE TABLE IF NOT EXISTS user_kyc_submissions (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    user_id BIGINT UNSIGNED NOT NULL,
    real_name VARCHAR(128) NOT NULL,
    country VARCHAR(128) NOT NULL,
    id_number VARCHAR(128) NOT NULL,
    document_type VARCHAR(64) NOT NULL DEFAULT 'identity_card',
    document_front_image MEDIUMTEXT NOT NULL,
    document_back_image MEDIUMTEXT NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'pending',
    target_kyc_level INT NOT NULL DEFAULT 1,
    reviewed_by BIGINT UNSIGNED NULL,
    review_reason VARCHAR(512) NULL,
    submitted_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    reviewed_at TIMESTAMP(6) NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    INDEX idx_user_kyc_submissions_user_status (user_id, status, submitted_at),
    INDEX idx_user_kyc_submissions_status_time (status, submitted_at),
    INDEX idx_user_kyc_submissions_reviewed_by (reviewed_by),
    CONSTRAINT fk_user_kyc_submissions_user FOREIGN KEY (user_id) REFERENCES users(id),
    CONSTRAINT fk_user_kyc_submissions_reviewed_by FOREIGN KEY (reviewed_by) REFERENCES admin_users(id)
);
