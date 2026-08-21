# 修复行情订阅长时间运行后停止推送

## Goal

修复外部交易所行情 WebSocket 与手机端公共行情 WebSocket 在连接仍显示为打开、但已长时间收不到任何入站帧时不会自愈的问题，确保行情长期运行、断流后自动重连并完整恢复订阅。

## What I already know

- 后端每个供应商都有无限重连循环，但 `run_provider_once` 只有在读取到关闭、流结束或错误后才返回；半开连接会永久阻塞在 `reader.next().await`。
- Bitget 官方要求客户端约每 30 秒发送文本 `ping`，未收到文本 `pong` 时主动重连；现有后端没有主动发送 Bitget 心跳，也不能解析纯文本 `pong`。
- HTX 会主动发送 JSON `ping`，现有后端已经回 JSON `pong`；Coinbase 已订阅 `heartbeats` 频道。
- 手机端 ticker 与详情流虽然每 25 秒发送一次 `ping`，但只在 `close`、`error` 或同步发送失败时重连，没有入站静默超时，因此浏览器中的半开 socket 也会永久停留在 `OPEN`。
- 后端进程内广播订阅在 receiver lagged 时会跳过丢失帧并继续接收，不是本次永久断流的主要根因。

## Requirements

### 后端上游行情

- Bitget 连接按官方协议周期性发送纯文本 `ping`，并把纯文本 `pong` 视为有效控制帧而不是 JSON 解析错误。
- 所有供应商连接维护入站静默超时；任何行情、确认、ping 或 pong 帧都应刷新连接活性。
- 超过静默上限后必须返回明确错误，让既有 REST 兜底与指数退避重连重新接管。
- 心跳、订阅和协议回复写入必须有有界超时，避免写侧半开导致连接任务永久卡住。
- 保留既有供应商隔离、REST 兜底、已落库/已广播数据不回滚与重连退避语义。

### 手机端下游行情

- ticker 共享流与市场详情流都维护入站静默看门狗。
- socket 打开后启动看门狗，任何入站帧（包括文本/JSON `pong`、订阅确认和行情帧）都刷新看门狗。
- 静默超时后关闭当前 socket、按既有有界退避重连，并重新发送当前有效的全部订阅。
- 释放最后一个 ticker lease 或停止详情 session 后必须清理心跳、静默看门狗、重连和待提交动画帧；旧 socket 的延迟事件不能影响新连接。
- 不改变价格 authority、REST/WS 合并、K 线渲染和交易业务契约。

## Acceptance Criteria

- [x] 后端测试证明 Bitget 心跳报文为纯文本 `ping`，纯文本 `pong` 不触发解析失败。
- [x] 后端测试证明持续静默会触发超时事件，而正常入站活动会延后静默截止时间。
- [x] ticker 流测试证明 socket 保持 `OPEN` 但没有任何入站帧时会被关闭、重连并恢复当前 lease 的订阅。
- [x] 详情流测试证明同一静默场景会重连并恢复 depth、trade、kline 订阅。
- [x] 手机端测试证明收到 pong/控制帧会刷新看门狗，stop/最后 lease 会清理所有定时器。
- [x] 聚焦 Rust 与 mobile 测试、Rust fmt/check/clippy、mobile type-check 与完整测试通过。
- [x] 实时 WebSocket 与手机后端集成规范记录主动心跳、静默检测和重订阅合同。

## Quality Review

- Rust 全量 280 个 lib 测试、行情 worker 32 个测试、后端架构 11 个测试全部通过。
- `cargo check --all-targets`、`cargo clippy --all-targets --all-features -- -D warnings` 与 `cargo fmt --check` 通过。
- Mobile 全量 466 个测试、type-check、PWA 构建、Tauri 构建通过；项目没有单独 lint script。
- Mobile 静默测试验证 OPEN 半开连接、pong 刷新、旧 watchdog generation、完整重订阅与定时器清理。
- 后端测试额外锁定高频行情已经 ready 时到期心跳优先，防止主动心跳被读分支饿死。

## Out of Scope

- 更换交易所行情供应商、修改交易对配置或行情价格计算规则。
- 修改订单簿/K 线/逐笔成交的业务展示与图表视觉。
- 为 WebSocket 广播增加持久化重放；断连窗口仍由现有 REST 数据源收敛。
- 修改管理员行情配置页面。

## Technical Notes

- 后端入口：`src/workers/market_feed.rs`。
- 后端聚焦测试：`tests/unit_src/src_workers_market_feed_tests.rs`。
- 手机 ticker：`mobile/src/api/marketTickerStream.ts`、`mobile/tests/market-ticker-stream.test.ts`。
- 手机详情流：`mobile/src/api/marketDetailStream.ts`、`mobile/tests/market-detail-stream.test.ts`。
- 研究记录：`research/market-stream-liveness.md`。
