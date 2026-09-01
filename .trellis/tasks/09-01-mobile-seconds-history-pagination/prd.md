# 手机端秒合约历史订单分页加载

## Goal

将手机端 `/seconds/history` 从一次请求最多 100 条改为服务端分页，并在用户滚动接近列表底部时自动加载下一页，同时保证会话隔离、订单去重、无更多数据停止和追加失败可重试。

## What I Already Know

- 当前历史页通过 `fetchSecondsOrders(100)` 一次读取，前端再用 `historicalSecondsOrders()` 排除活动订单。
- 用户订单接口目前只接受 `limit`，后端查询也只有 `LIMIT`，因此手机端单独增加 `offset` 参数不会真正翻页。
- 用户订单查询已有稳定排序：`created_at DESC, id DESC`；后台订单列表已经使用统一的 `route_offset` 归一规则。
- 现有历史请求生命周期只按“是否登录”隔离，分页后还需要绑定精确 session token，避免换号后的迟到页写入当前列表。
- Mobile 已有完整的历史页面设计和方向筛选，本任务只改变数据获取、列表尾部加载状态与必要的辅助样式，不重做卡片视觉。
- 工作区中已有上一项 Admin 任务的未提交改动；本任务不得修改、回滚或混入这些文件。

## Requirements

### Backend

- `GET /api/v1/seconds-contracts/orders` 支持 `limit` 与 `offset`，继续只读取当前认证用户。
- `limit` 继续按 1–100 归一，`offset` 使用统一上限规则。
- 响应保留 `orders` 并新增 `has_more: boolean`；通过多取一条判断，不为每一页额外执行 COUNT。
- 每页保持 `created_at DESC, id DESC` 的稳定顺序，SQL 同时使用 `LIMIT` 与 `OFFSET`。
- 现有不传 `offset` 的 PC/Mobile 调用保持第一页兼容。

### Mobile API and lifecycle

- 新增分页读取适配器，显式发送 `limit`、`offset`，映射 `orders`、`has_more` 和客户端下一偏移量。
- 现有 `fetchSecondsOrders(limit)` 保持原签名，秒合约交易工作台不切换到无限滚动。
- 历史分页生命周期绑定精确 session token，迟到请求在换号、退出、重试覆盖或卸载后不得提交。
- 追加页按订单 ID 合并去重，后到的权威行可更新同 ID 记录。
- 如果兼容旧服务时缺少 `has_more`，可按“返回条数等于页大小”推断；若一整页没有新增 ID，必须停止继续请求，避免旧服务忽略 offset 后形成无限循环。

### Mobile view

- 首屏使用固定页大小 20，从 `offset=0` 加载。
- 列表底部使用 `IntersectionObserver` 哨兵，接近底部时自动请求 `nextOffset`。
- 首屏加载与追加加载分别管理状态：追加失败保留已加载订单，并显示局部重试，不切回整页错误。
- 请求进行中禁止重复触发；服务端返回 `has_more=false`、空页或无去重进展后停止观察/加载。
- 访客、换号、退出登录和卸载时重置分页状态并使旧响应失效。
- 方向筛选继续作用于已加载的真实历史订单；当当前筛选暂时无匹配但仍有下一页时，底部哨兵仍可继续加载。
- 仅复用现有国际化文案与 Lucide 图标，不加入表情符号或演示订单。

## Acceptance Criteria

- [x] 用户订单接口的 `limit=1&offset=0` 与 `limit=1&offset=1` 返回不同、顺序稳定且只属于当前用户的订单页。
- [x] 首页面存在下一页时 `has_more=true`，末页为 `false`；旧的不带 offset 调用保持可用。
- [x] `/seconds/history` 首次请求 `{ limit: 20, offset: 0 }`，触底后按返回条数推进 offset 并追加下一页。
- [x] 多次 Observer 回调不会并发重复请求同一页。
- [x] 重叠页按 ID 去重且使用后到的权威记录；完全无新 ID 时终止继续加载。
- [x] 追加失败保留已有卡片并提供局部重试，重试成功后从同一 offset 继续。
- [x] logout、token ABA、较新请求和 unmount 都能使旧页响应失效。
- [x] 已无更多数据时不再触发接口请求。
- [x] Backend 秒合约路由测试、格式/架构检查及 Mobile 聚焦测试、类型检查、全量发布门禁通过。

## Definition of Done

- 后端分页查询、响应 DTO、应用编排和路由测试完成。
- Mobile API、分页生命周期、历史页触底加载和测试完成。
- Backend/Mobile 相关 code-spec 与 `docs/superpowers/PROGRESS.md` 更新。
- 不修改 PC、Admin 业务代码，也不混入上一任务的未提交文件。

## Technical Approach

- 将仅含 `limit` 的产品查询 DTO 与用户订单分页 DTO 分开，避免给产品目录引入无效 offset。
- 应用层以 `limit + 1` 调基础设施查询，截断到请求页大小并计算 `has_more`。
- 在 `secondsOrder.ts` 将历史请求生命周期扩展为 `{ offset, limit } -> page`，并新增分页合并纯函数供竞态、去重和终止行为测试。
- 历史视图维护 `loading`、`loadingMore`、`nextOffset`、`hasMore`、首屏错误和追加错误；观察一个位于列表状态之后的哨兵节点。

## Decision (ADR-lite)

**Context**：一次读取 100 条既无法访问更早历史，也会随着订单量增长增加首屏负担；只有前端切片不是真分页，因为后端当前忽略 offset。

**Decision**：使用现有项目一致的 limit/offset 分页，并通过 limit+1 返回 `has_more`；前端用 IntersectionObserver 自动追加并按 ID 处理边界重叠。

**Consequences**：无需 COUNT 查询，旧客户端保持兼容；offset 分页在新订单插入时可能出现边界重叠，客户端去重可避免重复展示，后续若历史规模需要深翻页可再升级为时间+ID 游标。

## Out of Scope

- 不重做秒合约历史卡片、标题或方向筛选视觉。
- 不修改秒合约交易工作台的活动订单轮询策略。
- 不增加取消、删除或导出历史订单功能。
- 不修改 PC 或 Admin 页面。

## Technical Notes

- Backend：`src/modules/seconds_contract/{presentation,routes,application,infrastructure}.rs`、`tests/seconds_contract_routes.rs`。
- Mobile：`mobile/src/api/seconds.ts`、`mobile/src/core/secondsOrder.ts`、`mobile/src/views/SecondsHistoryView.vue`、`mobile/tests/{seconds-history-view,seconds-api-adapter}.test.ts`。
- 相关规范：`.trellis/spec/backend/{seconds-contracts,quality-guidelines}.md`、`.trellis/spec/mobile/{index,backend-integration}.md`。
