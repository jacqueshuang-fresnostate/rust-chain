ALTER TABLE earn_products
    ADD COLUMN category VARCHAR(64) NOT NULL DEFAULT 'fixed_term' AFTER name,
    ADD COLUMN introduction_json JSON NULL AFTER category;

UPDATE earn_products
SET introduction_json = JSON_OBJECT(
    'version', 1,
    'default_locale', 'zh-CN',
    'items', JSON_ARRAY(
        JSON_OBJECT(
            'locale', 'zh-CN',
            'country', 'CN',
            'title', name,
            'content', JSON_ARRAY(
                JSON_OBJECT(
                    'type', 'p',
                    'children', JSON_ARRAY(JSON_OBJECT('text', name))
                )
            )
        )
    )
)
WHERE introduction_json IS NULL;

ALTER TABLE earn_products
    MODIFY COLUMN introduction_json JSON NOT NULL;
