ALTER TABLE loan_products
    ADD COLUMN name_json JSON NULL COMMENT '贷款产品多语言名称配置：version/default_locale/items(locale,country,title)' AFTER name;

UPDATE loan_products
SET name_json = JSON_OBJECT(
    'version', 1,
    'default_locale', 'zh-CN',
    'items', JSON_ARRAY(JSON_OBJECT(
        'locale', 'zh-CN',
        'country', 'CN',
        'title', name
    ))
)
WHERE name_json IS NULL;

ALTER TABLE loan_products
    MODIFY COLUMN name_json JSON NOT NULL COMMENT '贷款产品多语言名称配置：version/default_locale/items(locale,country,title)';
