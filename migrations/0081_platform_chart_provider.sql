ALTER TABLE platform_brand_configs
    ADD COLUMN chart_provider VARCHAR(32) NULL COMMENT 'PC K线图引擎：klinecharts 或 tradingview' AFTER logo_url;

UPDATE platform_brand_configs
SET chart_provider = 'klinecharts'
WHERE chart_provider IS NULL OR TRIM(chart_provider) = '';

ALTER TABLE platform_brand_configs
    MODIFY COLUMN chart_provider VARCHAR(32) NOT NULL DEFAULT 'klinecharts' COMMENT 'PC K线图引擎：klinecharts 或 tradingview';
