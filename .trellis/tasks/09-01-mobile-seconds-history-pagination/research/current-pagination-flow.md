# 秒合约历史分页现状与修复边界

## 当前链路

1. `/seconds/history` 创建 `createSecondsHistoryRequestLifecycle`。
2. 生命周期固定调用 `fetchSecondsOrders(100)`。
3. Mobile API 仅发送 `{ limit }`。
4. Rust `ListQuery` 只有 `limit`，路由调用 `list_user_orders_use_case(pool, user_id, limit)`。
5. 基础设施 SQL 使用稳定排序与 `LIMIT ?`，没有 `OFFSET`。
6. 页面收到最多 100 条后，在客户端过滤活动状态并按方向筛选。

因此当前既不是服务端分页，也无法访问第 101 条及更早订单。

## 可复用能力

- `route_limit`：默认 50，夹在 1–100。
- `route_offset`：默认 0，封顶 100000。
- 用户订单 SQL 已使用 `ORDER BY orders.created_at DESC, orders.id DESC`，具备 offset 分页所需的确定性次级排序。
- Admin 订单分页已有 limit/offset 和 total，但用户无限列表不需要每页 COUNT。
- Mobile 钱包账单已有首屏/追加状态、offset 推进、会话隔离和去重模式，可复用其状态分层思想；本页按用户要求改用 IntersectionObserver 自动触底，而不是按钮分页。

## 后端设计

- 保留产品目录 `ListQuery { limit }`。
- 新增用户订单 `ListOrdersQuery { limit, offset }`。
- 路由归一后把 limit/offset 交给应用层。
- 应用层请求 `limit + 1` 条；超过 limit 即 `has_more=true`，随后截断。
- 用户响应从 `{ orders }` 扩展为 `{ orders, has_more }`。
- SQL 加 `OFFSET ?`，用户 ID 仍是第一过滤条件。

## Mobile 设计

- 保留 `fetchSecondsOrders(limit)` 给交易工作台。
- 新增分页函数，返回 `{ orders, offset, nextOffset, hasMore }`。
- 历史生命周期改为精确 token + request generation；分页请求参数使用对象，避免 limit/offset 位置写反。
- 合并使用订单 ID，页内/页间重复均只保留一条，后到行覆盖旧行。
- offset 按服务端原始返回条数推进，而不是按筛选后或去重后的条数推进。
- `hasMore` 必须同时满足服务端提示、非空页和实际合并有进展；这样旧后端忽略 offset 时最多多请求一页，不会无限循环。
- Observer 只在 `hasMore` 且无追加错误时触发；追加错误保留卡片并由局部按钮重试。

## 风险与验证

- **重复触发**：Observer 可能连续回调；视图必须同步设置 `loadingMore` 并在入口 guard。
- **会话泄漏**：只检查 boolean 登录态不足以防 A→B 换号；生命周期捕获完整 token。
- **追加失败**：不得清空首屏或显示整页错误。
- **筛选空页**：方向筛选无匹配不等于服务端无下一页，哨兵必须独立于列表/空态分支存在。
- **旧服务兼容**：缺少 `has_more` 时按满页推断，并以“无新增 ID”作为硬停止条件。
- **后端集成测试**：同一用户至少两条、另一用户一条；分别请求 offset 0/1，验证页内容、has_more、时间字段和越权隔离。
