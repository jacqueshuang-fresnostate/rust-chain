# Mobile Backend Integration Contract

## 1. Scope / Trigger

Apply this contract when changing mobile runtime backend configuration, Vite
proxying, Axios authentication behavior, public market WebSockets, or mobile
adapters for Rust user APIs.

The same rules apply to browser development, production PWA deployments, and
Tauri builds. A build mode must not silently select an origin that points at
the end user's device.

## 2. Signatures

Environment variables:

```text
VITE_BACKEND_API_DOMAIN
VITE_BACKEND_API_PREFIX=/api/v1
VITE_BACKEND_DEV_PROXY_TARGET=https://hipoex.cllbmz.kdns.fr
```

Product policy:

```ts
PRODUCT_BACKEND_ORIGIN = 'https://hipoex.cllbmz.kdns.fr'
resolveProductBackendOrigin(value): string
```

Runtime URL functions:

```ts
resolveBackendRuntimeConfig(input): BackendRuntimeConfig
resolveBackendApiUrl(config, path): string
resolveBackendHealthUrl(config): string
resolveBackendWebSocketUrl(config, path, pageOrigin?): string
```

Public market WebSocket:

```text
GET /api/v1/ws/public
subscribe -> {"op":"subscribe","channel":"ticker|depth|trade","symbol":"BTCUSDT"}
kline subscribe -> {"op":"subscribe","channel":"kline","symbol":"BTCUSDT","interval":"1m|5m|15m|1h|1d"}
heartbeat -> text "ping" / text "pong"
depth -> {"symbol":"BTCUSDT","bids":[{"price":"...","quantity":"..."}],"asks":[...],"observed_at":<unix-ms>,"provider":"..."}
trade -> {"symbol":"BTCUSDT","trade_id":"...","side":"buy|sell","price":"...","quantity":"...","traded_at":<unix-ms>,"provider":"..."}
kline -> {"symbol":"BTCUSDT","interval":"1m","open_time":<unix-ms>,"open":"...","high":"...","low":"...","close":"...","volume":"...","observed_at":<unix-ms>,"provider":"..."}
```

Depth, trade, and K-line broadcasts are direct payloads without a `type`
discriminator. Live K-line timestamps are JSON numbers in Unix milliseconds;
OHLCV values are decimal strings, `volume` may be zero, and `provider` is a
non-empty string.
The REST compatibility shapes remain `bids/asks[].amount` for depth and
`trades[].id/direction/amount/time` for recent trades.

## 3. Runtime URL and Proxy Contracts

- Browser development always calls the Vite origin. `VITE_BACKEND_API_DOMAIN`
  does not bypass the development proxy.
- Vite proxies the normalized API prefix with `ws: true` and proxies
  `/health` to `VITE_BACKEND_DEV_PROXY_TARGET`. A missing or whitespace-only
  proxy target uses `PRODUCT_BACKEND_ORIGIN`; a non-empty value wins.
- The mobile product adapter passes a missing or whitespace-only
  `VITE_BACKEND_API_DOMAIN` through `resolveProductBackendOrigin` before
  calling the generic runtime resolver. Production PWA and Tauri builds
  therefore default to `https://hipoex.cllbmz.kdns.fr/api/v1` for HTTP and
  `wss://hipoex.cllbmz.kdns.fr/api/v1/ws/public` for public market data.
- A non-empty `VITE_BACKEND_API_DOMAIN` wins over the product default and must
  satisfy the production-origin validation rules.
- An explicit production backend domain must be an absolute HTTPS origin
  without credentials, path, query, or fragment.
- Production configuration rejects `localhost`, IPv4 loopback, IPv6 loopback,
  and `0.0.0.0`.
- The generic `resolveBackendRuntimeConfig` contract remains reusable and
  product-neutral: no domain means same-origin for browser production and a
  diagnostic `BackendConfigurationError` for native production. Product
  defaults belong to the adapter, not this generic fallback.
