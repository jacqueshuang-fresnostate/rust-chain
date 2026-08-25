-- 抵押贷产品风险阈值、白名单与订单行情快照。
ALTER TABLE loan_products
    ADD COLUMN initial_ltv DECIMAL(18,8) NULL COMMENT '申请与放款最大LTV' AFTER max_amount,
    ADD COLUMN maintenance_ltv DECIMAL(18,8) NULL COMMENT '维持保证金LTV' AFTER initial_ltv,
    ADD COLUMN liquidation_ltv DECIMAL(18,8) NULL COMMENT '强制清算LTV' AFTER maintenance_ltv;

-- 历史抵押产品没有可信的阈值和行情绑定，先停止新申请，由运营通过新配置入口审核后重新上架。
UPDATE loan_products
SET status = 'disabled'
WHERE loan_type = 'collateralized';

-- 信用贷不允许携带 LTV；历史抵押产品只有在已停用时才可保留空阈值。
ALTER TABLE loan_products
    ADD CONSTRAINT chk_loan_products_ltv CHECK (
        (
            loan_type = 'credit'
            AND initial_ltv IS NULL AND maintenance_ltv IS NULL AND liquidation_ltv IS NULL
        )
        OR (
            loan_type = 'collateralized' AND status = 'disabled'
            AND initial_ltv IS NULL AND maintenance_ltv IS NULL AND liquidation_ltv IS NULL
        )
        OR (
            loan_type = 'collateralized'
            AND initial_ltv IS NOT NULL AND maintenance_ltv IS NOT NULL
            AND liquidation_ltv IS NOT NULL
            AND initial_ltv > 0 AND initial_ltv < maintenance_ltv
            AND maintenance_ltv < liquidation_ltv AND liquidation_ltv <= 1
        )
    );

CREATE TABLE loan_product_collateral_assets (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT COMMENT '产品抵押资产白名单ID',
    product_id BIGINT UNSIGNED NOT NULL,
    collateral_asset_id BIGINT UNSIGNED NOT NULL,
    oracle_symbol VARCHAR(64) NOT NULL COMMENT '权威ticker符号，价格必须以贷款资产计价',
    oracle_source VARCHAR(64) NOT NULL COMMENT '行情适配器来源，当前仅market_ticker_redis',
    oracle_max_age_seconds BIGINT UNSIGNED NOT NULL COMMENT '最大行情年龄',
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    UNIQUE KEY uq_loan_product_collateral_asset (product_id, collateral_asset_id),
    UNIQUE KEY uq_loan_product_oracle_symbol (product_id, oracle_symbol),
    INDEX idx_loan_product_collateral_asset (collateral_asset_id),
    CONSTRAINT fk_loan_product_collateral_product FOREIGN KEY (product_id) REFERENCES loan_products(id) ON DELETE CASCADE,
    CONSTRAINT fk_loan_product_collateral_asset FOREIGN KEY (collateral_asset_id) REFERENCES assets(id),
    CONSTRAINT chk_loan_product_oracle_source CHECK (oracle_source = 'market_ticker_redis'),
    CONSTRAINT chk_loan_product_oracle_age CHECK (
        oracle_max_age_seconds > 0 AND oracle_max_age_seconds <= 86400
    )
) COMMENT='抵押贷抵押资产与行情源绑定';

ALTER TABLE loan_orders
    ADD COLUMN request_fingerprint CHAR(64) NOT NULL DEFAULT '' COMMENT '规范化请求SHA-256指纹' AFTER idempotency_key,
    ADD COLUMN initial_ltv DECIMAL(18,8) NULL COMMENT '下单时产品初始LTV快照' AFTER collateral_amount,
    ADD COLUMN maintenance_ltv DECIMAL(18,8) NULL COMMENT '下单时维持LTV快照' AFTER initial_ltv,
    ADD COLUMN liquidation_ltv DECIMAL(18,8) NULL COMMENT '下单时清算LTV快照' AFTER maintenance_ltv,
    ADD COLUMN oracle_symbol VARCHAR(64) NULL COMMENT '下单时行情符号快照' AFTER liquidation_ltv,
    ADD COLUMN oracle_source VARCHAR(64) NULL COMMENT '下单时行情源快照' AFTER oracle_symbol,
    ADD COLUMN oracle_max_age_seconds BIGINT UNSIGNED NULL COMMENT '下单时最大行情年龄快照' AFTER oracle_source,
    ADD COLUMN application_collateral_price DECIMAL(38,18) NULL COMMENT '申请时权威抵押价' AFTER oracle_max_age_seconds,
    ADD COLUMN application_price_observed_at TIMESTAMP(6) NULL COMMENT '申请价观测时间' AFTER application_collateral_price,
    ADD COLUMN application_ltv DECIMAL(38,18) NULL COMMENT '申请时LTV' AFTER application_price_observed_at,
    ADD COLUMN approval_collateral_price DECIMAL(38,18) NULL COMMENT '放款前权威抵押价' AFTER application_ltv,
    ADD COLUMN approval_price_observed_at TIMESTAMP(6) NULL COMMENT '放款前价格观测时间' AFTER approval_collateral_price,
    ADD COLUMN approval_ltv DECIMAL(38,18) NULL COMMENT '放款前LTV' AFTER approval_price_observed_at,
    ADD COLUMN health_checked_at TIMESTAMP(6) NULL COMMENT '健康扫描最近领取时间' AFTER approval_ltv,
    ADD INDEX idx_loan_orders_health_scan (
        loan_type, status, collateral_released_at, health_checked_at, id
    );

