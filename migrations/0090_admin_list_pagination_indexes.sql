-- 后台列表改为服务端分页后，深翻页与 COUNT(*) 都依赖排序列索引，以下四张表的默认排序此前无索引可用。

-- 资金流水列表默认 ORDER BY created_at DESC, id DESC 且常不带筛选。
ALTER TABLE wallet_ledger
    ADD INDEX idx_wallet_ledger_created_id (created_at, id);

-- 审计日志列表默认 ORDER BY created_at DESC, id DESC，原索引以 admin_id 为前缀。
ALTER TABLE admin_audit_logs
    ADD INDEX idx_admin_audit_logs_created_id (created_at, id);

-- 风控事件列表默认 ORDER BY created_at DESC, id DESC，原索引以 user_id/decision 为前缀。
ALTER TABLE risk_events
    ADD INDEX idx_risk_events_created_id (created_at, id);
