CREATE TABLE trading_pairs (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    base_asset BIGINT UNSIGNED NOT NULL,
    quote_asset BIGINT UNSIGNED NOT NULL,
    symbol VARCHAR(64) NOT NULL UNIQUE,
    price_precision INT NOT NULL,
    qty_precision INT NOT NULL,
    min_order_value DECIMAL(38,18) NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'disabled',
    market_type VARCHAR(32) NOT NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    CONSTRAINT fk_trading_pairs_base FOREIGN KEY (base_asset) REFERENCES assets(id),
    CONSTRAINT fk_trading_pairs_quote FOREIGN KEY (quote_asset) REFERENCES assets(id)
);

CREATE TABLE market_sources (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    name VARCHAR(64) NOT NULL UNIQUE,
    priority INT NOT NULL DEFAULT 100,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    rest_base_url VARCHAR(255) NOT NULL,
    ws_url VARCHAR(255) NOT NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6)
);

CREATE TABLE market_strategies (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    pair_id BIGINT UNSIGNED NOT NULL,
    strategy_type VARCHAR(32) NOT NULL,
    start_price DECIMAL(38,18) NOT NULL,
    target_price DECIMAL(38,18) NOT NULL,
    start_time TIMESTAMP(6) NOT NULL,
    end_time TIMESTAMP(6) NOT NULL,
    volatility DECIMAL(18,8) NOT NULL,
    volume_min DECIMAL(38,18) NOT NULL,
    volume_max DECIMAL(38,18) NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'draft',
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    CONSTRAINT fk_market_strategies_pair FOREIGN KEY (pair_id) REFERENCES trading_pairs(id)
);

CREATE TABLE strategy_runs (
    strategy_id BIGINT UNSIGNED PRIMARY KEY,
    run_status VARCHAR(32) NOT NULL,
    current_price DECIMAL(38,18) NULL,
    last_tick_at TIMESTAMP(6) NULL,
    last_generated_at TIMESTAMP(6) NULL,
    last_kline_open_time TIMESTAMP(6) NULL,
    recovery_status VARCHAR(32) NULL,
    error_message TEXT NULL,
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    CONSTRAINT fk_strategy_runs_strategy FOREIGN KEY (strategy_id) REFERENCES market_strategies(id)
);

CREATE TABLE strategy_versions (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    strategy_id BIGINT UNSIGNED NOT NULL,
    version INT NOT NULL,
    effective_time TIMESTAMP(6) NOT NULL,
    config_json JSON NOT NULL,
    seed VARCHAR(128) NOT NULL,
    created_by BIGINT UNSIGNED NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    UNIQUE KEY uq_strategy_versions_strategy_version (strategy_id, version),
    CONSTRAINT fk_strategy_versions_strategy FOREIGN KEY (strategy_id) REFERENCES market_strategies(id),
    CONSTRAINT fk_strategy_versions_admin FOREIGN KEY (created_by) REFERENCES admin_users(id)
);

CREATE TABLE strategy_events (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,
    strategy_id BIGINT UNSIGNED NOT NULL,
    event_type VARCHAR(64) NOT NULL,
    payload_json JSON NOT NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    INDEX idx_strategy_events_strategy_time (strategy_id, created_at),
    CONSTRAINT fk_strategy_events_strategy FOREIGN KEY (strategy_id) REFERENCES market_strategies(id)
);
