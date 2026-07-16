# Coinbase Advanced Trade Market Feed Notes

Sources:

* https://docs.cdp.coinbase.com/coinbase-app/advanced-trade-apis/
* https://docs.cdp.coinbase.com/coinbase-app/advanced-trade-apis/websocket/websocket-overview
* https://docs.cdp.coinbase.com/coinbase-app/advanced-trade-apis/websocket/websocket-channels
* https://docs.cdp.coinbase.com/coinbase-app/advanced-trade-apis/rest-api

Findings:

* Coinbase Advanced Trade public market data uses the REST host `https://api.coinbase.com` with market endpoints under `/api/v3/brokerage/market/...`.
* The public WebSocket endpoint is `wss://advanced-trade-ws.coinbase.com`.
* Public WebSocket subscriptions use `type: "subscribe"`, `product_ids`, and a `channel`.
* Useful market channels for our existing ingestion model are `ticker`, `level2` inbound as `l2_data`, `market_trades`, `candles`, and `heartbeats`.
* Coinbase product IDs use dash-separated symbols such as `BTC-USD` / `BTC-USDT`; the app stores compact symbols such as `BTCUSDT`.
* Coinbase WebSocket candles are 5-minute candles, so live candle frames map to internal `5m`; REST candle fallback can request supported granularities with query parameter `granularity`.

Repo mapping:

* Keep `coinbase` as a `MarketFeedProvider` code and convert product IDs inside provider/adapter helpers.
* Use REST product endpoint for ticker fallback and REST candles endpoint for kline fallback.
* Ignore heartbeat messages and surface Coinbase error frames as validation errors.
