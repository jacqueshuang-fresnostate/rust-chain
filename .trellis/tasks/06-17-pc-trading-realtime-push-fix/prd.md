# PC交易实时推送优化

## 背景

PC 端现货、杠杆、秒合约页面的实时推送更新不稳定，用户在页面上看到的价格、盘口、成交、委托/持仓可能不会随 WebSocket 事件及时刷新。

## 目标

- 梳理 PC 端交易相关 WebSocket 订阅入口，确认现货、杠杆、秒合约是否订阅了正确的业务频道。
- 修复三个页面行情、盘口、成交、私有订单/持仓刷新链路中的订阅遗漏、topic 不一致或事件处理遗漏。
- 尽量复用现有 `stompService`、store 和 adapter，不引入新的实时通信栈。

## 范围

- PC 前端：
  - `pc/src/api/stomp.ts`
  - `pc/src/stores/market.ts`
  - `pc/src/stores/contract.ts`
  - `pc/src/stores/second.ts`
  - `pc/src/views/Trade.vue`
  - `pc/src/views/Contract.vue`
  - `pc/src/views/SecondOptions.vue`
  - 相关测试
- 后端仅在确认 topic 或事件 payload 与前端契约不一致时修改。

## 非目标

- 不重做下单撮合逻辑。
- 不改变行情 provider 接入策略。
- 不新增第三方 WebSocket 客户端。

## 验收标准

- 现货页订阅现货专用行情 topic，盘口/最新成交/顶部 ticker 能从推送更新。
- 杠杆页订阅杠杆专用行情 topic，盘口/最新成交/顶部 ticker 能从推送更新。
- 秒合约页订阅秒合约专用行情 topic，交易对价格、涨跌、K线/行情摘要和持仓刷新能从推送更新。
- 私有 `/ws/private` 事件仍能触发对应业务订单、持仓、余额刷新。
- 断线重连后能恢复当前页面的有效订阅。
- 有针对性测试覆盖 topic 构造、订阅分离和事件分发。
