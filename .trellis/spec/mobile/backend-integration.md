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
VITE_BACKEND_DEV_PROXY_TARGET=http://127.0.0.1:8080
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
subscribe -> {"op":"subscribe","channel":"ticker","symbol":"BTCUSDT"}
heartbeat -> text "ping" / text "pong"
```

## 3. Runtime URL and Proxy Contracts

- Browser development always calls the Vite origin. `VITE_BACKEND_API_DOMAIN`
  does not bypass the development proxy.
- Vite proxies the normalized API prefix with `ws: true` and proxies
  `/health` to `VITE_BACKEND_DEV_PROXY_TARGET`.
- A production PWA with no explicit backend domain uses the current page
  origin, `/api/v1` for HTTP, `/health` for health, and
  `/api/v1/ws/public` for public market WebSockets.
- An explicit production backend domain must be an absolute HTTPS origin
  without credentials, path, query, or fragment.
- Production configuration rejects `localhost`, IPv4 loopback, IPv6 loopback,
  and `0.0.0.0`.
- A Tauri production build without a valid explicit backend domain keeps a
  diagnostic `BackendConfigurationError`; it must never fall back to device
  loopback.
- HTTP, health, and WebSocket URLs are derived from one validated runtime
  configuration. WebSocket schemes map `https -> wss` and `http -> ws`.

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
| Native production backend is missing or invalid | Show backend-not-configured diagnostics; make no loopback request |
| Request times out | Show the localized timeout state |
| Browser reports offline | Show the localized device-offline state |
| Network fails without an HTTP response | Show the localized backend/network state |
| Backend returns an error body | Preserve the backend message when present |
| WebSocket closes while listeners remain | Reconnect with bounded backoff and resubscribe |
| WebSocket has no listeners | Cancel timers, clear subscriptions, and close |

## 7. Tests Required

- Unit tests for prefix/origin normalization, PWA same-origin URLs, Tauri
  configuration errors, HTTPS/loopback rejection, health URLs, and WS scheme
  conversion.
- Source/config tests for the dedicated development proxy target, API
  `ws: true`, and `/health` proxy.
- Request-layer tests for bootstrap Bearer removal, bootstrap 401 exclusion,
  singleton refresh, one replay, and failed-refresh session cleanup.
- WebSocket protocol tests for subscribe, confirmation, ticker, heartbeat, and
  invalid frames.
- Adapter tests for new-coin quote quantity, safe news rich text, prediction
  order number, and stable margin pair labels.
- Run `npm run type-check`, `npm test`, `npm run build:pwa`, and
  `npm run build:tauri` after changing this contract's runtime paths.
