-- 佣金自动结算按 status + created_at 扫描待结算记录，原 (agent_id, status) 索引无法命中。
ALTER TABLE agent_commission_records
    ADD INDEX idx_agent_commission_records_status_created (status, created_at);

-- 仪表盘现货挂单 KPI 只按 status 聚合，原索引以 pair_id 为前缀无法命中。
ALTER TABLE spot_orders
    ADD INDEX idx_spot_orders_status (status);
