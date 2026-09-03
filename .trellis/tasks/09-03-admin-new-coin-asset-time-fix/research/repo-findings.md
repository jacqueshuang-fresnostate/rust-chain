# 新币项目资产与时间配置调查

## 根因

1. 后端 `CreateNewCoinProjectRequest` 将 `quote_asset_id` 定义为必填 `u64`；Axum 在字段缺失时直接返回 JSON 反序列化错误。
2. Admin `CreateNewCoinProjectAction` 的状态、表单和请求体都没有 `quoteAssetId`，其测试甚至仍断言请求体不含该字段，属于前后端契约漂移。
3. 创建页以及 `NewCoinActions` 的绝对时间字段使用普通文本输入，并以裸 Unix 毫秒校验；项目已有 `datetime-local` 的可复用实现模式。

## 资产资格事实

- `useAssetOptions` 请求 `/admin/api/v1/assets` 时只传 `status=active` 和分页参数。
- `AdminAssetQuery` 与 `list_admin_assets` SQL 没有充提开关筛选。
- 新币创建时 `load_active_new_coin_asset_precision_in_tx` 只读取 `precision_scale` 与 `status`，不检查 `deposit_enabled` 或 `withdraw_enabled`。
- 因此业务合同已经是：active 资产可以用于新币项目，充提开关与资格无关。需要用前后端回归测试固定该行为，而不是放宽 active 状态要求。

## 最小实现方案

- Admin 新币创建状态新增 `quoteAssetId`，渲染“计价资产”下拉，排除当前项目资产，并在项目资产切换产生冲突时清空计价资产。
- `isNewCoinProjectCreatable` 和最终 payload 都要求有效、不同的计价资产。
- 将创建页与生命周期动作页的上市/固定解禁时间改成 `AdminTextInput type="datetime-local"`。
- 使用单一纯函数把有效本地日期时间转换为 Unix 毫秒，并分别支持 required/optional 语义，避免两个页面再次漂移。
- 更新 Admin UI 测试，使用 `fireEvent.change(..., { value: 'YYYY-MM-DDTHH:mm' })` 并断言 `new Date(value).getTime()`。
- 在 Rust 新币创建路由现有成功测试中，将项目资产的 `deposit_enabled`、`withdraw_enabled` 设为 false 后继续断言创建成功。

## 关键文件

- `web/src/admin/resources/actions/newCoins.tsx`
- `web/src/admin/actions/NewCoinActions.tsx`
- `web/src/admin/resources/actions/shared.tsx`
- `web/src/admin/resources/resourceConfigs.test.tsx`
- `web/src/admin/actions/helperCopy.test.tsx`
- `src/modules/admin/presentation/new_coin.rs`
- `src/modules/admin/application/new_coin.rs`
- `src/modules/admin/infrastructure/new_coin.rs`
- `tests/admin_routes.rs`