UPDATE loan_orders
SET request_fingerprint = SHA2(CONCAT_WS('|', 'legacy', user_id, product_id, CAST(amount AS CHAR),
    COALESCE(collateral_asset_id, 0), COALESCE(CAST(collateral_amount AS CHAR), '')), 256)
WHERE request_fingerprint = '';

-- 回填完成后移除空串默认值，后续所有新订单都必须显式写入真实请求指纹。
ALTER TABLE loan_orders
    MODIFY COLUMN request_fingerprint CHAR(64) NOT NULL COMMENT '规范化请求SHA-256指纹';

-- 历史订单允许整组风险快照为空并安全失败；一旦存在，新快照必须完整且保持单调关系与来源契约。
ALTER TABLE loan_orders
    ADD CONSTRAINT chk_loan_orders_ltv_snapshots CHECK (
        (initial_ltv IS NULL AND maintenance_ltv IS NULL AND liquidation_ltv IS NULL)
        OR (initial_ltv IS NOT NULL AND maintenance_ltv IS NOT NULL
            AND liquidation_ltv IS NOT NULL
            AND initial_ltv > 0 AND initial_ltv < maintenance_ltv
            AND maintenance_ltv < liquidation_ltv AND liquidation_ltv <= 1)
    ),
    ADD CONSTRAINT chk_loan_orders_oracle_snapshot CHECK (
        (oracle_symbol IS NULL AND oracle_source IS NULL AND oracle_max_age_seconds IS NULL)
        OR (oracle_symbol IS NOT NULL AND oracle_source IS NOT NULL
            AND oracle_max_age_seconds IS NOT NULL
            AND oracle_source = 'market_ticker_redis'
            AND oracle_max_age_seconds > 0 AND oracle_max_age_seconds <= 86400)
    ),
    ADD CONSTRAINT chk_loan_orders_application_risk_snapshot CHECK (
        (application_collateral_price IS NULL
            AND application_price_observed_at IS NULL AND application_ltv IS NULL)
        OR (application_collateral_price IS NOT NULL
            AND application_price_observed_at IS NOT NULL AND application_ltv IS NOT NULL
            AND application_collateral_price > 0
            AND application_ltv > 0 AND application_ltv <= initial_ltv)
    ),
    ADD CONSTRAINT chk_loan_orders_risk_snapshot_group CHECK (
        (initial_ltv IS NULL AND oracle_symbol IS NULL
            AND application_collateral_price IS NULL)
        OR (initial_ltv IS NOT NULL AND oracle_symbol IS NOT NULL
            AND application_collateral_price IS NOT NULL)
    ),
    ADD CONSTRAINT chk_loan_orders_approval_risk_snapshot CHECK (
        (approval_collateral_price IS NULL
            AND approval_price_observed_at IS NULL AND approval_ltv IS NULL)
        OR (approval_collateral_price IS NOT NULL
            AND approval_price_observed_at IS NOT NULL AND approval_ltv IS NOT NULL
            AND approval_collateral_price > 0
            AND approval_ltv > 0 AND approval_ltv <= initial_ltv)
    ),
    ADD CONSTRAINT chk_loan_orders_approval_requires_application CHECK (
        (approval_collateral_price IS NULL
            AND approval_price_observed_at IS NULL AND approval_ltv IS NULL)
        OR (initial_ltv IS NOT NULL AND oracle_symbol IS NOT NULL
            AND application_collateral_price IS NOT NULL)
    );
