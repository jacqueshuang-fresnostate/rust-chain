# 修复手机端现货订单簿与最新成交实时刷新

## Goal

修复手机端现货行情详情页仅在首次进入时通过 REST 获取订单簿和最新成交、后续数据不更新的问题。页面应使用后端公共行情 WebSocket 的 `depth` 与 `trade` 频道持续接收实时数据，同时保留 REST 首屏兜底。

## Requirements

- 保留现有 K 线、行情摘要和 REST 首次加载逻辑。
- 现货行情详情页进入后订阅当前交易对的公共 `depth` 与 `trade` WebSocket 频道。
- `depth` 推送按后端完整快照替换买卖盘，并保持买盘价格降序、卖盘价格升序及最多 12 档。
- `trade` 推送实时插入最新成交，按成交 ID 去重并最多保留 16 条。
- 切换交易对时停止旧订阅并只处理当前交易对数据。
- 页面卸载后关闭详情页连接、心跳与重连定时器。
- 网络断开且页面仍在使用时，以有上限的指数退避重连并重新订阅。
- WebSocket 数据异常时忽略无效帧，REST 数据继续作为可用兜底。

## Acceptance Criteria

- [x] 首次进入详情页仍可看到 REST 返回的订单簿与最近成交。
- [x] 收到 `depth` 推送后，页面订单簿无需刷新页面即可更新。
- [x] 收到 `trade` 推送后，最新成交列表头部立即出现该成交。
- [x] 重复成交不会重复展示，列表长度不超过 16。
- [x] 交易对切换、页面卸载后不存在旧连接继续写入页面状态。
- [x] 连接异常后自动重连并重新发送 `depth`、`trade` 订阅。
- [x] WebSocket 协议测试、手机端类型检查和完整自动化测试通过。
- [x] PWA 与 Tauri 构建通过，最新 Android APK 覆盖安装到已连接真机。

## Definition of Done

- 协议解析、连接生命周期和页面状态合并均有自动化回归覆盖。
- `npm --prefix mobile run type-check` 与 `npm --prefix mobile test` 通过。
- `npm --prefix mobile run build:pwa` 与 `npm --prefix mobile run build:tauri` 通过。
- Android Debug APK 构建、安装、冷启动完成。
- 进度记录和手机端后端集成规范同步更新。

## Technical Approach

1. 扩展现有 `marketSocketProtocol.ts`，统一生成 `ticker`、`depth`、`trade` 订阅帧，并把后端无显式 `type` 的深度/成交 JSON 识别为强类型事件。
2. 新建仅服务行情详情页的单交易对 WebSocket 生命周期模块，避免改动列表页既有的多交易对 ticker 连接；连接打开后订阅 `depth` 与 `trade`，维护心跳、指数退避与幂等关闭。
3. 抽取并复用 REST 与 WebSocket 共用的深度/成交适配函数，确保字段、时间戳、排序与过滤规则一致。
4. 在 `MarketDetailView.vue` 中将推送快照合并到现有 refs；切换交易对或卸载时清理旧订阅。

## Decision (ADR-lite)

**Context**: 现有全局行情连接为列表页设计，只维护 ticker symbol 集合和 ticker listener。直接加入深度及成交会引入多频道引用计数和监听器生命周期耦合。

**Decision**: 复用公共协议解析与 URL 配置，但为详情页建立单独、按页面生命周期管理的 WebSocket。REST 用于首屏和断线兜底，WebSocket 用于增量实时更新。

**Consequences**: 行情详情页打开期间会多一个 WebSocket，但连接职责清晰、不会影响首页/行情列表的 ticker 实时更新，切换与卸载清理也更容易验证。

## Out of Scope

- 不修改 PC 端或管理端。
- 不改变后端行情生产、数据库或公共 WebSocket 路由。
- 不改造现货下单页订单簿。
- 不新增离线伪造行情或演示数据。

## Technical Notes

- 后端 `/api/v1/ws/public` 接受 `{"op":"subscribe","channel":"depth|trade","symbol":"BTCUSDT"}`。
- 深度事件字段为 `symbol`、`bids`、`asks`、`observed_at`；每档包含 `price`、`quantity`。
- 成交事件字段为 `symbol`、`trade_id`、`side`、`price`、`quantity`、`traded_at`。
- 相关规范：`.trellis/spec/mobile/index.md`、`.trellis/spec/mobile/backend-integration.md`、`.trellis/spec/guides/cross-layer-thinking-guide.md`。
