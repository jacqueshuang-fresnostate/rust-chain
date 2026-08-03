# Mobile Backend Integration Contract

## 1. Scope / Trigger

Apply this contract when changing mobile runtime backend configuration, Vite
proxying, Axios authentication behavior, public market WebSockets, or mobile
adapters for Rust user APIs. It also applies to quote/create/confirm workflows
whose mutation response becomes immediate UI state, and to wallet metadata
rendered by deposit, withdrawal, ledger, or quick-recharge pages.

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

Financial adapter signatures:

```ts
type SecondsDirection = 'up' | 'down'
type PredictionOutcome = 'yes' | 'no'

interface SecondsOrder {
  id: number
  direction: SecondsDirection
  stakeAmount: number
  payoutRate: number
  entryPrice?: number
  settlementPrice?: number
  status: string
  result?: string
}

interface PredictionQuote {
  quoteId: string
  outcome: PredictionOutcome
  assetId: number
  stakeAmount: number
  expiresAt: number
}

interface PredictionOrder {
  id: number
  outcome: string
  status: string
  result?: string
  refundAmount: number
}

interface DepositAsset { symbol: string; name?: string }
interface QuickRechargeConfig { currency: string; token: string; network: string }

openSecondsOrder(input: {
  productId: number
  durationSeconds: number
  direction: SecondsDirection
  stakeAmount: number
}): Promise<SecondsOrder>

requestPredictionQuote(input: {
  marketId: number
  outcome: PredictionOutcome
  assetId: number
  stakeAmount: number
}): Promise<PredictionQuote>

confirmPredictionQuote(quoteId: string): Promise<PredictionOrder>

fetchDepositAssets(): Promise<DepositAsset[]>
fetchDepositNetworks(assetSymbol: string, minimum?: number): Promise<DepositNetwork[]>
fetchQuickRechargeConfig(): Promise<QuickRechargeConfig>
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
- The response interceptor classifies a request from whether that exact request
  carried Authorization when it was sent. A guest/public request without a
  Bearer token keeps its own 401 as a local page error; it must not refresh,
  clear a session, or emit a delayed session-expired navigation after the user
  has moved to another public page.
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
- The market-detail visible price follows the freshest real source in this
  order: latest validated live trade, latest normalized forming candle, then
  ticker snapshot. MA5/MA10/MA20 are simple moving averages of those normalized
  real candle closes; never populate indicators or summary fields with demo
  values.
- The chart may call `fitContent()` for its initial non-empty dataset and after
  the replacement array for a real interval change arrives. A same-candle live
  update must update candles, volume, and MA series without consuming the
  pending interval fit or resetting the user's pan/zoom viewport.
- `MarketDetailView` remains the sole owner of the HIPPO REST/WebSocket detail
  session. Chart engines are render-only consumers of the same normalized
  `KlinePoint[]`; changing the local renderer must not call a market API,
  reconnect, resubscribe, clear points, or replace the active detail session.
- The default renderer is the locally bundled `klinecharts@10.0.0` base package.
  The selectable TradingView renderer is the locally bundled
  `lightweight-charts@5.2.0` package. Both render real OHLCV, MA5/MA10/MA20, and
  volume. Only the selected engine is mounted, and the TradingView renderer
  disables its optional attribution logo so it does not create an external
  anchor.
- KLineChart v10 receives HIPPO rows through an in-memory `DataLoader`; it must
  not use a Pro/default/remote loader. Same-candle and simple append changes use
  the subscribed local bar callback. Replacement history preserves an existing
  viewport unless it is the initial or interval-replacement fit. Preserve that
  viewport with a timestamp anchor plus its logical right-edge offset; retaining
  raw logical indexes alone shifts the user's window when history is prepended
  or trimmed.
- Each direct depth broadcast is a complete snapshot. Normalize numeric
  strings, reject the whole malformed frame, sort bids descending and asks
  ascending, retain at most 12 levels per side, and coalesce high-frequency
  snapshots so only the latest pending snapshot is committed per animation
  frame.
- Live trades are validated, prepended in arrival order, deduplicated by id,
  and capped at 16. A replayed id must not reorder or replace an already
  rendered trade.
- Switching between the split order book and latest-trades panels is a local
  presentation change. It must reuse the active detail session and must not
  reconnect, resubscribe, or replace REST/WS state.

### Financial mutation and wallet metadata contracts

- The Seconds adapter must map and retain `payout_rate`, `entry_price`,
  `settlement_price`, and the backend `status`. The active-order selector must
  recognize `opened` and `active` (and `pending` when emitted) without rewriting
  the stored status. Missing optional prices remain absent, not zero or a live
  ticker substitute.
- Seconds "estimated profit" is exactly
  `stakeAmount * payoutRate`. `payoutRate` is the profit rate, so neither the
  preview nor an active order may add the stake principal a second time.
- A resolved `openSecondsOrder()` response is the mutation commit point. Upsert
  that returned order immediately by `id`, close/lock the confirmation path,
  and show success before any reconciliation fetch. A later refresh failure is
  a refresh warning; it must retain the returned order, must not relabel the
  mutation as failed, and must not reopen a duplicate-submit path.
- Prediction quote request and response outcomes are the closed union
  `yes | no`. Normalize case only at the adapter boundary and reject every
  other response value; never pass an arbitrary string into confirmation.
- A resolved `confirmPredictionQuote()` response is likewise authoritative:
  upsert the returned order immediately. The order adapter must retain
  `result` and map `refund_amount` to `refundAmount`, because settlement labels
  distinguish wins/losses from invalid/refunded markets with those fields.
  Wallet/history reconciliation is a separate phase with the same
  success-versus-refresh-failure boundary as Seconds.
- Wallet screens display only server-owned metadata. `DepositAsset.name` stays
  optional; a missing name may show the exact asset symbol or generic localized
  support copy, but never a guessed asset name. Deposit networks may fall back
  only to the exact backend network identifier, never an invented arrival time.
  Quick-recharge `currency` and `token` remain empty/unavailable when omitted;
  do not default either field to `USD`, `USDT`, or Pencil/demo values.

### Financial-order Good / Base / Bad Cases

- **Good**: create returns an `opened` Seconds order with payout and entry
  price; the active panel renders that same object immediately and computes
  profit as `stakeAmount * payoutRate` even if order refresh then fails.
- **Base**: a deposit asset has a symbol but no name, and a network has no
  display name; the page shows the symbol and exact network code with no ETA.
- **Bad**: a successful create is followed by a failed list refresh, so the UI
  shows "submit failed", removes the returned order, or enables another submit.
- **Bad**: a prediction quote accepts `up`, drops `refund_amount`, or fills an
  absent quick-recharge token with `USDT`.

### Financial-order Wrong vs Correct

```ts
// Wrong: refresh owns mutation success, and payout adds principal.
const created = await openSecondsOrder(input)
await fetchSecondsOrders()
success.value = true
const profit = created.stakeAmount * (1 + created.payoutRate)

