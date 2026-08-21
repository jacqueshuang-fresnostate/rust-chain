# 行情长连接断流研究

## 链路地图

```text
Bitget / HTX / Coinbase WebSocket
  -> src/workers/market_feed.rs::run_provider_once
  -> MarketIngestionService（Redis / Mongo / outbox / broadcast hub）
  -> /api/v1/ws/public
  -> mobile marketTickerStream / marketDetailStream
  -> Home / Markets / Trade / Market Detail
```

## 本地代码诊断

### 1. 上游连接只处理显式断开

`src/workers/market_feed.rs::run_provider_once` 在发送订阅后直接等待
`reader.next().await`。只有服务端 close、流结束或读错误才会退出，因此 TCP、代理或 NAT
形成半开连接时，外层 `run_provider_reconnect_loop_with` 永远拿不回控制权，REST 兜底与
指数退避重连都不会执行。

### 2. Bitget 缺少协议要求的主动心跳

Bitget 官方 WebSocket 文档要求客户端约每 30 秒发送纯文本 `ping`，并期待纯文本
`pong`；没有 `pong` 应主动重连，服务端在约 2 分钟没有收到 `ping` 时会断开。
现有后端没有主动发送该心跳，而且 `market_feed_text_action` 会直接把纯文本 `pong`
交给 JSON 解析并返回校验错误。

官方资料：

- https://www.bitget.com/api-doc/classic/quickStart/websocket-intro

### 3. 其他供应商有服务端活性帧，但仍需静默上限

- HTX 官方说明服务端约每 5 秒发送 ping，客户端连续未响应会被断开；现有代码已支持
  JSON `ping` -> JSON `pong`。网络半开时仍需要客户端侧静默上限主动结束读循环。
- Coinbase Exchange/Advanced Trade 提供 heartbeat(s) 频道；现有 provider 订阅已经包含
  `heartbeats`，因此可把任何收到的 heartbeat 当作连接活性，但同样不能依赖 close/error
  才发现故障。

官方资料：

- https://www.htx.com/en-us/opend/
- https://huobiapi.github.io/docs/spot/v1/en/
- https://docs.cdp.coinbase.com/exchange/websocket-feed/channels
- https://docs.cdp.coinbase.com/coinbase-business/advanced-trade-apis/websocket/websocket-channels

### 4. 手机端有发送心跳但没有响应看门狗

`mobile/src/api/marketTickerStream.ts` 与
`mobile/src/api/marketDetailStream.ts` 都每 25 秒发送文本 `ping`，但没有记录最后入站时间。
浏览器 WebSocket 的 `readyState` 可能长期保持 `OPEN`，`send()` 也可能不立即抛错；只依赖
`close/error` 因而无法自愈。

### 5. 广播 lag 不是永久停止根因

`src/modules/events/service/websocket.rs` 在 broadcast receiver 返回 `Lagged` 时会继续循环，
不会永久结束订阅。广播本身仍是进程内、无重放的提示流，连接恢复后应由 REST 快照收敛
断连窗口。

## 方案决定

1. Bitget 每 25 秒主动发送文本 `ping`；所有入站帧都刷新静默截止时间。
2. 所有上游 provider 使用 75 秒静默上限。HTX 的服务端 ping、Coinbase heartbeats 与
   Bitget pong 都远短于该上限，可避免把清淡市场误判为失活。
3. WebSocket 写入使用 10 秒上限，静默或写超时以 `AppError::Internal` 结束本轮，让现有
   REST fallback + bounded exponential reconnect 接管。
4. 手机 ticker 与详情流保留 25 秒 ping，并增加默认 65 秒入站静默看门狗。任何消息先
   刷新看门狗再解析；即使是 pong/确认帧也证明传输链路仍活着。
5. 超时恢复复用既有 socket identity guard、退避和重订阅逻辑，不在业务页面复制连接状态。
6. 后端 `select!` 在心跳到期时优先处理心跳，再处理已到达行情，最后处理静默截止；这可避免
   高频 books/trade 数据让 Bitget 心跳永久饥饿，同时仍让截止点边界上的已到达帧先于超时。

## 风险与验证

- 定时器节流：PWA 退到后台时浏览器可能冻结定时器；恢复前台后过期看门狗会关闭旧连接并
  重订阅，优于继续使用未知状态的旧 socket。
- 清淡交易对：看门狗由协议 pong/heartbeat 刷新，不要求一定有成交或深度变化。
- 旧连接竞态：所有回调必须继续核对 `socket === next` 与 disconnect/session 状态。
- 资源泄漏：最后 lease、stop、close/error 和静默超时都必须同时清理 heartbeat 与 watchdog。
