ALTER TABLE convert_quotes
    ADD COLUMN request_fingerprint CHAR(64) NULL AFTER fee_amount,
    ADD COLUMN price_source VARCHAR(32) NULL AFTER request_fingerprint,
    ADD COLUMN price_symbol VARCHAR(64) NULL AFTER price_source,
    ADD COLUMN price_observed_at TIMESTAMP(6) NULL AFTER price_symbol,
    ADD COLUMN price_version VARCHAR(128) NULL AFTER price_observed_at,
    ADD COLUMN consumed_at TIMESTAMP(6) NULL AFTER status;

UPDATE convert_quotes
SET request_fingerprint = SHA2(CONCAT_WS('|',
        user_id,
        convert_pair_id,
        from_asset,
        to_asset,
        CAST(from_amount AS CHAR),
        CAST(to_amount AS CHAR),
        CAST(rate AS CHAR),
        CAST(fee_amount AS CHAR),
        DATE_FORMAT(expires_at, '%Y-%m-%dT%H:%i:%s.%f')
    ), 256),
    price_source = 'legacy',
    price_observed_at = created_at,
    price_version = CONCAT('legacy:', id),
    consumed_at = CASE WHEN status = 'quoted' THEN NULL ELSE created_at END
WHERE request_fingerprint IS NULL;

ALTER TABLE convert_quotes
    MODIFY COLUMN request_fingerprint CHAR(64) NOT NULL,
    MODIFY COLUMN price_source VARCHAR(32) NOT NULL,
    MODIFY COLUMN price_observed_at TIMESTAMP(6) NOT NULL,
    MODIFY COLUMN price_version VARCHAR(128) NOT NULL,
    ADD INDEX idx_convert_quotes_consume (quote_id, user_id, status, consumed_at, expires_at),
    ADD CONSTRAINT chk_convert_quotes_fingerprint
        CHECK (CHAR_LENGTH(TRIM(request_fingerprint)) = 64),
    ADD CONSTRAINT chk_convert_quotes_price_source
        CHECK (price_source IN ('fixed', 'bitget', 'htx', 'coinbase', 'strategy', 'legacy')),
    ADD CONSTRAINT chk_convert_quotes_price_symbol
        CHECK (
            (price_source IN ('bitget', 'htx', 'coinbase', 'strategy')
                AND price_symbol IS NOT NULL AND CHAR_LENGTH(TRIM(price_symbol)) > 0)
            OR (price_source IN ('fixed', 'legacy') AND price_symbol IS NULL)
        ),
    ADD CONSTRAINT chk_convert_quotes_price_version
        CHECK (CHAR_LENGTH(TRIM(price_version)) > 0),
    ADD CONSTRAINT chk_convert_quotes_consumed_state
        CHECK (
            (status = 'quoted' AND consumed_at IS NULL)
            OR (status <> 'quoted' AND consumed_at IS NOT NULL)
        );