// Correct: the returned order is authoritative; refresh is best-effort.
const created = await openSecondsOrder(input)
orders.value = upsertById(orders.value, created)
success.value = true
const profit = created.stakeAmount * created.payoutRate
try { orders.value = await fetchSecondsOrders() }
catch { refreshWarning.value = true }
```

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
| Guest/public request without Authorization returns 401 | Reject locally; do not refresh, clear session, or navigate globally |
| WebSocket closes while listeners remain | Reconnect with bounded backoff and resubscribe |
| WebSocket has no listeners | Cancel timers, clear subscriptions, and close |
| Detail interval is unsupported | Do not create a socket or send an empty K-line subscription |
| Direct K-line timestamp is seconds/string, OHLCV has a non-string, or provider is empty | Reject the whole live frame; keep the last valid chart |
| REST K-line settles after one or more live candles | Merge REST behind live points; never replace a matching live `open_time` |
| Symbol/interval changes or repeats through an ABA sequence | Invalidate the old context, REST token, and pending animation-frame callback |
| Seconds create response omits `order` or has an invalid direction | Reject the mutation response; preserve existing orders and surface submit failure |
| Seconds create succeeds but order refresh fails | Keep/upsert the returned order and success state; surface only a refresh warning; keep duplicate submission locked |
| Prediction quote response outcome is not `yes` or `no` | Reject the quote; do not expose confirmation |
| Prediction confirm succeeds but wallet/history refresh fails | Keep/upsert the returned order, including `result`/`refundAmount`; retain success and show a refresh-specific warning |
| Wallet asset name, network ETA, recharge currency, or token is absent | Keep it absent or show exact server identifiers/localized unavailable copy; never synthesize USD/USDT, a name, or minutes |

## 7. Tests Required

- Unit tests for product defaults and non-empty overrides in PWA/Tauri,
  prefix/origin normalization, generic PWA same-origin URLs, generic Tauri
  configuration errors, HTTPS/loopback rejection, health URLs, and WS scheme
  conversion.
- Source/config tests for the dedicated development proxy target, API
  `ws: true`, `/health` proxy, and absence of a startup health gate.
- Request-layer tests for bootstrap Bearer removal, bootstrap 401 exclusion,
  guest/public 401 isolation, singleton refresh, one replay, and failed-refresh
  session cleanup.
- WebSocket protocol tests for ticker/depth/trade/K-line subscribe frames,
  supported interval normalization, confirmation, verified direct payload
  shapes, heartbeat, strict live K-line rejection, and malformed frames.
- Market-detail lifecycle tests for REST/WebSocket races, live K-line upsert,
  new-candle append, 160-point retention, latest-only depth/K-line frame
  coalescing, symbol/interval filtering, ABA interval isolation, reconnect
  resubscription and backoff, duplicate close/error idempotency, cancelled RAF
  callbacks, and stop-before-open cleanup. Race tests must execute fake sockets
  and delayed promises rather than assert source text alone.
- Market-detail presentation tests for live-trade/candle/ticker price priority,
  MA5/MA10/MA20 calculations, same-candle updates that preserve pan/zoom,
  interval replacement fitting, and order-book/trades tab switches that leave
  the active stream untouched.
- Chart-engine tests for exact local package versions, KLineChart's in-memory
  loader, disabled TradingView attribution/external anchors, one active renderer, persisted selection,
  real OHLCV/MA/volume wiring, lifecycle cleanup, and absence of remote chart
  code or data sources.
- Adapter tests for new-coin quote quantity, safe news rich text, prediction
  order number, and stable margin pair labels.
- Seconds adapter tests must assert exact raw-to-camel mapping for
  `payout_rate`, optional `entry_price`/`settlement_price`, and unchanged
  `opened`/`active` statuses. View-flow tests must assert
  `stakeAmount * payoutRate`, immediate upsert of the returned create order,
  and a delayed refresh rejection that leaves success/order state intact and
  does not enable a second mutation.
- Prediction adapter tests must accept only `yes`/`no` quote outcomes, reject a
  third value before confirmation, and preserve `result` plus `refund_amount`.
  Confirm-flow tests must use a successful confirm followed by rejected wallet
  or history promises and assert one confirm call, retained returned order,
  retained success, and refresh-specific feedback.
- Wallet adapter/source tests must cover missing optional names/display names
  and empty quick-recharge fields, and assert the client contains no guessed
  arrival-minute map or `|| 'USD'` / `|| 'USDT'` fallback.
- Run `npm run type-check`, `npm test`, `npm run build:pwa`, and
  `npm run build:tauri` after changing this contract's runtime paths.
