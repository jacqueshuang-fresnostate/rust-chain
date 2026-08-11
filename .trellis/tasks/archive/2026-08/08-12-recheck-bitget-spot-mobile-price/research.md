# Bitget 现货与手机端行情复核

## 对比边界

- Bitget 页面：`https://www.bitget.com/zh-CN/spot/BTCUSDT`
- Bitget REST：`/api/v2/spot/market/tickers` 、`/api/v2/spot/market/orderbook` 与 `/api/v2/spot/market/fills`
- HIPPO 页面：`/#/` 与 `/#/trade/BTC_USDT`，未使用 `mode=contract`
- HIPPO REST：`/api/v1/markets/BTCUSDT/{ticker,depth,trades}`

## 实测证据

2026-08-12 04:51:41 HKT 由 Ego 在同一任务空间读取：

| 字段 | HIPPO | Bitget 现货 |
| --- | ---: | ---: |
| 最新价 | 63698.46 | 63698.46 |
| 24h 高 | 64493.51 | 64493.51 |
| 24h 低 | 63235.29 | 63235.29 |
| 24h 成交量 | 1367.299671 BTC | 1367.299671 BTC |
| 买一 | 63698.46 | 63698.46 |
| 卖一 | 63698.47 | 63698.47 |
| 涨跌幅 | -0.71400% | -0.00714（即 -0.714%） |
| ticker 时间戳 | 1786481498284 | 1786481498600 |

数量在两次 HTTP 请求的毫秒间隔内会变动，价格档位和 ticker 数值一致。

2026-08-12 04:50:35 HKT 实页验收：

- 手机首页 DOM：`BTC/USDT 63,699.06 USDT -0.70%`
- Bitget 现货 REST：`lastPr=63699.06`、`change24h=-0.00703`
- 首页价格精确一致，涨跌幅按两位小数显示为 `-0.70%`。

## 数据口径

- 后端 Bitget 订阅明确使用 `instType=SPOT`，并订阅 ticker、books50、trade 和 candle 频道。
- ticker、订单簿和 K 线的实时帧来自 Bitget 现货 feed。
- `/markets/:symbol/trades` 的 REST 首屏回退读取 HIPPO 内部 `spot_trades`，当时返回空数组；页面连接后收到的 live trade 帧才是 Bitget 现货全市场成交。两者不得混为同一历史口径。
- 主价格权威源现统一为 ticker；订单簿、live trade 和 K 线仅更新各自组件。

## 根因与修复

1. 现货交易页之前优先使用内部历史成交或 K 线收盘价，会让限价输入框停在旧价，即使订单簿和 ticker 已刷新。
2. 首页 REST 刷新与 WebSocket 并发时，迟到的 REST 快照可能覆盖更新的实时帧。
3. 手机映射器没有消费后端已返回的 `price_change_percent_24h`，而是使用不适合反推开盘价的 `price_change_24h`，造成首页涨跌方向错误。
4. Vue 路由转场期间新旧页面会短暂共存；单一全局启停开关可能被离场页面关闭，使已进入的现货页失去 ticker 更新。

修复后，Home、Markets、Trade 和 Market Detail 共用 ticker 权威价；合并 REST/WS 时以 `observed_at` 新者优先；涨跌幅优先映射 `price_change_percent_24h`，并把零视为有效值；四个行情页面使用稳定 consumer lease，仅最后一个离场时关闭共享 ticker 流。
