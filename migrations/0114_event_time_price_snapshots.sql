CREATE TABLE market_price_ticks (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    event_key CHAR(64) NOT NULL,
    symbol VARCHAR(64) NOT NULL,
    price DECIMAL(38,18) NOT NULL,
    source VARCHAR(32) NOT NULL,
    observed_at TIMESTAMP(6) NOT NULL,
    generation BIGINT UNSIGNED NOT NULL,
    source_version VARCHAR(128) NOT NULL,
    ingested_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    UNIQUE KEY uk_market_price_ticks_event_key (event_key),
    INDEX idx_market_price_ticks_symbol_observed (symbol, observed_at, source, id),
    CONSTRAINT chk_market_price_ticks_price CHECK (price > 0),
    CONSTRAINT chk_market_price_ticks_symbol CHECK (CHAR_LENGTH(TRIM(symbol)) > 0),
    CONSTRAINT chk_market_price_ticks_source
        CHECK (source IN ('bitget', 'htx', 'coinbase', 'strategy')),
    CONSTRAINT chk_market_price_ticks_generation CHECK (generation > 0),
    CONSTRAINT chk_market_price_ticks_source_version
        CHECK (CHAR_LENGTH(TRIM(source_version)) > 0)
) COMMENT='Append-only market ticker observations used by event-time settlement';

ALTER TABLE seconds_contract_orders
    ADD COLUMN settlement_price_tick_id BIGINT UNSIGNED NULL AFTER settlement_price,
    ADD COLUMN settlement_price_source VARCHAR(32) NULL AFTER settlement_price_tick_id,
    ADD COLUMN settlement_price_observed_at TIMESTAMP(6) NULL AFTER settlement_price_source,
    ADD COLUMN settlement_price_generation BIGINT UNSIGNED NULL AFTER settlement_price_observed_at,
    ADD COLUMN settlement_price_version VARCHAR(128) NULL AFTER settlement_price_generation,
    ADD INDEX idx_seconds_contract_orders_settlement_tick (settlement_price_tick_id),
    ADD CONSTRAINT fk_seconds_contract_orders_settlement_tick
        FOREIGN KEY (settlement_price_tick_id) REFERENCES market_price_ticks(id);
