CREATE TABLE market_strategy_nodes (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT
        COMMENT '行情策略目标节点：主键',
    strategy_id BIGINT UNSIGNED NOT NULL
        COMMENT '行情策略目标节点：所属行情策略 ID',
    sequence_no INT UNSIGNED NOT NULL
        COMMENT '行情策略目标节点：策略内顺序号',
    target_time TIMESTAMP(6) NOT NULL
        COMMENT '行情策略目标节点：目标 UTC 时间',
    target_type VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '行情策略目标节点：目标价格计算类型',
    target_value DECIMAL(38,18) NOT NULL
        COMMENT '行情策略目标节点：目标价格或涨跌百分比',
    execution_mode VARCHAR(16)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '行情策略目标节点：节点命中模式',
    tolerance DECIMAL(18,8) NOT NULL DEFAULT 0
        COMMENT '行情策略目标节点：允许的价格容差百分比',
    volatility DECIMAL(18,8) NOT NULL
        COMMENT '行情策略目标节点：局部波动率',
    volume_min DECIMAL(38,18) NULL DEFAULT NULL
        COMMENT '行情策略目标节点：局部最小成交量',
    volume_max DECIMAL(38,18) NULL DEFAULT NULL
        COMMENT '行情策略目标节点：局部最大成交量',
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6)
        COMMENT '行情策略目标节点：创建时间',
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6)
        COMMENT '行情策略目标节点：更新时间',
    UNIQUE KEY uq_market_strategy_nodes_sequence (strategy_id, sequence_no),
    UNIQUE KEY uq_market_strategy_nodes_time (strategy_id, target_time),
    INDEX idx_market_strategy_nodes_strategy_time (strategy_id, target_time),
    CONSTRAINT fk_market_strategy_nodes_strategy
        FOREIGN KEY (strategy_id) REFERENCES market_strategies(id) ON DELETE CASCADE,
    CONSTRAINT chk_market_strategy_nodes_target_type
        CHECK (target_type IN ('absolute_price', 'percent_from_start', 'percent_from_previous')),
    CONSTRAINT chk_market_strategy_nodes_execution_mode
        CHECK (execution_mode IN ('hard', 'soft', 'range')),
    CONSTRAINT chk_market_strategy_nodes_tolerance CHECK (tolerance >= 0),
    CONSTRAINT chk_market_strategy_nodes_volatility CHECK (volatility >= 0),
    CONSTRAINT chk_market_strategy_nodes_target_value
        CHECK ((target_type = 'absolute_price' AND target_value > 0)
            OR (target_type IN ('percent_from_start', 'percent_from_previous')
                AND target_value > -100)),
    CONSTRAINT chk_market_strategy_nodes_volume_pair
        CHECK ((volume_min IS NULL AND volume_max IS NULL)
            OR (volume_min IS NOT NULL AND volume_max IS NOT NULL
                AND volume_min >= 0 AND volume_max >= volume_min))
) ENGINE=InnoDB
  DEFAULT CHARACTER SET utf8mb4
  COLLATE utf8mb4_unicode_ci
  COMMENT='行情策略有序目标节点';

ALTER TABLE strategy_runs
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    ADD COLUMN active_version INT NULL
        COMMENT '策略运行状态：当前绑定的配置版本'
        AFTER strategy_id,
    ADD COLUMN lease_owner VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '策略运行状态：实时生成短租约持有者'
        AFTER recovery_status,
    ADD COLUMN lease_expires_at TIMESTAMP(6) NULL DEFAULT NULL
        COMMENT '策略运行状态：实时生成短租约过期时间'
        AFTER lease_owner,
    ADD INDEX idx_strategy_runs_lease (lease_expires_at, strategy_id);

INSERT INTO strategy_versions
    (strategy_id, version, effective_time, config_json, seed, created_by)
SELECT runs.strategy_id,
       1,
       strategies.start_time,
       JSON_OBJECT(
           'pair_id', strategies.pair_id,
           'market_type', pairs.market_type,
           'strategy_type', strategies.strategy_type,
           'start_price', CAST(strategies.start_price AS CHAR),
           'target_price', CAST(strategies.target_price AS CHAR),
           'start_time', CAST(FLOOR(UNIX_TIMESTAMP(strategies.start_time) * 1000) AS SIGNED),
           'end_time', CAST(FLOOR(UNIX_TIMESTAMP(strategies.end_time) * 1000) AS SIGNED),
           'volatility', CAST(strategies.volatility AS CHAR),
           'volume_min', CAST(strategies.volume_min AS CHAR),
           'volume_max', CAST(strategies.volume_max AS CHAR),
           'status', strategies.status,
           'nodes', JSON_ARRAY()
       ),
       CONCAT('migration-0102-strategy-', runs.strategy_id, '-version-1'),
       NULL