- HTTP, health, and WebSocket URLs are derived from one validated runtime
  configuration. WebSocket schemes map `https -> wss` and `http -> ws`.
- `/health` remains an independently derived diagnostic URL. Mobile startup,
  routing, and business-page availability must not await or gate on it.

## 4. Authentication and Refresh Contracts

- Login, registration, login 2FA setup/confirm/reset, password reset, and
  refresh requests do not send a stored Bearer token.
- A 401 from those authentication bootstrap routes is returned directly and
  never starts a refresh or clears a newer session.
- Protected requests send `Authorization: Bearer <access_token>`.
- Concurrent protected 401 responses share one refresh operation. Each
  original request is replayed at most once with the refreshed user token.
- Refresh uses the public `/auth/refresh` route without the intercepted client.
  The response must contain non-empty access and refresh tokens with
  `scope=user`.
- If refresh is missing, invalid, or fails, persisted tokens are cleared and
  the application emits the session-expired transition.

## 5. WebSocket and Adapter Contracts

- Same-origin WebSockets use the nested Rust alias
  `/api/v1/ws/public`, so the development API proxy handles the upgrade.
- One mobile connection may subscribe to multiple normalized symbols.
  Reconnect resubscribes all active symbols and uses bounded exponential
  backoff.
- The client accepts subscription confirmations, direct ticker payloads, and
  text/JSON pong frames. Unknown or backend error frames must not be treated as
  ticker updates.
- The market-detail page owns a separate single-symbol public connection. It
  subscribes to `depth`, `trade`, and the selected `kline` interval alongside
  the initial REST requests, closes the old connection before a symbol or
  interval switch, and clears heartbeat, reconnect, and pending render-frame
  work on stop.
- `MARKET_KLINE_INTERVALS` is the single mobile market-detail interval source
  and must match the backend domain exactly: `1m`, `5m`, `15m`, `1h`, `1d`.
  Do not expose `4h` on this surface or accept arbitrary interval suffixes.
- REST depth and recent trades remain first-screen fallbacks. A settled REST
  response must not replace a depth snapshot already received from WebSocket;
  recent REST history may only append behind already rendered live trades and
  must deduplicate by trade id.
- REST K-lines and live K-lines share one adapter and merge function. REST may
  accept the compatibility number/string and seconds/milliseconds shapes, but
  the direct WebSocket parser validates the deployed payload strictly. The
  live list is the primary argument during reconciliation, so a matching
  `open_time` from WebSocket always wins over a later REST row.
- The detail session is identified by normalized symbol, selected interval,
  page request version, and monotonically increasing generation. Replacing a
  session invalidates the previous stream, pending REST request token, and any
  cancelled render-frame callback, including an `A -> B -> A` interval switch.
- A live K-line with the current `open_time` replaces the forming candle; a
  newer `open_time` appends a candle. Normalize, deduplicate, sort ascending,
  and retain the newest 160 points. Coalesce high-frequency updates so only
  the latest valid pending K-line is committed per animation frame.
- Each direct depth broadcast is a complete snapshot. Normalize numeric
  strings, reject the whole malformed frame, sort bids descending and asks
  ascending, retain at most 12 levels per side, and coalesce high-frequency
  snapshots so only the latest pending snapshot is committed per animation
  frame.
- Live trades are validated, prepended in arrival order, deduplicated by id,
  and capped at 16. A replayed id must not reorder or replace an already
  rendered trade.

### Market-detail Good / Base / Bad Cases

- **Good**: REST and WebSocket start together; a live depth snapshot wins over
  a later REST response, REST trade history appends behind live trades, and a
  live forming candle wins over the same `open_time` in later REST history.
- **Base**: WebSocket is temporarily unavailable; REST depth/trades remain
  visible and REST K-lines render the chart while the connection reconnects
  with bounded exponential backoff and resubscribes all three channels.
