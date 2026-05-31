# 02. 行情、K 线与存储设计

## 1. 行情数据流

1. Market Adapter 连接 Bitget / HTX REST 与 WebSocket。
2. 将外部 ticker、depth、trade、kline 标准化为平台内部 MarketEvent。
3. 最新 ticker、盘口、短周期 K 线写入 Redis。
4. 标准化行情事件发布到 RabbitMQ。
5. K 线聚合器、WebSocket 推送、交易模块订阅行情事件。
6. 历史 K 线、策略行情结果、可选 ticker / trade 归档写入 MongoDB。
7. MySQL 仅保存行情源、交易对、策略配置等低频业务配置。

## 2. 存储职责

- MySQL 只保存行情源、交易对、策略配置等低频业务配置。
- Redis 保存最新 ticker、最新盘口、近期 K 线和 WebSocket 推送缓存。
- MongoDB 保存历史 K 线、新币策略行情结果、可选 ticker / trade / depth 归档。
- RabbitMQ 负责行情事件分发，不作为长期存储。

## 3. MySQL 表

| 表 | 作用 | 关键字段 |
|---|---|---|
| market_sources | 行情源配置 | name, priority, enabled, rest_base_url, ws_url |
| trading_pairs | 交易对 | id, base_asset, quote_asset, price_precision, qty_precision, min_order_value, status, market_type |
| market_strategies | 策略配置 | id, pair_id, strategy_type, start_price, target_price, start_time, end_time, volatility, volume_min, volume_max, status |
| strategy_runs | 策略运行记录 | strategy_id, run_status, current_price, last_tick_at, last_generated_at, last_kline_open_time, recovery_status, error_message |
| strategy_versions | 策略版本记录 | strategy_id, version, effective_time, config_json, seed, created_by, created_at |
| strategy_events | 策略事件 | strategy_id, event_type, payload_json, created_at |

`market_type`：

- external：外部行情交易对，如 BTC/USDT。
- internal：平台内部交易对，如 NEW/USDT。
- strategy：平台策略行情交易对。

## 4. MongoDB 按交易对拆分

MongoDB 按交易对拆分 Collection，避免所有交易对混写到同一个大集合。集合命名统一使用标准化交易对名，例如 `BTCUSDT`、`ETHUSDT`、`NEWUSDT`。

| Collection 模式 | 作用 | 示例 |
|---|---|---|
| market_klines_{symbol} | 单交易对历史 K 线 | market_klines_BTCUSDT, market_klines_NEWUSDT |
| market_tickers_{symbol} | 单交易对 ticker 快照归档，可 TTL | market_tickers_BTCUSDT |
| market_trades_{symbol} | 单交易对成交流归档，可 TTL | market_trades_NEWUSDT |
| market_depth_snapshots_{symbol} | 单交易对盘口快照归档，可 TTL | market_depth_snapshots_BTCUSDT |
| strategy_market_events_{symbol} | 单交易对新币策略行情事件 | strategy_market_events_NEWUSDT |

文档中的 `market_klines`、`market_tickers`、`market_trades`、`market_depth_snapshots`、`strategy_market_events` 表示逻辑集合族，不表示所有交易对共用一个物理集合。

## 5. MongoDB 文档字段

| Collection 模式 | 关键字段 |
|---|---|
| market_klines_{symbol} | interval, open_time, close_time, open, high, low, close, volume, source |
| market_tickers_{symbol} | last_price, volume_24h, source, ts |
| market_trades_{symbol} | price, quantity, side, source, ts |
| market_depth_snapshots_{symbol} | bids, asks, source, ts |
| strategy_market_events_{symbol} | strategy_id, event_type, price, volume, ts, payload |

## 6. MongoDB 索引

- market_klines_{symbol}：unique(interval, open_time)
- market_tickers_{symbol}：index(ts)，可按 ts 设置 TTL
- market_trades_{symbol}：index(ts)，可按 ts 设置 TTL
- market_depth_snapshots_{symbol}：index(ts)，可按 ts 设置 TTL
- strategy_market_events_{symbol}：index(strategy_id, ts)，index(ts)

查询规则：

- 服务层根据交易对 symbol 路由到对应 Collection。
- 禁止 API 直接拼接任意 Collection 名，必须通过交易对白名单和命名规范生成。
- 新交易对上线时，由后台创建交易对配置，并初始化或延迟创建对应 MongoDB Collection 与索引。

## 7. Redis Key

