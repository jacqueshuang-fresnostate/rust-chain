-- 借款利率必须在借款开始时固化到持仓上，否则管理员改配产品利率会追溯整个未计费窗口。
-- 新列只做快照，产品表仍是新开仓位的利率来源，历史仓位的资金含义不变。
ALTER TABLE margin_positions
    ADD COLUMN hourly_interest_rate DECIMAL(18,8) NOT NULL DEFAULT 0 COMMENT '开仓时快照的小时利率' AFTER borrowed_amount;

-- 存量持仓没有历史快照，只能按当前产品利率回填：这正是它们此前一直在用的口径。
UPDATE margin_positions positions
INNER JOIN margin_products products ON products.id = positions.product_id
SET positions.hourly_interest_rate = products.hourly_interest_rate;
