# 修复手机端现货 K 线实时刷新

## Goal

修复手机端现货行情详情页 K 线仅依赖 REST 首次加载、进入页面后不会随后端实时行情更新的问题。页面应复用现有详情页公共行情 WebSocket，在订阅订单簿与最新成交的同时订阅当前交易对和当前周期的 `kline` 频道，并持续更新正在形成的蜡烛。

## What I Already Know

- 后端公共 WebSocket 路由为 `/api/v1/ws/public`。
- K 线订阅命令为 `{"op":"subscribe","channel":"kline","symbol":"BTCUSDT","interval":"15m"}`。
- 后端直接推送无 `type` 包装的 K 线对象，字段为 `symbol`、`interval`、`open_time`、`open`、`high`、`low`、`close`、`volume`、`observed_at`、`provider`。
- `open_time` 与 `observed_at` 为 Unix 毫秒，价格和成交量为十进制字符串。
- 现有 `MarketDetailView.vue` 只通过 REST 获取 K 线；专用详情 WebSocket 已具备心跳、重连、交易对隔离及 `depth`/`trade` 高频渲染合并能力。
- `MobileMarketChart.vue` 已深度监听 `points` 并在数据变化时重绘，因此缺口位于协议、连接和页面状态合并层。

## Requirements

- 保留现有 REST K 线首屏兜底，不改变后端、PC 端或管理端。
- 详情页 WebSocket 打开及每次重连后订阅当前交易对、当前周期的 `kline` 频道。
- 将后端 K 线帧严格验证并归一化为现有 `KlinePoint`，统一处理毫秒时间戳。
- 同一 `open_time` 的推送替换正在形成的蜡烛；新的 `open_time` 追加为新蜡烛；历史按时间排序、去重并保持既有上限。
- 高频 K 线推送只提交每个渲染帧内的最后一条有效更新，避免图表抖动和无效重绘。
- REST 与 WebSocket 并发启动；晚到的 REST 响应只能补充历史，不能覆盖已经收到的实时蜡烛。
- 切换 K 线周期时关闭旧连接并立即以新周期重新订阅；旧周期或旧连接的迟到帧不得写入当前图表。
- 交易对切换、组件卸载和连接异常时继续遵循现有清理、重连、心跳及 `depth`/`trade` 行为。
- 错误交易对、错误周期及畸形 K 线帧应被忽略，并保留最后一份有效图表数据。

## Acceptance Criteria

- [x] 首次进入详情页仍可通过 REST 显示历史 K 线。
- [x] 当前蜡烛收到同一 `open_time` 推送后无需刷新页面即可更新 OHLCV。
- [x] 新周期蜡烛到达后会按时间追加，且无重复时间点。
- [x] REST 晚于 WebSocket 返回时，不会把实时蜡烛覆盖为旧数据。
- [x] 切换 `1m`、`5m`、`15m` 等周期后只接收新周期帧，订单簿和最新成交仍持续刷新。
- [x] 网络重连后重新发送 `depth`、`trade` 和带 `interval` 的 `kline` 订阅。
- [x] 畸形、错误交易对或错误周期帧不改变当前图表。
- [x] 协议、连接生命周期、合并竞态和周期切换均有自动化回归覆盖。
- [x] 手机端类型检查、完整测试、PWA/Tauri 构建和 Android APK 构建通过。
- [x] 最新 APK 覆盖安装到已连接真机，并验证目标交易对图表可接收实时 K 线。

## Definition of Done

- 协议解析、连接生命周期、REST/WS 合并和页面周期切换均由测试覆盖。
- `npm --prefix mobile run type-check` 与 `npm --prefix mobile test` 通过。
- `npm --prefix mobile run build:pwa` 与 `npm --prefix mobile run build:tauri` 通过。
- Android Debug APK 构建、安装、冷启动和真机实时数据验证完成。
- 手机端后端集成规范与 `docs/superpowers/PROGRESS.md` 同步更新。
- 任务改动已提交且 Trellis 任务已归档。

## Technical Approach

1. 扩展 `marketSocketProtocol.ts` 的频道类型、订阅帧、K 线识别/验证和历史合并工具，作为 REST 与 WebSocket 共享的唯一适配边界。
2. 扩展 `marketDetailStream.ts`，在连接和重连时携带 `interval` 订阅 K 线，并像深度快照一样按动画帧合并待提交更新。
3. 在 `MarketDetailView.vue` 中以交易对、页面版本、连接版本和周期共同守卫回调；将实时蜡烛 upsert 到 `points`，并让晚到 REST 仅补历史。
4. 扩展协议及详情流测试，覆盖直接载荷、毫秒归一化、同蜡烛替换、新蜡烛追加、错误周期过滤、重连订阅、渲染帧合并和 REST/WS 竞态。

## Decision (ADR-lite)

**Context**: K 线与订单簿、成交来自同一公共 WebSocket，但订阅键额外包含周期；切换周期时若复用旧回调或让 REST 直接赋值，会出现旧周期串写和实时蜡烛回退。

**Decision**: 扩展现有详情页单交易对连接，使其同时拥有当前周期；周期切换时原子替换整条详情连接，并在共享协议层完成 K 线解析与 upsert。REST 与实时数据通过同一合并函数组合，实时同时间点始终优先。

**Consequences**: 周期切换会重建一个已有连接，但生命周期边界明确，`depth`/`trade` 会在新连接中自动恢复；无需扩大全局 ticker 连接的职责，也无需改变后端协议。

## Out of Scope

- 不修改后端行情生产、缓存、数据库和 WebSocket 路由。
- 不修改 PC 端、管理端或现货下单页图表。
- 不新增前端定时伪造蜡烛、轮询或离线演示行情。
- 不重构图表视觉、交互手势或页面布局。

## Technical Notes

- 数据流：行情提供方 → Rust `market_kline` 事件 → `public:kline:<SYMBOL>_<INTERVAL>` → 手机协议解析 → `KlinePoint` upsert → `MobileMarketChart` watcher。
- 相关规范：`.trellis/spec/mobile/index.md`、`.trellis/spec/mobile/backend-integration.md`、`.trellis/spec/guides/cross-layer-thinking-guide.md`、`.trellis/spec/guides/code-reuse-thinking-guide.md`。
- 重点文件：`mobile/src/api/marketSocketProtocol.ts`、`mobile/src/api/marketDetailStream.ts`、`mobile/src/api/market.ts`、`mobile/src/views/MarketDetailView.vue`、`mobile/tests/market-socket.test.ts`、`mobile/tests/market-detail-stream.test.ts`。
