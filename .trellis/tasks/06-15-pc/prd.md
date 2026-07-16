# PC端杠杆路由限制为已启用交易对

## Goal

PC 端合约/杠杆页面 `/contract/:symbol?` 只能访问后台 `/margin/products` 返回的杠杆交易对。当前后台只配置 BTC 时，直接访问 `/contract/ETH_USDT` 仍然渲染 ETH 行情和深度，这是错误的。

## What I already know

* PC 合约路由定义在 `pc/src/router/index.ts`，路径为 `/contract/:symbol?`。
* `pc/src/views/Contract.vue` 当前会立即把 URL symbol 写入 `marketStore.activeSymbol`。
* `contractStore.loadCoins()` 会从 `/api/v1/margin/products` 加载可用杠杆产品。
* 当前加载完产品后，如果 URL symbol 不在产品列表里，只是不设置 `contractStore.activeCoin`，不会纠正 URL 或 active symbol。

## Requirements

* `/contract/:symbol?` 的 symbol 必须以 `/margin/products` 返回的交易对为准。
* 如果 URL 缺少 symbol，应跳转到第一个可用杠杆交易对。
* 如果 URL symbol 不属于可用杠杆交易对，应使用 `router.replace` 跳转到第一个可用杠杆交易对。
* 切换交易对应继续只从 `contractStore.coins` 列表中选择。
* 若没有任何杠杆产品，不应继续拉取任意交易对的行情、深度或订阅。

## Acceptance Criteria

* [ ] 后台只有 `BTC/USDT` 杠杆产品时，访问 `/contract/ETH_USDT` 会被替换到 `/contract/BTC_USDT`。
* [ ] 访问 `/contract` 会替换到第一个可用杠杆交易对。
* [ ] 合法交易对 URL 保持不变，并设置为当前 active coin。
* [ ] 没有杠杆产品时，页面不会订阅非法 symbol。
* [ ] 增加 PC 源码级回归测试覆盖该路由约束。

## Definition of Done

* 修改集中在 PC 合约页/store 必要位置。
* PC 类型检查通过。
* PC 目标测试通过。
* `docs/superpowers/PROGRESS.md` 更新。

## Technical Approach

在 `Contract.vue` 中把 URL symbol 写入 active symbol 的逻辑改为产品加载后的校验流程：

1. 从 route 读取并规范化 symbol。
2. 加载 `contractStore.coins` 后用 `getCoinBySymbol` 判断是否可用。
3. 找不到时选择 `contractStore.coins[0]` 并 `router.replace` 到它的 URL。
4. 只有 resolved coin 存在时才刷新深度和订阅行情。

## Decision (ADR-lite)

**Context**: 合约页复用了市场 active symbol，因此任意现货交易对 URL 都可能污染合约页状态。
**Decision**: 以 `/margin/products` 返回的产品列表作为合约页 symbol 白名单，在页面初始化和 route watcher 中统一纠正。
**Consequences**: 前端不会再打开未配置杠杆产品的交易对；当后台没有任何杠杆产品时页面保持空状态，不发起非法交易对订阅。

## Out of Scope

* 不新增后端 API。
* 不修改后台杠杆产品配置。
* 不重构合约页整体 UI。

## Technical Notes

* 相关文件：`pc/src/views/Contract.vue`, `pc/src/stores/contract.ts`, `pc/src/api/contract.ts`, `pc/tests/*`。
* `Contract.vue` 当前 `watch(route.params.symbol, { immediate: true })` 是问题入口。
* `contractStore.getCoinBySymbol()` 已存在，可复用为产品白名单判断。