- **Bad**: a frame has an invalid side, zero/boolean numeric value, incomplete
  depth side, unsupported interval, non-millisecond live K-line timestamp,
  malformed OHLC relationship, missing provider, or mismatched symbol; ignore
  the whole frame and preserve the last valid state.

### Market-detail Wrong vs Correct

```ts
// Wrong: keep one interval-agnostic stream and let late REST overwrite WS.
await Promise.all([fetchKlines(symbol), fetchOrderBook(symbol)])
startDetailStream(symbol)

// Correct: replace the session for each supported interval, connect alongside
// REST, and reconcile with live points as the primary source.
const context = detailSession.replace(symbol, interval, requestVersion)
const request = detailSession.beginKlineRequest(context)
const initial = await Promise.allSettled([fetchKlines(symbol), fetchOrderBook(symbol)])
const points = detailSession.resolveKlineRequest(request, restKlines(initial))
```

- New-coin purchase uses the backend-configured pair id, locks the displayed
  payment asset to that pair's quote asset, and computes percentage quantity
  as `quote_available * percentage / execution_price`.
- News details render supported structured text marks, safe HTTP(S)/relative
  links, and image blocks through Vue bindings; never inject backend HTML.
- Prediction history displays backend `order_no`. Margin position labels join
  `product_id` or `pair_id` to stable product/market metadata instead of
  treating a numeric pair id as a symbol.
- Guest spot trading must not request authenticated margin products.

## 6. Error Matrix

| Condition | Required behavior |
| --- | --- |
| Product backend environment is empty | Use `PRODUCT_BACKEND_ORIGIN` for PWA and Tauri |
| Generic native production resolver receives no domain, or the product override is invalid | Show backend-not-configured diagnostics; make no loopback request |
| Request times out | Show the localized timeout state |
| Browser reports offline | Show the localized device-offline state |
| Network fails without an HTTP response | Show the localized backend/network state |
| Backend returns an error body | Preserve the backend message when present |
| WebSocket closes while listeners remain | Reconnect with bounded backoff and resubscribe |
| WebSocket has no listeners | Cancel timers, clear subscriptions, and close |
| Detail interval is unsupported | Do not create a socket or send an empty K-line subscription |
| Direct K-line timestamp is seconds/string, OHLCV has a non-string, or provider is empty | Reject the whole live frame; keep the last valid chart |
| REST K-line settles after one or more live candles | Merge REST behind live points; never replace a matching live `open_time` |
| Symbol/interval changes or repeats through an ABA sequence | Invalidate the old context, REST token, and pending animation-frame callback |

## 7. Tests Required

- Unit tests for product defaults and non-empty overrides in PWA/Tauri,
  prefix/origin normalization, generic PWA same-origin URLs, generic Tauri
  configuration errors, HTTPS/loopback rejection, health URLs, and WS scheme
  conversion.
- Source/config tests for the dedicated development proxy target, API
  `ws: true`, `/health` proxy, and absence of a startup health gate.
- Request-layer tests for bootstrap Bearer removal, bootstrap 401 exclusion,
  singleton refresh, one replay, and failed-refresh session cleanup.
- WebSocket protocol tests for ticker/depth/trade/K-line subscribe frames,
  supported interval normalization, confirmation, verified direct payload
  shapes, heartbeat, strict live K-line rejection, and malformed frames.
- Market-detail lifecycle tests for REST/WebSocket races, live K-line upsert,
  new-candle append, 160-point retention, latest-only depth/K-line frame
  coalescing, symbol/interval filtering, ABA interval isolation, reconnect
  resubscription and backoff, duplicate close/error idempotency, cancelled RAF
  callbacks, and stop-before-open cleanup. Race tests must execute fake sockets
  and delayed promises rather than assert source text alone.
- Adapter tests for new-coin quote quantity, safe news rich text, prediction
  order number, and stable margin pair labels.
- Run `npm run type-check`, `npm test`, `npm run build:pwa`, and
  `npm run build:tauri` after changing this contract's runtime paths.
