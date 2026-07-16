ALTER TABLE assets
    ADD COLUMN logo_url TEXT NULL AFTER name;

ALTER TABLE trading_pairs
    ADD COLUMN logo_url TEXT NULL AFTER symbol;

ALTER TABLE seconds_contract_products
    ADD COLUMN logo_url TEXT NULL AFTER stake_asset;

ALTER TABLE margin_products
    ADD COLUMN logo_url TEXT NULL AFTER margin_asset;

ALTER TABLE earn_products
    ADD COLUMN banner_url TEXT NULL AFTER name,
    ADD COLUMN small_logo_url TEXT NULL AFTER banner_url;

ALTER TABLE admin_news_items
    ADD COLUMN banner_url TEXT NULL AFTER title,
    ADD COLUMN small_logo_url TEXT NULL AFTER banner_url;
