ALTER TABLE prediction_markets
    ADD COLUMN market_version BIGINT UNSIGNED NOT NULL DEFAULT 1 AFTER last_synced_at,
    ADD COLUMN locally_closed_at TIMESTAMP(6) NULL AFTER market_version,
    ADD INDEX idx_prediction_markets_local_close
        (settlement_status, end_at, id);

ALTER TABLE prediction_quotes
    ADD COLUMN market_version BIGINT UNSIGNED NOT NULL DEFAULT 1 AFTER effective_payout_cap,
    ADD COLUMN market_last_synced_at TIMESTAMP(6) NULL AFTER market_version;

UPDATE prediction_quotes quotes
INNER JOIN prediction_markets markets ON markets.id = quotes.market_id
SET quotes.market_version = markets.market_version,
    quotes.market_last_synced_at = COALESCE(markets.last_synced_at, quotes.created_at)
WHERE quotes.market_last_synced_at IS NULL;

ALTER TABLE prediction_quotes
    MODIFY COLUMN market_last_synced_at TIMESTAMP(6) NOT NULL,
    ADD INDEX idx_prediction_quotes_market_version (market_id, market_version);
