# Admin 新币预热转申购现状

## 现有合同

- `LifecycleStatus::transition_to` 只接受 `preheat -> subscription -> distribution -> listed`。
- `update_admin_new_coin_lifecycle` 会在事务中锁定项目，校验项目为 active，执行单向迁移，再同时记录生命周期事件和 Admin 审计。
- Admin 已注册 `PATCH /admin/api/v1/new-coins/:id/lifecycle`，因此不需要新后端端点或数据库改动。

## 前端根因

- 项目路由使用 `createBrowserRouter`。
- 新币列表的「配置与操作」却通过 `window.location.hash = "/admin/new-coins/actions?..."` 导航。
- 这只会修改当前 URL 的 fragment，不会让 BrowserRouter 匹配 `/admin/new-coins/actions`，所以从列表看起来就是「无法编辑/操作」。
- 独立操作页已会从 `window.location.search` 读取 `project_id`，且生命周期默认目标已是 `subscription`。

## 实现边界

- 列表快捷操作仅在 `lifecycle_status === "preheat"` 时显示，不在前端提供跳级。
- `status !== "active"` 时禁用快捷操作；后端继续做权威复核。
- 快捷操作继续使用 `AdminRequestActionBoundary`，与 endpoint/method 解析出的 `new_coin.projects.write` 权限一致。
- 成功后调用 `helpers.reload()`，不在客户端乐观改写行数据。
- 使用 `useNavigate` 处理配置页入口，同时用内存路由测试 pathname/search，避免再回归为 hash 路由。

## 相关文件

- `web/src/app/router.tsx`
- `web/src/admin/resources/actions/newCoins.tsx`
- `web/src/admin/actions/NewCoinActions.tsx`
- `web/src/admin/resources/resourceConfigs.tsx`
- `web/src/admin/access.tsx`
- `.trellis/spec/admin/ui-system.md`
