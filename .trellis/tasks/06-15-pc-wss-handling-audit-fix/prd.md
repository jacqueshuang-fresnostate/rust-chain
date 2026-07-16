# PC端WSS处理统一审计与修复

## Goal

PC 端多个页面都依赖实时行情 WebSocket，但当前连接生命周期、模块语义和订阅取消处理不一致，导致页面切换、秒合约/杠杆行情、K 线订阅等场景容易出现不实时更新、重复订阅或误用旧示例 socket。目标是在不修改后端 WS 协议的前提下，把 PC 端 WSS 按业务拆成现货、杠杆、秒合约三条独立订阅链路，便于后期对某一个业务做特殊处理。

## What I Already Know

- 用户反馈 PC 端很多 WSS 处理都有问题。
- 后端公开 WS 合约只有 market feed 频道：`ticker`、`depth`、`trade`、`kline`。
- PC 端目前主要通过 `pc/src/api/stomp.ts` 的共享单例连接 `/ws/public`。
- 上一个任务已修复现货页 direct payload/envelope payload 解析和 ticker compact symbol 合并。
- `Home.vue` 在卸载时调用 `stompService.disconnect()`，会影响全局共享连接。
- `TVChart.vue` 和 `MarketTrades.vue` 暴露 `market|second|swap` 模块参数，但 `stompService` 实际只处理 `market`。
- `SecondOptions.vue` 秒合约 K 线当前传 `module="market"`，秒合约 HTTP 行情也复用 market ticker。
- `api/socket.ts` 里有一个未使用的 Binance 示例 WebSocket singleton，容易误导。
- 用户明确希望现货、杠杆、秒合约拆成独立订阅链接，以便后期某个业务可以单独做特殊处理。

## Requirements

- PC 端 WSS 应提供业务级连接入口：`spot`、`margin`、`seconds`。
- 三个业务入口当前都可以连接后端 `/ws/public`，但前端连接实例、订阅池、重连状态和后续特殊处理入口必须彼此独立。
- 页面卸载只能取消当前业务/当前组件自己的订阅，不能关闭其他业务的 WSS 链路。
- 现货、秒合约、杠杆这类需要实时价格的页面，应按业务使用对应 WSS client，并按产品配置过滤允许展示的交易对。
- `TVChart` 的订阅和取消订阅 key 必须一致，切换交易对或周期后不能遗留旧 K 线订阅。
- `MarketTrades`、`TVChart` 的 `module` 行为必须和业务 WSS client 一致；不能因为传入 `second` / `margin` 就静默空订阅。
- 需要清理或禁用未使用的 Binance 示例 `api/socket.ts`，避免未来误接外部 WSS。
- 补充回归测试覆盖共享连接生命周期、模块归一化、重连后重新订阅、K 线取消订阅。

## Acceptance Criteria

- [ ] 从 Home 跳转到交易页或从交易页返回时，不会因为 Home unmount 清空其他业务行情订阅。
- [ ] 现货、杠杆、秒合约分别维护独立 WSS 连接实例和订阅池。
- [ ] `seconds` / `margin` 模块消费者不会静默收到 no-op 订阅；当前底层可复用后端 market topic，但必须走各自业务 client。
- [ ] 任一业务 WebSocket 异常关闭后，该业务已有订阅可以在重连后重新发送订阅命令，且不影响其他业务连接。
- [ ] KLineChartPro 取消订阅时能够正确取消对应 K 线订阅。
- [ ] 秒合约页面继续只展示秒合约产品交易对，但实时价格/K 线从统一 market feed 更新。
- [ ] 未使用的 Binance 示例 socket 不再暴露成可误用的生产 singleton。
- [ ] 相关 PC 单元测试和 type-check 通过。

## Definition of Done

- 测试添加/更新。
- `npm run type-check` 通过。
- 变更记录写入 `docs/superpowers/PROGRESS.md`。
- 不引入后端 WS 协议变更；当前三条业务连接都可以连接 `/ws/public`。

## Technical Approach

推荐采用“业务级 WSS client + 后端 market feed 适配”的方案：

- 在 `stompService` 内部维护 `spot`、`margin`、`seconds` 三个业务 client；每个 client 有自己的 socket、订阅 map、重连 timer、ticker watcher。
- 当前三类业务 client 的 backend path 都是 `/ws/public`，发送给后端的 channel 仍是 `ticker/depth/trade/kline`；未来如果后端提供 `/ws/spot`、`/ws/margin`、`/ws/seconds`，只需要改业务 client 的 endpoint map。
- 将连接关闭改为业务级生命周期策略：页面只取消自己业务下的订阅；全局断开仅用于显式应用级 teardown 或测试。
- 为每个业务 client 的 WS close 增加轻量自动重连，并在重连后 flush 该业务仍存在的订阅。
- 修复 `TVChart` 订阅 key 生成函数，subscribe/unsubscribe 统一从 symbol object/string 中取 ticker/symbol。
- 秒合约和杠杆页面当前继续使用后端 market feed，但代码命名和测试要明确它们走各自业务连接实例。

## Decision (ADR-lite)

**Context**: 后端公开 WS 当前只提供 market feed，不存在独立 `seconds` 或 `margin` WS namespace。PC 端的模块参数和页面语义已经开始发散，同时用户希望后期可以按某个业务做特殊处理。

**Decision**: 本任务不扩展后端协议，但前端先拆出 `spot`、`margin`、`seconds` 三条独立业务 WSS 链路；三条链路当前都适配 `/ws/public` market feed。

**Consequences**: 秒合约、杠杆等产品页面暂时仍消费后端 market feed payload，但不会共享同一个前端 socket/订阅池；如果未来后端增加独立产品行情频道，可以在业务 endpoint map 上扩展，而不是每个组件分别改。

## Out of Scope

- 不修改后端 WebSocket 协议。
- 不新增私有用户 WS 推送。
- 不重构交易页 UI。
- 不处理订单撮合或钱包流水等非行情实时推送。

## Research References

- [`research/pc-wss-audit.md`](research/pc-wss-audit.md) — PC WSS 使用点、后端公开 WS 合约、主要问题和推荐 MVP。

## Technical Notes

- 关键前端文件：`pc/src/api/stomp.ts`、`pc/src/components/chart/TVChart.vue`、`pc/src/components/trade/MarketTrades.vue`、`pc/src/views/Home.vue`、`pc/src/views/Trade.vue`、`pc/src/views/Contract.vue`、`pc/src/views/SecondOptions.vue`。
- 后端合约参考：`src/modules/events/routes.rs`、`src/modules/events/mod.rs`、`tests/events_ws.rs`。
