-- 杠杆引擎目前只有逐仓风险模型，历史 cross 配置不能继续对外暴露。
UPDATE margin_products
SET margin_mode = 'isolated',
    margin_modes = JSON_ARRAY('isolated')
WHERE margin_mode = 'cross'
   OR JSON_CONTAINS(margin_modes, JSON_QUOTE('cross'));

-- 返佣结算资产由产生佣金的业务在同一事务中确定，避免结算时依赖特定业务订单表。
ALTER TABLE agent_commission_records
    ADD COLUMN payout_asset_id BIGINT UNSIGNED NULL AFTER source_amount,
    ADD INDEX idx_agent_commission_records_payout_asset (payout_asset_id),
    ADD CONSTRAINT fk_agent_commission_records_payout_asset
        FOREIGN KEY (payout_asset_id) REFERENCES assets(id);

-- 历史闪兑返佣保留原有的入账币种，后续竞猜订单会在创建时直接写入该字段。
UPDATE agent_commission_records records
INNER JOIN convert_orders orders
        ON records.source_type = 'convert_order'
       AND orders.quote_id = records.source_id
SET records.payout_asset_id = orders.from_asset
WHERE records.payout_asset_id IS NULL;
