ALTER TABLE smtp_configs
    ADD COLUMN priority INT UNSIGNED NOT NULL DEFAULT 100 AFTER enabled;

CREATE TABLE smtp_delivery_settings (
    id TINYINT UNSIGNED PRIMARY KEY,
    strategy VARCHAR(32) NOT NULL DEFAULT 'priority',
    round_robin_cursor BIGINT UNSIGNED NULL,
    updated_by BIGINT UNSIGNED NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    CONSTRAINT fk_smtp_delivery_settings_updated_by FOREIGN KEY (updated_by) REFERENCES admin_users(id)
);

INSERT INTO smtp_delivery_settings (id, strategy)
VALUES (1, 'priority');
