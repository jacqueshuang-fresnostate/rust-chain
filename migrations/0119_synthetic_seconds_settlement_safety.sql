ALTER TABLE market_price_ticks
    ADD COLUMN strategy_id BIGINT UNSIGNED NULL
        COMMENT '合成行情归档所属策略 ID，外部行情为 NULL'
        AFTER source_version,
    ADD COLUMN strategy_version INT NULL
        COMMENT '合成行情归档使用的活跃策略版本，外部行情为 NULL'
        AFTER strategy_id,
    ADD UNIQUE KEY uq_market_price_ticks_strategy_event
        (strategy_id, observed_at),
    ADD INDEX idx_market_price_ticks_strategy_version
        (strategy_id, strategy_version),
    ADD CONSTRAINT fk_market_price_ticks_strategy_version
        FOREIGN KEY (strategy_id, strategy_version)
        REFERENCES strategy_versions(strategy_id, version),
    ADD CONSTRAINT chk_market_price_ticks_strategy_identity_pair
        CHECK ((strategy_id IS NULL AND strategy_version IS NULL)
            OR (strategy_id IS NOT NULL AND strategy_version IS NOT NULL)),
    ADD CONSTRAINT chk_market_price_ticks_strategy_identity_values
        CHECK (strategy_id IS NULL
            OR (source = 'strategy' AND strategy_version > 0
                AND generation = strategy_version));

ALTER TABLE seconds_contract_orders
    ADD COLUMN settlement_failure_code VARCHAR(64) NULL
        COMMENT '自动结算失败后转人工审核的稳定错误码'
        AFTER settlement_price_version,
    ADD COLUMN settlement_failed_at TIMESTAMP(6) NULL
        COMMENT '自动结算确认无法继续的 UTC 时间'
        AFTER settlement_failure_code,
    ADD COLUMN settlement_window_start TIMESTAMP(6) NULL
        COMMENT '最后一次查找事件价格的窗口起点'
        AFTER settlement_failed_at,
    ADD COLUMN settlement_window_end TIMESTAMP(6) NULL
        COMMENT '最后一次查找事件价格的窗口终点（右边界不包含）'
        AFTER settlement_window_start,
    ADD INDEX idx_seconds_contract_orders_manual_review
        (status, settlement_failed_at, id),
    ADD CONSTRAINT chk_seconds_contract_orders_manual_review_evidence
        CHECK (status <> 'manual_review'
            OR (settlement_failure_code IS NOT NULL
                AND settlement_failed_at IS NOT NULL
                AND settlement_window_start IS NOT NULL
                AND settlement_window_end > settlement_window_start));

CREATE TABLE seconds_contract_settlement_exceptions (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    order_id BIGINT UNSIGNED NOT NULL,
    failure_code VARCHAR(64) NOT NULL,
    detected_at TIMESTAMP(6) NOT NULL,
    window_start TIMESTAMP(6) NOT NULL,
    window_end TIMESTAMP(6) NOT NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    UNIQUE KEY uq_seconds_contract_settlement_exceptions_order (order_id),
    INDEX idx_seconds_contract_settlement_exceptions_detected (detected_at, id),
    CONSTRAINT fk_seconds_contract_settlement_exceptions_order
        FOREIGN KEY (order_id) REFERENCES seconds_contract_orders(id),
    CONSTRAINT chk_seconds_contract_settlement_exceptions_failure_code
        CHECK (CHAR_LENGTH(TRIM(failure_code)) > 0),
    CONSTRAINT chk_seconds_contract_settlement_exceptions_window
        CHECK (window_end > window_start)
) ENGINE=InnoDB
  DEFAULT CHARACTER SET utf8mb4
  COLLATE utf8mb4_unicode_ci
  COMMENT='秒合约自动结算转人工审核的追加式异常证据';
