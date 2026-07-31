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
heartbeat -> text "ping" / text "pong"
depth -> {"symbol":"BTCUSDT","bids":[{"price":"...","quantity":"..."}],"asks":[...],"observed_at":<unix-ms>,"provider":"..."}
trade -> {"symbol":"BTCUSDT","trade_id":"...","side":"buy|sell","price":"...","quantity":"...","traded_at":<unix-ms>,"provider":"..."}
```

Depth and trade broadcasts are direct payloads without a `type` discriminator.
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
  subscribes to `depth` and `trade` alongside the initial REST requests, closes
  the old connection before a symbol switch, and clears heartbeat, reconnect,
  and pending render-frame work on stop.
- REST depth and recent trades remain first-screen fallbacks. A settled REST
  response must not replace a depth snapshot already received from WebSocket;
  recent REST history may only append behind already rendered live trades and
  must deduplicate by trade id.
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
  a later REST response, and REST trade history appends behind live trades.
- **Base**: WebSocket is temporarily unavailable; REST depth/trades remain
  visible while the connection reconnects with bounded exponential backoff.
- **Bad**: a frame has an invalid side, zero/boolean numeric value, incomplete
  depth side, or mismatched symbol; ignore the whole frame and preserve the
  last valid state.

### Market-detail Wrong vs Correct

```ts
// Wrong: every interval change blanks live data and late REST overwrites WS.
await Promise.all([fetchKlines(symbol), fetchOrderBook(symbol)])
startDetailStream(symbol)

// Correct: connect alongside REST, guard by symbol/version, and only refresh
// klines when the chart interval changes.
const live = startDetailStream(symbol)
const initial = await Promise.allSettled([fetchKlines(symbol), fetchOrderBook(symbol)])
if (!live.depthReceived) applyRestDepth(initial)
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

## 7. Tests Required

- Unit tests for product defaults and non-empty overrides in PWA/Tauri,
  prefix/origin normalization, generic PWA same-origin URLs, generic Tauri
  configuration errors, HTTPS/loopback rejection, health URLs, and WS scheme
  conversion.
- Source/config tests for the dedicated development proxy target, API
  `ws: true`, `/health` proxy, and absence of a startup health gate.
- Request-layer tests for bootstrap Bearer removal, bootstrap 401 exclusion,
  singleton refresh, one replay, and failed-refresh session cleanup.
- WebSocket protocol tests for ticker/depth/trade subscribe frames,
  confirmation, verified direct payload shapes, heartbeat, and invalid frames.
- Market-detail lifecycle tests for REST/WebSocket races, latest-only depth
  frame coalescing, symbol filtering, reconnect resubscription and backoff,
  duplicate close/error idempotency, and stop-before-open cleanup.
- Adapter tests for new-coin quote quantity, safe news rich text, prediction
  order number, and stable margin pair labels.
- Run `npm run type-check`, `npm test`, `npm run build:pwa`, and
  `npm run build:tauri` after changing this contract's runtime paths.
