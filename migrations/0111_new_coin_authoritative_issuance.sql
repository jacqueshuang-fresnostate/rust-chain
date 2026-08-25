-- 新币发行的服务端权威计价、供给与请求指纹。
ALTER TABLE new_coin_projects
    ADD COLUMN quote_asset_id BIGINT UNSIGNED NULL COMMENT '发行价计价资产' AFTER issue_price,
    ADD COLUMN reserved_supply DECIMAL(38,18) NOT NULL DEFAULT 0 COMMENT '事务中预留供给' AFTER quote_asset_id,
    ADD COLUMN allocated_supply DECIMAL(38,18) NOT NULL DEFAULT 0 COMMENT '已入账供给' AFTER reserved_supply,
    ADD COLUMN remaining_supply DECIMAL(38,18) NOT NULL DEFAULT 0 COMMENT '尚可分配供给' AFTER allocated_supply,
    ADD INDEX idx_new_coin_projects_quote_asset (quote_asset_id),
    ADD CONSTRAINT fk_new_coin_projects_quote_asset FOREIGN KEY (quote_asset_id) REFERENCES assets(id);

-- 历史项目优先从唯一申购计价资产回填，其次从已绑定交易对回填。
UPDATE new_coin_projects projects
INNER JOIN (
    SELECT project_id, MIN(quote_asset) AS quote_asset_id
    FROM new_coin_subscriptions
    GROUP BY project_id
    HAVING COUNT(DISTINCT quote_asset) = 1
) subscriptions ON subscriptions.project_id = projects.id
SET projects.quote_asset_id = subscriptions.quote_asset_id
WHERE projects.quote_asset_id IS NULL;

UPDATE new_coin_projects projects
INNER JOIN trading_pairs pairs ON pairs.id = projects.post_listing_pair_id
SET projects.quote_asset_id = pairs.quote_asset
WHERE projects.quote_asset_id IS NULL;

-- 历史坏数据若把新币自身当作计价资产，清空绑定并让运行时失败关闭，等待运营重新配置。
UPDATE new_coin_projects
SET quote_asset_id = NULL
WHERE quote_asset_id = asset_id;

ALTER TABLE new_coin_projects
    ADD CONSTRAINT chk_new_coin_projects_distinct_quote_asset CHECK (
        quote_asset_id IS NULL OR quote_asset_id <> asset_id
    );

-- 解禁费是 issue_price 计价市值的比例；没有额外汇率快照时，收费资产必须就是项目计价资产。
-- 历史不符合这一单位契约的未释放应收直接豁免，禁止按相同数值扣另一种资产。
UPDATE asset_unlock_records unlocks
INNER JOIN new_coin_projects projects ON projects.asset_id = unlocks.asset_id
SET unlocks.unlock_fee_enabled = FALSE,
    unlocks.unlock_fee_rate = NULL,
    unlocks.unlock_fee_basis = NULL,
    unlocks.unlock_fee_asset = NULL,
    unlocks.unlock_fee_amount = NULL,
    unlocks.fee_paid_status = 'not_required'
WHERE unlocks.status <> 'released'
  AND unlocks.unlock_fee_enabled = TRUE
  AND (
      projects.quote_asset_id IS NULL
      OR unlocks.unlock_fee_asset IS NULL
      OR unlocks.unlock_fee_asset <> projects.quote_asset_id
  );

UPDATE new_coin_projects
SET unlock_fee_enabled = FALSE,
    unlock_fee_rate = NULL,
    unlock_fee_basis = NULL,
    unlock_fee_asset = NULL
WHERE unlock_fee_enabled = TRUE
  AND (
      unlock_fee_rate IS NULL OR unlock_fee_rate <= 0
      OR unlock_fee_basis IS NULL OR unlock_fee_basis NOT IN ('market_value', 'profit')
      OR quote_asset_id IS NULL OR unlock_fee_asset IS NULL
      OR unlock_fee_asset <> quote_asset_id
  );

ALTER TABLE new_coin_projects
    ADD CONSTRAINT chk_new_coin_projects_unlock_fee_quote CHECK (
        unlock_fee_enabled = FALSE
        OR (
            unlock_fee_rate IS NOT NULL AND unlock_fee_rate > 0
            AND unlock_fee_basis IS NOT NULL
            AND unlock_fee_basis IN ('market_value', 'profit')
            AND quote_asset_id IS NOT NULL AND unlock_fee_asset IS NOT NULL
            AND unlock_fee_asset = quote_asset_id
        )
    );

