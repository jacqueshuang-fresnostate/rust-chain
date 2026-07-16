CREATE TABLE country_configs (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    country_code VARCHAR(8) NOT NULL UNIQUE,
    country_name VARCHAR(128) NOT NULL,
    default_locale VARCHAR(16) NOT NULL,
    supported_locales JSON NOT NULL,
    registration_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    sort_order INT NOT NULL DEFAULT 0,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    INDEX idx_country_configs_registration (registration_enabled, status, sort_order),
    INDEX idx_country_configs_status (status)
);

ALTER TABLE users
    ADD COLUMN country_code VARCHAR(8) NULL AFTER phone,
    ADD COLUMN preferred_locale VARCHAR(16) NULL AFTER country_code,
    ADD INDEX idx_users_country_code (country_code);

INSERT INTO country_configs
    (country_code, country_name, default_locale, supported_locales, registration_enabled, status, sort_order)
VALUES
    ('CN', 'China', 'zh', '["zh", "en"]', TRUE, 'active', 10),
    ('US', 'United States', 'en', '["en"]', TRUE, 'active', 20);