FROM strategy_runs runs
INNER JOIN market_strategies strategies ON strategies.id = runs.strategy_id
INNER JOIN trading_pairs pairs ON pairs.id = strategies.pair_id
WHERE NOT EXISTS (
    SELECT 1
    FROM strategy_versions versions
    WHERE versions.strategy_id = runs.strategy_id
);

UPDATE strategy_runs runs
SET active_version = (
    SELECT MAX(versions.version)
    FROM strategy_versions versions
    WHERE versions.strategy_id = runs.strategy_id
)
WHERE active_version IS NULL;

ALTER TABLE strategy_runs
    MODIFY COLUMN active_version INT NOT NULL
        COMMENT '策略运行状态：当前绑定的配置版本',
    ADD CONSTRAINT fk_strategy_runs_active_version
        FOREIGN KEY (strategy_id, active_version)
        REFERENCES strategy_versions(strategy_id, version)
        ON UPDATE CASCADE;

CREATE TABLE kline_recovery_jobs (
    id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT
        COMMENT 'K 线手动补偿任务：主键',
    strategy_id BIGINT UNSIGNED NOT NULL
        COMMENT 'K 线手动补偿任务：所属行情策略 ID',
    requested_by BIGINT UNSIGNED NOT NULL
        COMMENT 'K 线手动补偿任务：发起管理员 ID',
    config_version INT NOT NULL
        COMMENT 'K 线手动补偿任务：预览绑定的策略版本',
    range_start TIMESTAMP(6) NOT NULL
        COMMENT 'K 线手动补偿任务：补偿范围起始时间',
    range_end TIMESTAMP(6) NOT NULL
        COMMENT 'K 线手动补偿任务：补偿范围结束时间',
    preview_token_hash CHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT 'K 线手动补偿任务：预览令牌 SHA-256 哈希',
    reason VARCHAR(512)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT 'K 线手动补偿任务：管理员审计原因',
    status VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'pending'
        COMMENT 'K 线手动补偿任务：当前执行状态',
    expected_1m_count INT UNSIGNED NOT NULL
        COMMENT 'K 线手动补偿任务：预期生成 1m K 线根数',
    actual_1m_count INT UNSIGNED NOT NULL DEFAULT 0
        COMMENT 'K 线手动补偿任务：实际写入 1m K 线根数',
    actual_aggregate_count INT UNSIGNED NOT NULL DEFAULT 0
        COMMENT 'K 线手动补偿任务：实际写入聚合 K 线根数',
    error_message TEXT
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT 'K 线手动补偿任务：失败错误信息',
    started_at TIMESTAMP(6) NULL DEFAULT NULL
        COMMENT 'K 线手动补偿任务：开始执行时间',
    completed_at TIMESTAMP(6) NULL DEFAULT NULL
        COMMENT 'K 线手动补偿任务：执行完成时间',
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6)
        COMMENT 'K 线手动补偿任务：创建时间',
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6)
        COMMENT 'K 线手动补偿任务：更新时间',
    UNIQUE KEY uq_kline_recovery_jobs_token (preview_token_hash),
    INDEX idx_kline_recovery_jobs_strategy_time (strategy_id, created_at),
    INDEX idx_kline_recovery_jobs_status_time (status, created_at),
    CONSTRAINT fk_kline_recovery_jobs_strategy
        FOREIGN KEY (strategy_id) REFERENCES market_strategies(id),
    CONSTRAINT fk_kline_recovery_jobs_admin
        FOREIGN KEY (requested_by) REFERENCES admin_users(id),
    CONSTRAINT chk_kline_recovery_jobs_range CHECK (range_end > range_start),
    CONSTRAINT chk_kline_recovery_jobs_expected_count CHECK (expected_1m_count > 0),
    CONSTRAINT chk_kline_recovery_jobs_actual_counts
        CHECK (actual_1m_count <= expected_1m_count),
    CONSTRAINT chk_kline_recovery_jobs_status
        CHECK (status IN ('pending', 'running', 'completed', 'failed'))
) ENGINE=InnoDB
  DEFAULT CHARACTER SET utf8mb4
  COLLATE utf8mb4_unicode_ci
  COMMENT='K 线手动补偿任务';
