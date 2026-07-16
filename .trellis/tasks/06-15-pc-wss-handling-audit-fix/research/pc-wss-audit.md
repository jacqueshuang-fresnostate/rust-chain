# PC WSS Audit Notes

## Inspected Files

- `pc/src/api/stomp.ts`
- `pc/src/api/socket.ts`
- `pc/src/views/Home.vue`
- `pc/src/views/Trade.vue`
- `pc/src/views/Contract.vue`
- `pc/src/views/SecondOptions.vue`
- `pc/src/views/BinaryOptions.vue`
- `pc/src/components/chart/TVChart.vue`
- `pc/src/components/trade/MarketTrades.vue`
- `pc/src/components/layout/MainLayout.vue`
- `pc/src/api/market.ts`
- `pc/src/api/second.ts`
- `pc/src/api/contract.ts`
- `src/modules/events/routes.rs`
- `src/modules/events/mod.rs`
- `tests/events_ws.rs`

## Backend Public WS Contract

- Public multi endpoint: `/ws/public`.
- Subscribe command: `{"op":"subscribe","channel":"ticker|depth|trade|kline","symbol":"BTCUSDT","interval":"1m"}`.
- Public namespaces are market feed namespaces only: `ticker`, `depth`, `trade`, `kline`.
- Backend broadcasts raw payload text, not a stable wrapper envelope.
- Kline topic is normalized as `BTCUSDT_1m`; other topics normalize to compact symbols like `BTCUSDT`.

## Findings

- `Home.vue` calls `stompService.disconnect()` on unmount. Since `stompService` is a shared singleton used by layouts and trading pages, navigating away from Home can close the shared WS and clear subscriptions owned by other mounted components.
- `stompService.connect(endpoint)` and `subscribe(endpoint, ...)` silently ignore endpoints other than `market`, but `TVChart` and `MarketTrades` expose `module: 'market' | 'second' | 'swap'`. This creates false support for second/swap WSS.
- `SecondOptions.vue` currently passes `module="market"` to `TVChart`, while second APIs reuse market history/trades. The product-level seconds tickers are derived from market tickers, so frontend should treat real-time price feed as market feed and filter by configured seconds products.
- `Contract.vue` uses `market:depth` and `market:ticker` to update margin UI. This matches backend feed availability, but the code should be explicit that margin uses shared market data rather than a separate contract WS namespace.
- `TVChart.vue` stores subscription keys using the string symbol during subscribe, but unsubscribe builds the key from the unsubscribe argument directly. If KLineChartPro passes a symbol object on unsubscribe, stale kline subscriptions may remain.
- `api/socket.ts` contains an unused generic `WebSocketClient` singleton pointed at Binance (`wss://stream.binance.com:9443/ws`). No PC code imports it. It is misleading and risks future accidental usage.
- `stompService` currently marks subscriptions as not subscribed on close but does not reconnect. Any transient close leaves PC real-time data stale until another component calls connect.

## Recommended MVP

- Keep backend contract unchanged.
- Split PC frontend WSS into business clients: `spot`, `margin`, `seconds`.
- Let the three business clients currently connect to the same backend `/ws/public` path, while keeping independent sockets, subscription maps, reconnect state, and future extension points.
- Add explicit topic/module normalization in the PC WS layer so seconds/margin consumers resolve to backend-supported market topics through their own business client instead of receiving no-op subscriptions.
- Replace page-level global `disconnect()` calls with subscription-level cleanup or business-scoped disconnect.
- Fix `TVChart` unsubscribe key normalization.
- Remove or neutralize the unused Binance example socket.
- Add tests for business connection isolation, module alias behavior, reconnect/resubscribe, and kline unsubscribe.