-- 每次发行只有一条正向新币钱包腿，据此回建历史已分配数量。
UPDATE new_coin_projects projects
LEFT JOIN (
    SELECT project.id AS project_id, COALESCE(SUM(ledger.amount), 0) AS allocated_supply
    FROM new_coin_projects project
    LEFT JOIN wallet_ledger ledger
      ON ledger.asset_id = project.asset_id
     AND ledger.amount > 0
     AND ledger.change_type IN (
        'new_coin_subscription_lock',
        'new_coin_purchase_lock',
        'new_coin_distribution_lock'
     )
    GROUP BY project.id
) issued ON issued.project_id = projects.id
SET projects.reserved_supply = 0,
    projects.total_supply = GREATEST(projects.total_supply, COALESCE(issued.allocated_supply, 0)),
    projects.allocated_supply = COALESCE(issued.allocated_supply, 0),
    projects.remaining_supply = GREATEST(
        GREATEST(projects.total_supply, COALESCE(issued.allocated_supply, 0))
        - COALESCE(issued.allocated_supply, 0),
        0
    );

-- 历史若已超发，以实际正向发行流水抬升账面总量，保留事实而不是静默抹掉已发行数量。
ALTER TABLE new_coin_projects
    ADD CONSTRAINT chk_new_coin_projects_supply_counters CHECK (
        reserved_supply >= 0 AND allocated_supply >= 0 AND remaining_supply >= 0
        AND reserved_supply + allocated_supply + remaining_supply = total_supply
    );

ALTER TABLE new_coin_subscriptions
    ADD COLUMN issue_price DECIMAL(38,18) NOT NULL DEFAULT 0 COMMENT '申购时服务端发行价快照' AFTER quote_asset,
    ADD COLUMN request_fingerprint CHAR(64) NULL COMMENT '规范化请求SHA-256指纹；NULL表示迁移前订单' AFTER idempotency_key;

UPDATE new_coin_subscriptions subscriptions
INNER JOIN new_coin_projects projects ON projects.id = subscriptions.project_id
SET subscriptions.issue_price = projects.issue_price
WHERE subscriptions.issue_price = 0;

ALTER TABLE new_coin_purchase_orders
    ADD COLUMN request_fingerprint CHAR(64) NULL COMMENT '规范化请求SHA-256指纹；NULL表示迁移前订单' AFTER idempotency_key;

-- 解禁记录原先直接复用订单裸键，不同新币业务表可使用同一键并误命中同一应收；迁移为来源类型命名空间。
UPDATE asset_unlock_records unlocks
INNER JOIN (
    SELECT lock_position_id, source_id, MIN(id) AS first_source_id
    FROM asset_lock_position_sources
    WHERE source_type IN (
        'new_coin_subscription', 'new_coin_purchase', 'new_coin_distribution'
    )
    GROUP BY lock_position_id, source_id
) chosen
        ON chosen.lock_position_id = unlocks.lock_position_id
       AND chosen.source_id = unlocks.idempotency_key
INNER JOIN asset_lock_position_sources sources ON sources.id = chosen.first_source_id
SET unlocks.idempotency_key = CONCAT(sources.source_type, ':', sources.source_id)
WHERE sources.source_type IN ('new_coin_subscription', 'new_coin_purchase', 'new_coin_distribution');

-- 命名空间键会比用户原始键更长，缴费和释放流水必须完整保存该键，禁止数据库截断后失去证据关联。
ALTER TABLE wallet_ledger
    MODIFY COLUMN ref_id VARCHAR(255) NOT NULL;

-- 旧逻辑只置位 paid 而没有真实扣款，未释放记录必须回到待缴。
ALTER TABLE asset_unlock_records
    ADD COLUMN fee_paid_at TIMESTAMP(6) NULL COMMENT '真实扣款完成时间' AFTER fee_paid_status,
    ADD COLUMN unlock_fee_payment_ledger_id BIGINT UNSIGNED NULL COMMENT '用户手续费扣款流水' AFTER fee_paid_at,
    ADD INDEX idx_asset_unlock_records_fee_ledger (unlock_fee_payment_ledger_id),
    ADD CONSTRAINT fk_asset_unlock_records_fee_ledger FOREIGN KEY (unlock_fee_payment_ledger_id) REFERENCES wallet_ledger(id);

UPDATE asset_unlock_records
SET fee_paid_status = 'pending', fee_paid_at = NULL, unlock_fee_payment_ledger_id = NULL
WHERE unlock_fee_enabled = TRUE
  AND fee_paid_status = 'paid'
  AND status <> 'released';
