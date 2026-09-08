CREATE TABLE market_default_generators (
    pair_id BIGINT UNSIGNED NOT NULL PRIMARY KEY,
    enabled BOOLEAN NOT NULL DEFAULT FALSE,
    all_market_paused BOOLEAN NOT NULL DEFAULT FALSE,
    initial_price DECIMAL(38,18) NULL,
    config_json JSON NOT NULL,
    version INT UNSIGNED NOT NULL,
    seed VARCHAR(128) NOT NULL,
    updated_by BIGINT UNSIGNED NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    CONSTRAINT fk_market_default_generators_pair FOREIGN KEY (pair_id) REFERENCES trading_pairs(id),
    CONSTRAINT fk_market_default_generators_admin FOREIGN KEY (updated_by) REFERENCES admin_users(id),
    CONSTRAINT chk_market_default_generators_price CHECK (initial_price IS NULL OR initial_price > 0),
    CONSTRAINT chk_market_default_generators_version CHECK (version > 0),
    CONSTRAINT chk_market_default_generators_seed CHECK (CHAR_LENGTH(TRIM(seed)) > 0)
) ENGINE=InnoDB DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
COMMENT='交易对常驻默认行情配置；初次保存不隐式启用';

CREATE TABLE market_default_generator_versions (
    pair_id BIGINT UNSIGNED NOT NULL,
    version INT UNSIGNED NOT NULL,
    initial_price DECIMAL(38,18) NULL,
    config_json JSON NOT NULL,
    seed VARCHAR(128) NOT NULL,
    created_by BIGINT UNSIGNED NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    PRIMARY KEY (pair_id, version),
    CONSTRAINT fk_market_default_versions_pair FOREIGN KEY (pair_id) REFERENCES market_default_generators(pair_id),
    CONSTRAINT fk_market_default_versions_admin FOREIGN KEY (created_by) REFERENCES admin_users(id),
    CONSTRAINT chk_market_default_versions_price CHECK (initial_price IS NULL OR initial_price > 0),
    CONSTRAINT chk_market_default_versions_version CHECK (version > 0)
) ENGINE=InnoDB DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
COMMENT='默认行情不可变配置版本；当前分钟继续使用原版本';

CREATE TABLE market_pair_generation_runs (
    pair_id BIGINT UNSIGNED NOT NULL PRIMARY KEY,
    generation BIGINT UNSIGNED NOT NULL DEFAULT 0,
    active_source VARCHAR(32) NOT NULL DEFAULT 'none',
    strategy_id BIGINT UNSIGNED NULL,
    strategy_version INT NULL,
    default_version INT UNSIGNED NULL,
    lease_owner VARCHAR(128) NULL,
    lease_expires_at TIMESTAMP(6) NULL,
    last_price DECIMAL(38,18) NULL,
    last_tick_at TIMESTAMP(6) NULL,
    state_json JSON NULL,
    error_message TEXT NULL,
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
    CONSTRAINT fk_market_pair_generation_runs_pair FOREIGN KEY (pair_id) REFERENCES trading_pairs(id),
    CONSTRAINT chk_market_pair_generation_runs_source CHECK (active_source IN ('none', 'strategy', 'default')),
    CONSTRAINT chk_market_pair_generation_runs_price CHECK (last_price IS NULL OR last_price > 0)
) ENGINE=InnoDB DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
COMMENT='人工策略与默认行情共享的单写入者代际和连续价格检查点';

-- 保留原策略身份及 generation=strategy_version 约束；默认来源使用独立交易对代际且不伪造策略身份。
ALTER TABLE market_price_ticks
    DROP CHECK chk_market_price_ticks_source,
    ADD CONSTRAINT chk_market_price_ticks_source
        CHECK (source IN ('bitget', 'htx', 'coinbase', 'strategy', 'default'));
