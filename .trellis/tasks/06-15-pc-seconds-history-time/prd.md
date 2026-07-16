# PC端秒合约历史持仓显示时间

## Goal

PC 端秒合约历史持仓列表需要显示订单时间。当前历史表已有“时间”列，但后端秒合约订单响应没有暴露 `created_at`，PC adapter 也把 `createTime` 固定为 `0`，导致时间列显示 `--`。

## Requirements

* 秒合约订单响应增加订单创建时间 `created_at`，使用已有 unix millis 序列化方式。
* 用户订单列表、后台订单列表、订单详情、幂等回放、锁单查询等所有 `SecondsContractOrderResponse` 查询都必须 select `orders.created_at`，避免 `sqlx::FromRow` 缺列。
* PC `BackendSecondsOrder` 类型补充 `created_at`，`mapSecondsOrdersToPcOrders` 把它映射为 `createTime`。
* PC 历史持仓表继续使用现有 `formatTime(order.createTime)` 显示时间。
* 兼容旧字段：如历史数据或旧接口只有 `opened_at` / `time` 时，也应尽量映射为 `createTime`。

## Acceptance Criteria

* [ ] `/api/v1/seconds-contracts/orders` 返回的订单包含 `created_at` 毫秒时间戳。
* [ ] PC 秒合约历史订单映射出的 `createTime` 不再是固定 `0`。
* [ ] 历史持仓时间列可以显示真实本地时间。
* [ ] 后端秒合约订单列表测试覆盖 `created_at`。
* [ ] PC adapter 测试覆盖 `created_at -> createTime`。

## Definition of Done

* 相关 Rust 和 PC TypeScript 测试通过。
* `cargo fmt` / PC type-check 通过。
* `docs/superpowers/PROGRESS.md` 更新。

## Technical Approach

* 在 `SecondsContractOrderResponse` 添加 `created_at: DateTime<Utc>` 并标注 `#[serde(with = "unix_millis")]`。
* 将所有返回 `SecondsContractOrderResponse` 的 SQL SELECT 末尾从 `orders.expires_at` 扩展为 `orders.expires_at, orders.created_at`。
* PC adapter 中给 `BackendSecondsOrder` 增加可选 `created_at` / `opened_at` / `time`，映射 `createTime` 时按优先级取第一个有效时间。

## Decision (ADR-lite)

**Context**: PC 历史持仓表已经有时间列，缺失的是跨层订单时间字段。
**Decision**: 后端暴露订单创建时间，PC adapter 映射后复用现有时间列渲染。
**Consequences**: 订单列表和详情响应多一个向后兼容字段；所有 `SecondsContractOrderResponse` 查询必须保持字段集合一致。

## Out of Scope

* 不调整秒合约历史持仓表整体样式。
* 不新增独立结算时间列。
* 不修改订单分页逻辑。

## Technical Notes

* 相关文件：`src/modules/seconds_contract/routes.rs`, `tests/seconds_contract_routes.rs`, `pc/src/api/backendAdapters.ts`, `pc/tests/backendAdapters.test.ts`, `pc/src/views/SecondOptions.vue`。
* 历史表当前位于 `pc/src/views/SecondOptions.vue`，已渲染 `formatTime(order.createTime)`。
