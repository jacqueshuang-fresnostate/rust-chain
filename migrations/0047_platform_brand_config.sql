CREATE TABLE IF NOT EXISTS platform_brand_configs (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    name VARCHAR(64) NOT NULL UNIQUE,
    platform_name VARCHAR(128) NOT NULL,
    logo_url TEXT NULL,
    updated_by BIGINT UNSIGNED NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    INDEX idx_platform_brand_configs_updated_by (updated_by),
    CONSTRAINT fk_platform_brand_configs_updated_by FOREIGN KEY (updated_by) REFERENCES admin_users(id)
);

INSERT INTO platform_brand_configs (
    name,
    platform_name,
    logo_url
)
VALUES (
    'default',
    'Hippo Exchange',
    NULL
)
ON DUPLICATE KEY UPDATE name = name;
