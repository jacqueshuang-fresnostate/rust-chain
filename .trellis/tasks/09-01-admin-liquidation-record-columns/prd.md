# 后台强平记录列调整

## Goal

让后台“强平记录”列表以管理员可识别的业务信息展示用户邮箱和交易对，不再把记录 ID、仓位 ID、用户 ID 作为可见表格列，同时保留详情查询和现有筛选能力。

## What I Already Know

- 当前页面由 `resourceConfigs.marginLiquidations` 驱动，前三列正是记录 ID、仓位 ID、用户 ID。
- 行操作依赖记录对象中的 `id` 打开详情，因此 ID 只从表格隐藏，API 字段继续保留。
- 当前强平接口 DTO/SQL 没有返回邮箱与交易对符号，前端无法可靠地自行展示。
- 强平记录已有用户与交易对外键；后端可在共用列表/详情查询中关联 `users` 与 `trading_pairs`。
- 用户描述的是“显示”调整，现有用户 ID、邮箱、交易对、仓位 ID 筛选继续保留。
- 工作区存在上一项 Admin 任务的未提交改动，本任务不得覆盖、回滚或混入其业务修改。

## Requirements

### Backend

- `GET /admin/api/v1/margin/liquidations` 的每条记录新增 `email: string | null` 与 `symbol: string`。
- `GET /admin/api/v1/margin/liquidations/:id` 返回相同的新增字段，列表与详情保持同一 DTO。
- 现有 `id`、`position_id`、`user_id` 等字段保持兼容，不能破坏详情按钮或其他调用方。
- 查询使用强平记录的用户与交易对外键获得展示值，并在 JOIN 后显式限定有歧义的列。
- 邮箱为空时返回 null，不伪造邮箱；数据库或关联合同异常继续按现有错误处理。

### Admin

- 强平记录表格移除“记录ID”“仓位ID”“用户ID”三列。
- 表格新增“邮箱”和“交易对”两列，并优先放在业务数据列前部。
- 邮箱和交易对直接使用接口返回字段，不在前端额外请求用户或交易对目录。
- 保留标记价、权益、累计利息、返还金额、原因、强平时间以及详情操作。
- 保留现有用户 ID、邮箱、交易对、仓位 ID 和分页筛选；本任务不调整筛选流程。
- 邮箱为空时使用资源表既有空值展示规则，详情按钮仍通过隐藏的记录 ID 正常打开。

## Acceptance Criteria

- [x] 强平列表 API 的目标记录同时返回正确的用户邮箱和交易对 symbol。
- [x] 强平详情 API 返回同一组 email/symbol，且旧 ID 与强平快照字段仍存在。
- [x] Admin 强平记录表头不再包含记录 ID、仓位 ID、用户 ID。
- [x] Admin 表格显示接口返回的邮箱和交易对，并保留原有业务金额、原因和时间列。
- [x] 点击“查看详情”仍使用记录 ID 请求正确详情接口。
- [x] 现有筛选参数和分页行为不变。
- [x] 后端聚焦测试、Admin 聚焦测试、类型检查、lint 与相关门禁通过。

## Definition of Done

- 后端 DTO 与共用列表/详情查询完成加法字段扩展。
- Admin 资源列和行为测试完成。
- 相关 Backend/Admin code-spec 与 `docs/superpowers/PROGRESS.md` 更新。
- 未修改强平结算、钱包、仓位状态或数据库结构。

## Technical Approach

- 在 `AdminMarginLiquidationResponse` 新增可空邮箱和交易对 symbol。
- 将 `admin_margin_liquidation_query()` 改为从强平记录关联用户与交易对，并让列表、详情继续复用同一查询。
- 前端仅调整 `marginLiquidations.columns`；记录对象继续保留 ID 给行操作使用。
- 使用真实渲染测试断言业务列可见、内部 ID 表头不可见和详情请求不回归。

## Decision (ADR-lite)

**Context**：前端只有内部 ID，无法把强平记录直接对应到用户和交易对；额外目录请求会引入缓存、竞态与 N+1 映射问题。

**Decision**：由强平列表/详情接口直接返回 email 和 symbol，Admin 表格隐藏内部 ID 但保留底层字段兼容。

**Consequences**：接口只做加法变更，旧调用方不受影响；后台查询多两个只读关联，但避免前端额外请求和不一致映射。

## Out of Scope

- 不删除 API 或数据库中的记录 ID、仓位 ID、用户 ID。
- 不移除现有 ID 筛选条件。
- 不修改强平算法、结算快照、钱包记账、仓位状态或数据库表结构。
- 不重做强平详情抽屉和其他杠杆后台页面。

## Technical Notes

- Admin：`web/src/admin/resources/resourceConfigs.tsx`、`web/src/admin/resources/resourceConfigs.test.tsx`。
- Backend：`src/modules/admin/presentation/dashboard_audit.rs`、`src/modules/admin/infrastructure/margin.rs`、`tests/admin_routes.rs`。
- 调研：`research/current-liquidation-record-flow.md`。
