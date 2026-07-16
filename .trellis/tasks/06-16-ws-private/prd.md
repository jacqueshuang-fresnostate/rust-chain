# 实现 /ws/private 私有 WebSocket

## Goal

PC 端需要接入后端已有的 `/ws/private` 私有 WebSocket，让用户级订单、仓位、钱包、秒合约结算等事件能主动触发页面刷新，减少交易页面只依赖提交后刷新或轮询的问题。

## What I Already Know

- 后端已经暴露 `/ws/private?token=<user access token>`，鉴权使用用户 access token query 参数。
- 后端 `PrivateWsAuth` 会拒绝非用户 token，并订阅 `private:user:<user_id>`。
- 后端已经在现货、杠杆、秒合约、新币、理财、钱包等流程里发布 `EventBroadcastMessage::private_user(...)`。
- PC 当前 `stompService` 只维护 `spot`、`margin`、`seconds` 三类 public market WS client，全部连接 `/ws/public`。
- PC 交易页当前没有私有事件 client，因此订单、仓位、秒合约结算主要靠下单后的主动刷新和局部轮询。

## Requirements

- PC `stompService` 新增独立 private WebSocket client，连接 `/ws/private?token=<localStorage token>`。
- private client 不发送 market subscribe 指令；连接成功后接收后端用户事件并分发给本地订阅回调。
- 未登录或没有 token 时，不建立 `/ws/private` 连接，订阅返回可安全取消的 no-op。
- token 变化或登出后，private client 需要能关闭旧连接，避免继续使用旧 token 重连。
- 现货页收到相关私有事件后，复用现有订单刷新逻辑刷新当前/历史委托。
- 杠杆页收到相关私有事件后，复用现有持仓/订单刷新逻辑。
- 秒合约页收到相关私有事件后，刷新当前持仓和历史记录，补齐下单后/结算后的实时更新链路。
- 保留现有 public market WS 的 spot/margin/seconds 隔离行为，不改变行情订阅协议。

## Acceptance Criteria

- [x] `stompService` 能建立 `ws://.../ws/private?token=...` 连接。
- [x] private client 收到 JSON 文本后按原样 `message.body` 分发给订阅者。
- [x] private client 断线后仅在仍有订阅且仍有 token 时重连。
- [x] 登出或 token 缺失时不会连接 `/ws/private`。
- [x] spot/margin/seconds public client 现有测试仍通过。
- [x] 新增测试覆盖 private WS URL、事件分发、重连和 token 缺失行为。
- [x] PC 类型检查通过。

## Technical Approach

- 在 `pc/src/api/stomp.ts` 内复用现有 WebSocket 管理模式，增加 private client 状态和 `subscribePrivate(callback)` / `connectPrivate()` / `disconnectPrivate()` API。
- private WS URL 使用 `APP_CONFIG.BACKEND_API_DOMAIN` 转换为 ws/wss，并将 token 做 `encodeURIComponent`。
- 交易页只根据事件到达触发已有刷新函数，不在前端硬编码所有事件 payload 结构，降低与后端事件类型耦合。

## Out of Scope

- 不新增后端 `/ws/private` 路由；后端已存在。
- 不改变后端私有事件 payload schema。
- 不新增服务端事件类型。
- 不改公共行情 `/ws/public` 订阅协议。

## Technical Notes

- 相关后端文件：`src/modules/events/routes.rs`、`src/modules/events/mod.rs`。
- 相关 PC 文件：`pc/src/api/stomp.ts`、`pc/src/views/Trade.vue`、`pc/src/views/Contract.vue`、`pc/src/views/SecondOptions.vue`、`pc/tests/stomp.test.ts`。
- 参考审计报告：`.trellis/tasks/06-16-pc-trading-pages-api-wss-audit/research/pc-trading-pages-api-wss-audit.md`。