- market:ticker:{pair}
- market:depth:{pair}
- market:kline:{pair}:{interval}
- user:session:{token_id}
- risk:limit:{user_id}

## 8. 策略行情数据流

| 模块 | 职责 |
|---|---|
| Admin Strategy Config | 管理目标价、涨跌方向、时间窗口、波动率、成交量 |
| Strategy Engine | 生成连续价格路径、模拟成交、K 线基础数据 |
| Market Event Bus | 将策略行情包装成统一 MarketEvent |
| Kline Aggregator | 聚合 1m、5m、15m、1h、1d K 线 |
| Risk Guard | 限制异常价格跳变、策略冲突、越界参数 |
| Audit Log | 记录策略创建、修改、启停 |

策略边界：

- 只能绑定 internal / strategy 交易对。
- 修改、启停必须写审计日志。
- 策略修改必须生成 strategy_versions，不能覆盖历史版本。
- 运行状态必须保存检查点，支持服务重启后补齐缺失 K 线。
- 参数必须经过 Risk Guard 校验。

## 9. 新币 K 线连续性与停机补偿

新币不能只依赖服务运行时的内存定时器生成 K 线，否则项目更新、服务重启或停机 30 分钟会导致 MongoDB 中缺少对应时间段的 K 线。根因是行情生成绑定了服务在线状态，而不是绑定策略配置、交易对时间轴和持久化检查点。

解决原则：策略行情必须按交易对时间轴确定性生成，服务重启后可从检查点补齐缺口。

| 机制 | 设计 |
|---|---|
| 持久化策略 | market_strategies 保存策略参数、时间窗口、随机种子、是否允许停机补偿 |
| 策略版本 | 每次修改策略生成新版本，记录 effective_time，补偿时按版本时间段生成 |
| 生成检查点 | strategy_runs 保存 last_generated_at、last_kline_open_time、current_price、recovery_status |
| 启动扫描 | 服务启动后扫描 active/running 策略，比较 MongoDB 最新 K 线时间与当前时间 |
| 缺口计算 | 按 symbol + interval 找到缺失 open_time 区间，例如 10:00 至 10:30 |
| 确定性补偿 | 使用 strategy_id、version、symbol、timestamp、seed 生成同一时间点的稳定价格路径 |
| 幂等写入 | 写入 market_klines_{symbol} 时使用 unique(interval, open_time) 和 upsert |
| 分布式锁 | 同一交易对同一策略补偿任务使用 Redis 锁，避免多实例重复生成 |
| 补偿状态 | 补偿期间策略状态为 catching_up，补齐后切回 live |
| 推送规则 | 历史补偿 K 线写 MongoDB，不按实时行情逐条推送；补齐完成后再推送最新行情 |

停机恢复流程：

1. 服务启动后加载所有运行中的新币策略。
2. 对每个交易对读取 MongoDB 中 market_klines_{symbol} 的最大 open_time。
3. 读取 strategy_runs.last_generated_at，取两者中更可信的时间作为补偿起点。
4. 计算补偿起点到当前时间之间缺失的 K 线周期。
5. 按策略版本、时间段、随机种子生成缺失 K 线。
6. 通过 upsert 写入 market_klines_{symbol}，更新 strategy_runs 检查点。
7. 补偿完成后恢复实时策略行情生成。

如果后台明确暂停策略，系统不补偿暂停期间 K 线；如果是服务维护、重启、异常宕机，系统按策略配置自动补偿。

## 10. 行情 API 与 WebSocket

用户端 REST API：

| 模块 | API | 说明 |
|---|---|---|
| 行情 | GET /api/v1/markets | 交易对列表 |
| 行情 | GET /api/v1/markets/{symbol}/ticker | 最新行情 |
| 行情 | GET /api/v1/markets/{symbol}/klines | K 线 |

用户端 WebSocket：

- WS /ws/public
- WS /ws/private

公共订阅：

- ticker.BTCUSDT
- depth.BTCUSDT
- trade.BTCUSDT
- kline.BTCUSDT.1m
- kline.BTCUSDT.5m

私有订阅：

- orders
- balances
- notifications

消息格式：

```json
{
  "topic": "ticker.BTCUSDT",
  "type": "snapshot",
  "ts": 1710000000000,
  "data": {}
}
```

## 11. RabbitMQ 事件

| Exchange | Routing Key | 说明 |
|---|---|---|
| market.events | ticker.BTCUSDT | 行情事件 |
| strategy.events | strategy.started | 策略状态 |
