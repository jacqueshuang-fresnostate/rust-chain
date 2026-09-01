# 后台强平记录字段链路调研

## 当前前端

- 路由 `/admin/margin/liquidations` 使用 `resourceConfigs.marginLiquidations`。
- 当前列表展示 `id`（记录 ID）、`position_id`（仓位 ID）、`user_id`（用户 ID）以及标记价、权益、利息、返还金额、原因和强平时间。
- 行操作仍依赖隐藏的 `record.id` 请求 `/admin/api/v1/margin/liquidations/:id`，因此只应移除可见列，不应删除 API 中的记录 ID。
- 现有筛选支持用户 ID、邮箱、交易对、仓位 ID；用户仅要求调整显示列，筛选能力应保留。
- `web/src/admin/resources/resourceConfigs.tsx` 与对应测试存在上一项 Admin 任务的未提交修改，本任务必须增量适配，禁止覆盖或回滚。

## 当前后端

- DTO `AdminMarginLiquidationResponse` 目前包含记录、仓位、用户、产品、交易对等 ID 和完整强平快照，但没有邮箱与交易对符号。
- 列表与详情共用 `admin_margin_liquidation_query()`，因此在该查询中关联 `users` 和 `trading_pairs`，即可让两个接口一致返回 `email` 与 `symbol`。
- `users.email` 允许为空，应映射为 `Option<String>`；`trading_pairs.symbol` 非空，应映射为 `String`。
- 强平记录表对用户和交易对已有外键，关联不会改变记录范围；查询引入别名后必须限定 SELECT、WHERE 与 ORDER BY 的列来源，避免 `id` 歧义。
- COUNT 查询只计算记录数，不需要为展示字段额外 JOIN；邮箱筛选继续复用现有用户子查询。

## 结论

- 后端采用加法响应合同：保留所有现有字段，新增 `email` 和 `symbol`。
- Admin 列表首列改为“邮箱”“交易对”，移除三个内部 ID 表头；详情按钮继续使用未展示的 `id`。
- 需要同时补充后端路由测试和 Admin 行为测试，证明列表显示与详情动作没有回归。
