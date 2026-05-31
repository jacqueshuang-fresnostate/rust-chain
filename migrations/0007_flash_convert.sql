CREATE TABLE convert_pairs (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    from_asset BIGINT UNSIGNED NOT NULL,
    to_asset BIGINT UNSIGNED NOT NULL,
    pricing_mode VARCHAR(32) NOT NULL,
    spread_rate DECIMAL(18,8) NOT NULL DEFAULT 0,
    min_amount DECIMAL(38,18) NOT NULL DEFAULT 0,
    max_amount DECIMAL(38,18) NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    UNIQUE KEY uq_convert_pairs_assets (from_asset, to_asset),
    CONSTRAINT fk_convert_pairs_from_asset FOREIGN KEY (from_asset) REFERENCES assets(id),
    CONSTRAINT fk_convert_pairs_to_asset FOREIGN KEY (to_asset) REFERENCES assets(id),
    CONSTRAINT chk_convert_pairs_amounts CHECK (min_amount >= 0 AND (max_amount IS NULL OR max_amount >= min_amount)),
    CONSTRAINT chk_convert_pairs_spread CHECK (spread_rate >= 0)
);

CREATE TABLE new_coin_convert_rules (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    convert_pair_id BIGINT UNSIGNED NOT NULL UNIQUE,
    rate_source VARCHAR(32) NOT NULL,
    fixed_rate DECIMAL(38,18) NULL,
    floating_rate_json JSON NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    created_by BIGINT UNSIGNED NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    CONSTRAINT fk_new_coin_convert_rules_pair FOREIGN KEY (convert_pair_id) REFERENCES convert_pairs(id),
    CONSTRAINT fk_new_coin_convert_rules_admin FOREIGN KEY (created_by) REFERENCES admin_users(id),
    CONSTRAINT chk_new_coin_convert_rules_fixed_rate CHECK (fixed_rate IS NULL OR fixed_rate > 0)
);

CREATE TABLE convert_quotes (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    quote_id VARCHAR(128) NOT NULL UNIQUE,
    convert_pair_id BIGINT UNSIGNED NOT NULL,
    user_id BIGINT UNSIGNED NOT NULL,
    from_asset BIGINT UNSIGNED NOT NULL,
    to_asset BIGINT UNSIGNED NOT NULL,
    from_amount DECIMAL(38,18) NOT NULL,
    to_amount DECIMAL(38,18) NOT NULL,
    rate DECIMAL(38,18) NOT NULL,
    spread_rate DECIMAL(18,8) NOT NULL DEFAULT 0,
    expires_at TIMESTAMP(6) NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'quoted',
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    INDEX idx_convert_quotes_user_time (user_id, created_at),
    INDEX idx_convert_quotes_expires (expires_at),
    CONSTRAINT fk_convert_quotes_pair FOREIGN KEY (convert_pair_id) REFERENCES convert_pairs(id),
    CONSTRAINT fk_convert_quotes_user FOREIGN KEY (user_id) REFERENCES users(id),
    CONSTRAINT fk_convert_quotes_from_asset FOREIGN KEY (from_asset) REFERENCES assets(id),
    CONSTRAINT fk_convert_quotes_to_asset FOREIGN KEY (to_asset) REFERENCES assets(id),
    CONSTRAINT chk_convert_quotes_amounts CHECK (from_amount > 0 AND to_amount > 0 AND rate > 0)
);

CREATE TABLE convert_orders (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    quote_id VARCHAR(128) NOT NULL UNIQUE,
    convert_pair_id BIGINT UNSIGNED NOT NULL,
    user_id BIGINT UNSIGNED NOT NULL,
    from_asset BIGINT UNSIGNED NOT NULL,
    to_asset BIGINT UNSIGNED NOT NULL,
    from_amount DECIMAL(38,18) NOT NULL,
    to_amount DECIMAL(38,18) NOT NULL,
    rate DECIMAL(38,18) NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'pending',
    error_message TEXT NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    INDEX idx_convert_orders_user_time (user_id, created_at),
    INDEX idx_convert_orders_status (status, created_at),
    CONSTRAINT fk_convert_orders_pair FOREIGN KEY (convert_pair_id) REFERENCES convert_pairs(id),
    CONSTRAINT fk_convert_orders_user FOREIGN KEY (user_id) REFERENCES users(id),
    CONSTRAINT fk_convert_orders_from_asset FOREIGN KEY (from_asset) REFERENCES assets(id),
    CONSTRAINT fk_convert_orders_to_asset FOREIGN KEY (to_asset) REFERENCES assets(id),
    CONSTRAINT chk_convert_orders_amounts CHECK (from_amount > 0 AND to_amount > 0 AND rate > 0)
);

CREATE TABLE convert_events (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    convert_order_id BIGINT UNSIGNED NOT NULL,
    event_type VARCHAR(64) NOT NULL,
    payload_json JSON NOT NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    INDEX idx_convert_events_order_time (convert_order_id, created_at),
    CONSTRAINT fk_convert_events_order FOREIGN KEY (convert_order_id) REFERENCES convert_orders(id)
);
