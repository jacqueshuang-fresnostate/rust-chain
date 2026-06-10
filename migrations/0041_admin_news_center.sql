CREATE TABLE admin_news_items (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    title VARCHAR(255) NOT NULL,
    category VARCHAR(64) NOT NULL,
    status VARCHAR(32) NOT NULL,
    country_code VARCHAR(16) NULL,
    default_locale VARCHAR(16) NOT NULL,
    content_json JSON NOT NULL,
    published_at TIMESTAMP(6) NULL,
    created_by_admin_id BIGINT UNSIGNED NULL,
    updated_by_admin_id BIGINT UNSIGNED NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    INDEX idx_admin_news_status_country_published (status, country_code, published_at),
    INDEX idx_admin_news_category_status (category, status),
    INDEX idx_admin_news_updated_at (updated_at)
);
