ALTER TABLE margin_products
    ADD COLUMN margin_mode VARCHAR(16) NOT NULL DEFAULT 'isolated' AFTER margin_asset,
    ADD COLUMN leverage_levels JSON NULL AFTER margin_mode,
    ADD CONSTRAINT chk_margin_products_margin_mode CHECK (margin_mode IN ('isolated', 'cross'));

UPDATE margin_products
SET leverage_levels = JSON_MERGE_PRESERVE(
    COALESCE((
        SELECT JSON_ARRAYAGG(level_text)
        FROM (
            SELECT '2' AS level_text, CAST(2 AS DECIMAL(18,8)) AS level_value
            UNION ALL SELECT '5', CAST(5 AS DECIMAL(18,8))
            UNION ALL SELECT '10', CAST(10 AS DECIMAL(18,8))
            UNION ALL SELECT '20', CAST(20 AS DECIMAL(18,8))
            UNION ALL SELECT '30', CAST(30 AS DECIMAL(18,8))
            UNION ALL SELECT '40', CAST(40 AS DECIMAL(18,8))
            UNION ALL SELECT '50', CAST(50 AS DECIMAL(18,8))
            UNION ALL SELECT '100', CAST(100 AS DECIMAL(18,8))
            UNION ALL SELECT '200', CAST(200 AS DECIMAL(18,8))
            UNION ALL SELECT '1000', CAST(1000 AS DECIMAL(18,8))
        ) default_levels
        WHERE default_levels.level_value <= margin_products.max_leverage
    ), JSON_ARRAY()),
    CASE
        WHEN EXISTS (
            SELECT 1
            FROM (
                SELECT CAST(2 AS DECIMAL(18,8)) AS level_value
                UNION ALL SELECT CAST(5 AS DECIMAL(18,8))
                UNION ALL SELECT CAST(10 AS DECIMAL(18,8))
                UNION ALL SELECT CAST(20 AS DECIMAL(18,8))
                UNION ALL SELECT CAST(30 AS DECIMAL(18,8))
                UNION ALL SELECT CAST(40 AS DECIMAL(18,8))
                UNION ALL SELECT CAST(50 AS DECIMAL(18,8))
                UNION ALL SELECT CAST(100 AS DECIMAL(18,8))
                UNION ALL SELECT CAST(200 AS DECIMAL(18,8))
                UNION ALL SELECT CAST(1000 AS DECIMAL(18,8))
            ) default_levels
            WHERE default_levels.level_value = margin_products.max_leverage
        ) THEN JSON_ARRAY()
        ELSE JSON_ARRAY(CAST(margin_products.max_leverage AS CHAR))
    END
)
WHERE leverage_levels IS NULL;

ALTER TABLE margin_products
    MODIFY COLUMN leverage_levels JSON NOT NULL;

ALTER TABLE margin_positions
    ADD COLUMN margin_mode VARCHAR(16) NOT NULL DEFAULT 'isolated' AFTER margin_asset,
    ADD CONSTRAINT chk_margin_positions_margin_mode CHECK (margin_mode IN ('isolated', 'cross'));
