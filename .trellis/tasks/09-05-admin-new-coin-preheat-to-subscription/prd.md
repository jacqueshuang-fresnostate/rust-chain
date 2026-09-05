# 修复后台新币项目预热转申购

## Goal

让管理员能从「新币项目」列表中清晰、可用地将当前处于 `preheat` 的启用项目推进到 `subscription`，同时修复现有「配置与操作」入口在 BrowserRouter 下只写 hash、实际不会进入操作页的问题。

## Requirements

- 保留后端已有的单向生命周期合同：`preheat -> subscription -> distribution -> listed`。
- 对列表中处于 `preheat` 且项目状态为 `active` 的记录，显示可确认的「开始申购」行操作。
- 「开始申购」必须通过 `ConfirmAction` 要求原因，调用 `PATCH /admin/api/v1/new-coins/:id/lifecycle`，且只发送 `lifecycle_status: "subscription"` 与原因。
- 更新成功后重载列表，以后端权威状态为准。
- 非预热项目不显示「开始申购」，禁用项目不允许执行。
- 「配置与操作」使用 React Router 导航到 `/admin/new-coins/actions?project_id=:id`，不再写入 URL hash。
- 重复行控件带项目标识的中文可访问名称。

## Acceptance Criteria

- [x] 预热且启用的项目行显示「开始申购」和「配置与操作」。
- [x] 确认「开始申购」后发出精确 PATCH 请求并重载列表。
- [x] 申购中、派发中或已上市项目不显示该快捷操作。
- [x] 禁用的预热项目不能发出生命周期 PATCH。
- [x] 点击「配置与操作」会更新 BrowserRouter 路由与 query，不产生错误 hash 导航。
- [x] Web 聚焦测试、类型检查、lint 与 `git diff --check` 通过。

## Definition of Done

- 生产代码、回归测试、Admin 规范和进度记录同步。
- 不修改后端迁移图、数据表或其他新币业务流程。
- 保留工作树中用户已有的 Mobile 改动。

## Technical Approach

在现有 `NewCoinProjectRowActions` 中复用 `AdminRequestActionBoundary`、`ConfirmAction`、`submitAction` 和 `RowActionHelpers.reload`，实现严格的预热到申购快捷操作。用 `useNavigate` 替代 `window.location.hash`，使入口与项目当前的 `createBrowserRouter` 一致。

## Decision (ADR-lite)

**Context**: 后端已有带行锁、迁移校验与审计的生命周期 PATCH，前端也有独立操作页，但列表入口使用了与 BrowserRouter 不匹配的 hash 导航。  
**Decision**: 修复路由并在列表增加严格限定为 `preheat -> subscription` 的快捷操作。  
**Consequences**: 管理员可以一步开启申购，其他阶段仍通过独立操作页按后端单向迁移处理；不引入通用任意编辑，避免绕过生命周期合同。

## Out of Scope

- 任意字段的通用新币项目编辑表单。
- 批量更新多个预热项目。
- 跳级、回退或同状态重放。
- 自动派发、自动上市或修改申购规则。

## Technical Notes

- 现有后端合同位于 `src/modules/new_coin/domain.rs` 与 `src/modules/admin/application/new_coin.rs`。
- 现有列表行操作位于 `web/src/admin/resources/actions/newCoins.tsx`。
- 现有独立操作页位于 `web/src/admin/actions/NewCoinActions.tsx`。
- 根因与边界参见 [`research/current-flow.md`](research/current-flow.md)。
