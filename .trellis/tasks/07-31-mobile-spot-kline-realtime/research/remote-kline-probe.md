# Remote K-line WebSocket Probe

Probe time: 2026-07-31 (Asia/Hong_Kong)

Endpoint:

```text
wss://hipoex.cllbmz.kdns.fr/api/v1/ws/public
```

Subscription:

```json
{"op":"subscribe","channel":"kline","symbol":"BTCUSDT","interval":"1m"}
```

Observed confirmation:

```json
{"type":"subscribed","channel":"public:kline:BTCUSDT_1m"}
```

Observed direct payload:

```json
{"symbol":"BTCUSDT","interval":"1m","open_time":1785501420000,"open":"63718.61","high":"63718.62","low":"63703.04","close":"63708.85","volume":"0.800436","observed_at":1785501445320,"provider":"bitget"}
```

The first valid K-line arrived about 3.7 seconds after opening the connection. This confirms that the deployed backend already publishes the exact direct-payload contract described in the PRD; the missing behavior is client-side subscription and state merging.

A second bounded sample received five frames for the same `open_time` within
about eleven seconds. All five `(close, volume)` states were unique, proving
that the deployed feed continuously republishes the currently forming candle
rather than only emitting a completed-candle event.
