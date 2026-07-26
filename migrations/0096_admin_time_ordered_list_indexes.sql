-- 后台订单类列表改为服务端分页后默认按 created_at 倒序且常不带筛选，
-- 以下六张表此前均无以 created_at 为前导列的索引，深翻页与新增的 COUNT 会退化为全表扫描加文件排序。
-- 这些表只追加不改写 created_at，索引键单调递增，维护成本低。

ALTER TABLE spot_orders
    ADD INDEX idx_spot_orders_created_id (created_at, id);

ALTER TABLE spot_trades
    ADD INDEX idx_spot_trades_created_id (created_at, id);

ALTER TABLE prediction_orders
    ADD INDEX idx_prediction_orders_created_id (created_at, id);

ALTER TABLE seconds_contract_orders
    ADD INDEX idx_seconds_contract_orders_created_id (created_at, id);

ALTER TABLE earn_subscriptions
    ADD INDEX idx_earn_subscriptions_created_id (created_at, id);

ALTER TABLE quick_recharge_orders
    ADD INDEX idx_quick_recharge_orders_created_id (created_at, id);
