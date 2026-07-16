CREATE TABLE earn_product_categories (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT COMMENT '理财分类ID',
    code VARCHAR(64) NOT NULL COMMENT '分类代码，理财产品通过该代码关联分类',
    name_json JSON NOT NULL COMMENT '分类栏目多语言名称 JSON',
    sort_order INT NOT NULL DEFAULT 0 COMMENT '排序值，数值越小越靠前',
    status VARCHAR(32) NOT NULL DEFAULT 'active' COMMENT '状态：active启用，disabled停用',
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) COMMENT '创建时间',
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6) COMMENT '更新时间',
    UNIQUE KEY uq_earn_product_categories_code (code),
    INDEX idx_earn_product_categories_status_sort (status, sort_order),
    CONSTRAINT chk_earn_product_categories_status CHECK (status IN ('active', 'disabled'))
) COMMENT='理财产品分类栏目';

INSERT INTO earn_product_categories (code, name_json, sort_order, status)
VALUES
    (
        'fixed_term',
        JSON_OBJECT(
            'version', 1,
            'default_locale', 'zh-CN',
            'items', JSON_ARRAY(JSON_OBJECT('locale', 'zh-CN', 'country', 'CN', 'title', '定期'))
        ),
        10,
        'active'
    ),
    (
        'flexible',
        JSON_OBJECT(
            'version', 1,
            'default_locale', 'zh-CN',
            'items', JSON_ARRAY(JSON_OBJECT('locale', 'zh-CN', 'country', 'CN', 'title', '活期'))
        ),
        20,
        'active'
    ),
    (
        'structured',
        JSON_OBJECT(
            'version', 1,
            'default_locale', 'zh-CN',
            'items', JSON_ARRAY(JSON_OBJECT('locale', 'zh-CN', 'country', 'CN', 'title', '结构化'))
        ),
        30,
        'active'
    ),
    (
        'staking',
        JSON_OBJECT(
            'version', 1,
            'default_locale', 'zh-CN',
            'items', JSON_ARRAY(JSON_OBJECT('locale', 'zh-CN', 'country', 'CN', 'title', '质押'))
        ),
        40,
        'active'
    )
ON DUPLICATE KEY UPDATE code = VALUES(code);

INSERT INTO earn_product_categories (code, name_json, sort_order, status)
SELECT
    products.category,
    JSON_OBJECT(
        'version', 1,
        'default_locale', 'zh-CN',
        'items', JSON_ARRAY(JSON_OBJECT('locale', 'zh-CN', 'country', 'CN', 'title', products.category))
    ),
    1000,
    'active'
FROM earn_products products
LEFT JOIN earn_product_categories categories ON categories.code = products.category
WHERE products.category <> ''
  AND categories.id IS NULL
GROUP BY products.category;
