CREATE TABLE prediction_settings (
    id TINYINT UNSIGNED NOT NULL PRIMARY KEY,
    sync_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    sync_interval_seconds INT UNSIGNED NOT NULL DEFAULT 300,
    sync_tags_json JSON NOT NULL,
    allowed_asset_ids_json JSON NOT NULL,
    default_fee_rate DECIMAL(18,8) NOT NULL DEFAULT 0,
    default_settlement_mode VARCHAR(32) NOT NULL DEFAULT 'manual_confirm',
    default_invalid_refund_policy VARCHAR(32) NOT NULL DEFAULT 'refund_stake_and_fee',
    quote_ttl_seconds INT UNSIGNED NOT NULL DEFAULT 10,
    last_sync_status VARCHAR(32) NULL,
    last_sync_error VARCHAR(512) NULL,
    last_sync_started_at TIMESTAMP(6) NULL,
    last_sync_finished_at TIMESTAMP(6) NULL,
    last_successful_sync_at TIMESTAMP(6) NULL,
    last_sync_imported_count INT UNSIGNED NOT NULL DEFAULT 0,
    last_sync_updated_count INT UNSIGNED NOT NULL DEFAULT 0,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6)
);

INSERT INTO prediction_settings (
    id,
    sync_tags_json,
    allowed_asset_ids_json
) VALUES (
    1,
    JSON_ARRAY(),
    JSON_ARRAY()
);

CREATE TABLE prediction_asset_configs (
    asset_id BIGINT UNSIGNED NOT NULL PRIMARY KEY,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    max_payout_amount DECIMAL(38,18) NOT NULL DEFAULT 0,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    CONSTRAINT fk_prediction_asset_configs_asset FOREIGN KEY (asset_id) REFERENCES assets(id)
);

CREATE TABLE prediction_markets (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    source VARCHAR(32) NOT NULL DEFAULT 'polymarket',
    external_event_id VARCHAR(128) NULL,
    external_market_id VARCHAR(128) NOT NULL,
    slug VARCHAR(255) NULL,
    title VARCHAR(512) NOT NULL,
    description TEXT NULL,
    image_url VARCHAR(1024) NULL,
    category VARCHAR(128) NULL,
    tags_json JSON NOT NULL,
    outcome_yes_label VARCHAR(128) NOT NULL DEFAULT 'Yes',
    outcome_no_label VARCHAR(128) NOT NULL DEFAULT 'No',
    yes_price DECIMAL(18,8) NOT NULL DEFAULT 0.5,
    no_price DECIMAL(18,8) NOT NULL DEFAULT 0.5,
    volume DECIMAL(38,18) NULL,
    liquidity DECIMAL(38,18) NULL,
    end_at TIMESTAMP(6) NULL,
    source_status VARCHAR(32) NOT NULL DEFAULT 'active',
    display_status VARCHAR(32) NOT NULL DEFAULT 'active',
    external_resolution VARCHAR(32) NULL,
    local_resolution VARCHAR(32) NULL,
    invalid_refund_policy_used VARCHAR(32) NULL,
    settlement_status VARCHAR(32) NOT NULL DEFAULT 'open',
    settlement_mode_override VARCHAR(32) NULL,
    allowed_asset_ids_override_json JSON NULL,
    payout_cap_overrides_json JSON NULL,
    fee_rate_override DECIMAL(18,8) NULL,
    sync_payload_json JSON NULL,
    last_synced_at TIMESTAMP(6) NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    UNIQUE KEY uk_prediction_markets_source_external (source, external_market_id),
    INDEX idx_prediction_markets_status (display_status, settlement_status),
    INDEX idx_prediction_markets_source_status (source, source_status),
    INDEX idx_prediction_markets_synced (last_synced_at)
);

CREATE TABLE prediction_sync_logs (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    trigger_type VARCHAR(32) NOT NULL,
    status VARCHAR(32) NOT NULL,
    imported_count INT UNSIGNED NOT NULL DEFAULT 0,
    updated_count INT UNSIGNED NOT NULL DEFAULT 0,
    error_message VARCHAR(512) NULL,
    started_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    finished_at TIMESTAMP(6) NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    INDEX idx_prediction_sync_logs_started (started_at)
);

CREATE TABLE prediction_quotes (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    quote_id VARCHAR(64) NOT NULL,
    user_id BIGINT UNSIGNED NOT NULL,
    market_id BIGINT UNSIGNED NOT NULL,
    outcome VARCHAR(16) NOT NULL,
    asset_id BIGINT UNSIGNED NOT NULL,
    stake_amount DECIMAL(38,18) NOT NULL,
    fee_amount DECIMAL(38,18) NOT NULL,
    accepted_price DECIMAL(18,8) NOT NULL,
    shares DECIMAL(38,18) NOT NULL,
    theoretical_payout DECIMAL(38,18) NOT NULL,
    effective_payout_cap DECIMAL(38,18) NOT NULL,
    expires_at TIMESTAMP(6) NOT NULL,
    consumed_at TIMESTAMP(6) NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    UNIQUE KEY uk_prediction_quotes_quote_id (quote_id),
    INDEX idx_prediction_quotes_user_time (user_id, created_at),
    INDEX idx_prediction_quotes_market (market_id),
    CONSTRAINT fk_prediction_quotes_user FOREIGN KEY (user_id) REFERENCES users(id),
    CONSTRAINT fk_prediction_quotes_market FOREIGN KEY (market_id) REFERENCES prediction_markets(id),
    CONSTRAINT fk_prediction_quotes_asset FOREIGN KEY (asset_id) REFERENCES assets(id)
);

CREATE TABLE prediction_orders (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    order_no VARCHAR(64) NULL,
    user_id BIGINT UNSIGNED NOT NULL,
    market_id BIGINT UNSIGNED NOT NULL,
    quote_id VARCHAR(64) NOT NULL,
    idempotency_key VARCHAR(128) NOT NULL,
    outcome VARCHAR(16) NOT NULL,
    asset_id BIGINT UNSIGNED NOT NULL,
    stake_amount DECIMAL(38,18) NOT NULL,
    fee_amount DECIMAL(38,18) NOT NULL,
    accepted_price DECIMAL(18,8) NOT NULL,
    shares DECIMAL(38,18) NOT NULL,
    theoretical_payout DECIMAL(38,18) NOT NULL,
    effective_payout_cap DECIMAL(38,18) NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'open',
    result VARCHAR(32) NULL,
    payout_amount DECIMAL(38,18) NOT NULL DEFAULT 0,
    refund_amount DECIMAL(38,18) NOT NULL DEFAULT 0,
    fee_refund_amount DECIMAL(38,18) NOT NULL DEFAULT 0,
    invalid_refund_policy_used VARCHAR(32) NULL,
    settled_at TIMESTAMP(6) NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    UNIQUE KEY uk_prediction_orders_user_idempotency (user_id, idempotency_key),
    UNIQUE KEY uk_prediction_orders_order_no (order_no),
    UNIQUE KEY uk_prediction_orders_quote_id (quote_id),
    INDEX idx_prediction_orders_user_time (user_id, created_at),
    INDEX idx_prediction_orders_market (market_id),
    INDEX idx_prediction_orders_status (status),
    CONSTRAINT fk_prediction_orders_user FOREIGN KEY (user_id) REFERENCES users(id),
    CONSTRAINT fk_prediction_orders_market FOREIGN KEY (market_id) REFERENCES prediction_markets(id),
    CONSTRAINT fk_prediction_orders_asset FOREIGN KEY (asset_id) REFERENCES assets(id)
);
