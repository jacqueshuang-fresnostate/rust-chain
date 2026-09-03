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
  idempotencyKey?: string
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
- Shared ticker consumers own independent symbol leases. The wire subscription
  set is the union of current leases: releasing one lease sends `unsubscribe`
  only for symbols no remaining consumer needs, and releasing the final lease
  closes the socket and clears heartbeat/reconnect work. Message/open/close
  handlers must verify the current socket identity so a late event from a
  released or failed connection cannot dispatch into a newer same-symbol
  listener.
- The client accepts subscription confirmations, direct ticker payloads, and
  text/JSON pong frames. Unknown or backend error frames must not be treated as
  ticker updates.
- Home market rows, spot trade summary/price inputs, and market-detail summary
  use the normalized ticker `last_price` as their single visible-price
  authority. Recent internal trades, K-line closes, and order-book bid/ask
  remain valid data for their own panels but must not replace that ticker price.
- Ticker reconciliation is newest-observation-wins. A live frame with a newer
  `observed_at` replaces the matching REST snapshot, while a delayed REST
  response or an older WebSocket frame must not move the visible price
  backwards. Every Home/Markets/Trade/Market Detail instance registers a
  stable consumer lease after its initial refresh and releases that exact
  lease on unmount. Repeated refreshes deduplicate the lease, and an outgoing
  route cannot close the shared ticker stream while an entering route still
  owns another lease.
- When `price_change_percent_24h` is present it is the authoritative Bitget spot
  percentage, including the valid value zero. Derive a compatible open price
  from that percentage only when `open_24h` is absent. Do not derive the visible
  percentage from `price_change_24h` while the percentage field is available.
- Direct ticker WebSocket payloads propagate `high_24h`, `low_24h`,
  `volume_24h`, and `price_change_percent_24h` together with `last_price` and
  `observed_at`. A newer frame replaces those dynamic fields as one coherent
  snapshot; a delayed REST response may refresh market metadata but must not
  mix its older 24-hour fields into that newer snapshot. The explicit
  percentage, including zero, wins. A compatibility frame containing only
  `last_price` preserves the last authoritative percentage instead of deriving
  one from a stale open price.
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
- The market-detail visible price follows the shared ticker authority above.
  Live trades remain the latest-trades feed and normalized forming candles
  remain the chart feed; neither can overwrite the summary ticker. MA5/MA10/MA20
  are simple moving averages of those normalized real candle closes; never
  populate indicators or summary fields with demo values.
- The chart may call `fitContent()` for its initial non-empty dataset and after
  the replacement array for a real symbol or interval change arrives. The
  normalized symbol plus interval is the renderer dataset key; changing that
  key alone only marks a pending fit and must wait for a new `points` value.
  A same-candle live update must update candles, volume, and MA series without
  consuming the pending dataset fit or resetting the user's pan/zoom viewport.
- `MarketDetailView` remains the sole owner of the HIPPO REST/WebSocket detail
  session. Chart engines are render-only consumers of the same normalized
  `KlinePoint[]`; changing the local renderer must not call a market API,
  reconnect, resubscribe, clear points, or replace the active detail session.
- The sole renderer is the locally bundled `lightweight-charts@5.2.0` package.
  It renders real OHLCV, MA5/MA10/MA20, and volume from the parent-owned points,
  enables the official attribution logo/link, and performs no market request.
  Same-candle and simple append changes call `update` on candles, volume, and
  the latest available MA rows. Replacement history preserves an existing
  viewport unless it is the initial or symbol/interval-replacement fit. Preserve
  that viewport with a timestamp anchor plus its logical right-edge offset;
  retaining raw logical indexes alone shifts the user's window when history is
  prepended or trimmed.
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

- `GET /wallet/ledger` is the authenticated combined spot/margin account read
  model. Every row carries authoritative `account_type=spot|margin`; optional
  `account_type=all|spot|margin` defaults to `all` and remains independent of
  the business `category` filter.
- The wallet-ledger adapter validates account source and never derives it from
  `change_type`. List identity is `(account_type, id)` because the two physical
  ledgers have independent numeric ID sequences. Account/category switches
  invalidate older requests, reset pagination, and preserve only results for
  the current combined filter.

- `GET /margin/products` is a public read-only catalog in contract mode. It
  maps product id/pair id, margin asset id/symbol, backend Logo, price
  precision, leverage levels, margin bounds, maintenance/hourly rates, and the
  capability envelope. Spot mode does not request this unrelated catalog.
- Margin wallets, user settings, position risk, and every margin mutation stay
  protected. A guest contract page may browse and switch products but must
  route settings, balance, order, cancel, and close intents through login.
- The capability envelope is authoritative. Mobile renders only advertised
  order/margin modes and gates TP/SL, strategy, bulk close, and position-risk
  controls with `take_profit_stop_loss`, `strategy_orders`, `bulk_close`, and
  `position_risk`; it never fills an unsupported surface with demo records.
- `GET /margin/wallets` retains `cross_accounts[]`. Filled positions poll
  `/margin/positions/{id}/risk` and map unrealized PnL, base quantity, return
  rate, margin ratio, isolated liquidation estimate, and liquidation distance.
  The risk adapter also strictly maps the optional `cross_account_risk` object
  to typed camel-case fields: margin asset/reference pair, the fixed
  `reference_pair_only_other_marks_static` assumption, account equity,
  maintenance, buffer, ratio, PnL, interest, trigger state, net/gross quantity,
  estimate status, conditional price/distance, and min/max mark observation
  times. Missing/null means an older compatible backend; a present malformed
  object is a contract error. Both mark times are required non-negative safe
  millisecond integers and the minimum may not exceed the maximum. Backend DECIMAL read models may become finite
  JavaScript numbers only inside this display adapter and must never feed an
  order, transfer, or other mutation payload.
  The server risk response remains the fallback authority for mark price, PnL,
  and return, and the sole authority for margin ratio and liquidation distance.
  Section 20 permits the transaction-record current-position card to project
  only mark price, PnL, and return from one newer exact shared ticker; all five
  fields remain unavailable (`--`) after a failed first risk read.
  Maintenance margin rate prefers a finite non-negative snapshot value and
  falls back only to the matching product rate. An isolated liquidation price
  prefers a finite positive snapshot value; when absent, display code may use
  the backend-equivalent position/product formula. A cross position consumes
  only `cross_account_risk`: `estimate_status=estimated` plus a finite positive
  conditional price renders the localized account estimate, and its distance
  bar consumes only the conditional account distance. Zero/near-zero net
  delta, already-liquidatable, no-positive-boundary, unavailable, unknown, or
  structurally valid but semantically unusable estimate values render localized
  "no stable single price" copy and
  never enter the isolated formula. The card states that only the current pair
  changes while all other marks remain fixed. When the optional object is
  absent, retain the legacy localized account-level fallback for rollout
  compatibility. Failed risk refreshes retain prior successful snapshots for
  positions that are still active. A successful `/margin/wallets` account
  reconciliation is authoritative for the active-position set and removes
  positions absent from that response together with their obsolete risk
  snapshots.
- Bulk close/cancel responses are partially successful by contract. Mobile
  must inspect both `positions` and `failures`, reconcile balances, and report
  counts; an HTTP 200 with failures is not an all-success message.
- Financial request decimals remain explicit decimal strings at the API
  boundary. Display adapters may convert read-only risk fields to numbers, but
  that conversion must never feed a later submission.

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
- Opening the Seconds confirmation creates one immutable review snapshot with
  the selected product/cycle, direction, stake, reference price, payout rate,
  and one generated idempotency key. The dialog and submit request read only
  that snapshot. Retrying the same still-open review must reuse its key; closing
  the dialog and creating a new review generates a new key. Revalidate the
  frozen product/cycle and current wallet availability before submission.
- Seconds reconciliation requests are generation-isolated. An older list or
  wallet response must not overwrite state produced by a newer open/reconcile
  cycle. Keep locally committed create responses until an authoritative list
  contains the same ID; when it does, the server row wins so settlement status
  can advance. Public products, ticker fallback, and K-lines start independently
  of private order/wallet refresh, so one protected endpoint failure does not
  hide otherwise available public market data.
- The Seconds pair picker renders only `fetchSecondsProducts()` results. It
  reuses the page's existing all-product ticker subscription and resolves each
  visible price through `liveTickerSnapshots -> selected candle when applicable
  -> marketStore` without starting another socket. Logo authority is the
  backend market snapshot in `baseIconUrl -> iconUrl -> AssetMark symbol`
  order. Missing prices render `--`; neither Pencil sample pairs nor synthetic
  image/price fallbacks enter production. Choosing a row delegates to the
  existing `selectProduct()` path so cycle, minimum stake, and K-line switch
  together while the independent `orders` collection remains unchanged.
- The dedicated Seconds history page requests authenticated pages from
  `GET /seconds-contracts/orders` with `limit=20` and a monotonically advanced
  `offset`. It keeps the shared `isActiveSecondsOrder` boundary after DTO
  mapping, renders only non-active rows, preserves unknown result/status source
  values, and reads entry and settlement prices only from their optional API
  fields; a missing or invalid price stays unavailable and is never replaced
  with a live ticker.
- The history adapter maps the backend `has_more` continuation signal and
  computes the next offset from the raw number of returned rows, never from the
  direction-filtered or de-duplicated count. A compatibility payload without
  `has_more` may infer continuation from a full page, but a page that adds no
  new order ID must terminate pagination so a legacy server that ignores
  `offset` cannot create an infinite request loop.
- A bottom `IntersectionObserver` sentinel requests the next page when it nears
  the viewport. Loading guards keep one page request in flight. Append merges
  by order ID, lets the later authoritative row replace the earlier row, and
  preserves deterministic newest-first presentation. `has_more=false`, an
  empty page, or no merge progress removes the continuation state.
- History reads capture the exact authenticated session token and a request
  generation. They are invalidated on token replacement, logout, retry
  supersession, or unmount. Initial and append errors are separate: an append
  failure keeps all loaded cards and exposes a local retry for the same offset.
  Guest, initial loading, initial error, list, filtered-empty, and empty states
  remain truthful; a direction filter with no loaded match does not suppress a
  still-available continuation sentinel.
- Seconds history profit/loss is a read-only presentation derived only from the
  immutable order snapshot. A `win` displays net profit as
  `stakeAmount * payoutRate`, never principal-inclusive payout; a `loss`
  displays `-stakeAmount`. Cancelled, missing-result, and unknown-result rows
  keep the amount unavailable instead of fabricating zero or inferring from
  entry/settlement prices. The unit is always the order's
  `stakeAssetSymbol`, and the derived display value must never feed a wallet or
  order request.
- The Seconds trading page owns one non-persisted settlement-result tracker per
  component session. Reconciliation checks only IDs previously observed as
  active, then records active rows from the new authoritative list; this order
  prevents first-load historical wins/losses from replaying. An active create
  response is tracked immediately. Only a later API row with
  `status=settled` and `result=win|loss` may enter the notice queue; ticker prices, countdown
  completion, entry price, and settlement price never determine the outcome.
- A tracked non-active row without a result stays eligible and keeps expiry
  reconciliation retrying until a later snapshot supplies `win|loss` or marks
  it cancelled. Cancellation remains terminal even if a reordered older active
  snapshot arrives later. Results are de-duplicated for the page session, sorted by
  `expiresAt` then ID within each reconciliation, and appended to a FIFO queue.
  Logout, private-state reset, and unmount clear tracker and queue so a result
  cannot cross an account or route boundary. A create response that resolves
  after that session boundary is stale and must not rebuild the cleared state.
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
- **Good**: a settled 100 USDT win at rate 0.8 renders `+80 USDT`; a loss
  renders `-100 USDT`, both from the returned order snapshot.
- **Base**: a deposit asset has a symbol but no name, and a network has no
  display name; the page shows the symbol and exact network code with no ETA.
- **Base**: a cancelled or future-result Seconds row renders an unavailable
  profit/loss amount while preserving its source status/result label.
- **Bad**: a successful create is followed by a failed list refresh, so the UI
  shows "submit failed", removes the returned order, or enables another submit.
- **Bad**: a prediction quote accepts `up`, drops `refund_amount`, or fills an
  absent quick-recharge token with `USDT`.
- **Bad**: a history win adds principal again, derives outcome from live
  prices, or labels an unknown result as a zero-value profit.

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
- Guest spot mode must not request the unrelated margin catalog. Guest
  contract mode may request the public catalog but no private margin endpoint.

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
| Seconds product exists but has no market image | Render the deterministic `AssetMark` symbol fallback; do not guess another asset image |
| Seconds ticker is absent or invalid for one picker row | Render `--` for that row and keep all other products selectable |
| Seconds pair is selected while other pairs have active orders | Switch only selected product/cycle/amount/K-line state; retain every active order and the shared product subscription |
| Seconds history price is missing or malformed | Render the unavailable placeholder; do not coerce it to zero or substitute market data |
| Seconds history result/status is unknown | Show the trimmed backend source value instead of an incorrect known translation |
| Seconds history result is `win` | Show signed net profit `stakeAmount * payoutRate` in `stakeAssetSymbol`; do not add principal |
| Seconds history result is `loss` | Show signed loss `-stakeAmount` in `stakeAssetSymbol` |
| Seconds history result is absent, cancelled, or unknown | Show an unavailable profit/loss amount; do not infer from prices or fabricate zero |
| Seconds history request settles after logout, retry, or unmount | Ignore the stale response and preserve the newer guest/request state |
| Seconds history first page succeeds with `has_more=true` | Observe the bottom sentinel and request the returned next offset once |
| Observer fires repeatedly while an append is active | Keep one in-flight request; do not request the same page twice |
| A later page overlaps an existing order ID | Keep one row and replace it with the later authoritative payload |
| A full compatibility page adds no new ID | Mark pagination exhausted; do not loop against a server that ignored offset |
| A later-page request fails | Keep existing cards, preserve the same offset, and expose a local retry |
| Exact token changes or follows an A→B→A sequence while a page is in flight | Treat the old page as stale and reset pagination for the current session |
| Seconds first load contains historical wins/losses | Establish active-order baselines only; enqueue no historical notice |
| Tracked active Seconds order becomes non-active without a result | Keep tracking and retry reconciliation until a result or cancellation arrives |
| Tracked Seconds order returns `win|loss` repeatedly or in a reordered list | Enqueue it once for the page session and preserve FIFO display order |
| Seconds order is cancelled, user logs out, or the page unmounts | Emit no result for cancellation and clear all settlement tracking/queue state |
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
- Market presentation tests for ticker authority on Home, spot trade, and
  market detail; newer-frame acceptance; older-frame rejection; delayed REST
  reconciliation; authoritative percentage mapping (including zero);
  MA5/MA10/MA20 calculations; same-candle updates that preserve pan/zoom;
  interval replacement fitting; and order-book/trades tab switches that leave
  the active stream untouched.
- Chart-runtime tests for exact `lightweight-charts@5.2.0`, absence of
  `klinecharts`, one active renderer, official attribution enabled, accessible
  non-image container semantics, real OHLCV/MA/volume wiring, same-candle and
  append `update`, timestamp-anchored replacement restore, symbol/interval
  dataset fitting only after new points, mobile gestures, lifecycle cleanup,
  and absence of remote chart code or data sources.
- Adapter tests for new-coin quote quantity, safe news rich text, prediction
  order number, and stable margin pair labels.
- Margin-risk adapter/projection tests cover every `cross_account_risk`
  snake-case to camel-case field, strict finite-decimal handling, a stable
  account estimate and conditional distance, required ordered mark times,
  malformed-object rejection, exact/near hedges,
  already-liquidatable/no-positive-boundary states, invalid values, the old
  backend fallback, and the unchanged isolated server/local-formula path.
  Position-card source/parity tests lock the dynamic account label, explicit no
  stable price copy, conditional-distance source, visible accessible scenario
  note, unchanged action order, and symmetric `zh-CN`/English keys.
- Seconds adapter tests must assert exact raw-to-camel mapping for
  `payout_rate`, optional `entry_price`/`settlement_price`, and unchanged
  `opened`/`active` statuses. View-flow tests must assert
  `stakeAmount * payoutRate`, immediate upsert of the returned create order,
  and a delayed refresh rejection that leaves success/order state intact and
  does not enable a second mutation.
- Seconds history tests must execute delayed page promises to prove guest
  isolation, exact-token/ABA isolation, latest-request-wins retry,
  logout/unmount invalidation, and initial versus append error recovery. They
  must exercise `{ limit: 20, offset }` advancement, repeated observer guards,
  overlap de-duplication with later-row authority, empty/terminal/no-progress
  exhaustion, shared active filtering, optional invalid prices, known
  translations, and visible unknown result/status source values. Source/view
  tests must also prove that an `IntersectionObserver` owns the bottom sentinel
  and that append failure preserves the rendered list with a local retry.
- Seconds settlement-notice tests must execute first-load historical results,
  active-to-win/loss transitions, repeated and reordered snapshots, same-batch
  expiry sorting, delayed missing results, cancellation, create-response
  tracking, reset, and FIFO de-duplication. View contracts must prove
  `applyReconciledOrders()` passes the raw API list to the tracker and amount
  presentation reuses `secondsOrderProfitLossPresentation()`.
- Seconds pair-picker tests must prove rows come from the API collection, Logo
  precedence is `baseIconUrl -> iconUrl -> symbol`, prices use the existing
  product-wide ticker state, missing prices remain `--`, and choosing a product
  calls the existing selection/K-line path without assigning `orders.value`.
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

## 8. Selected Financial Confirmation Handoff

### 1. Scope / Trigger

Apply this scenario when a selected Pencil confirmation surface submits a
wallet transfer, renders earn fee rules, or derives immediate post-mutation UI
state from a backend response. It prevents a successful mutation from being
followed by guessed balances, zero-filled missing accounts, or a refresh race
that overwrites the authoritative response.

### 2. Signatures

```ts
interface WalletTransferResult {
  transferId: string
  spotWallet: WalletAccount
  marginWallet: WalletAccount
}

interface WalletAccount {
  assetId: number
  symbol: string
  logoUrl?: string
  marginTransferEnabled?: boolean
  available: number
  frozen: number
  locked: number
}

transferWalletFunds(
  assetSymbol: string,
  from: 'spot' | 'margin',
  to: 'spot' | 'margin',
  amount: number,
): Promise<WalletTransferResult>

interface EarnProduct {
  redemptionFeeRate?: number
  maturityProfitFeeRate?: number
  earlyRedeemFeeBasis?: string
  earlyRedeemFeeRate?: number
}
```

Backend transfer response fields:

```text
transfer_id
spot_wallet.asset_id|available|frozen|locked
margin_wallet.asset_id|available|frozen|locked
GET /margin/wallets -> wallets[].asset_id|asset_symbol|logo_url|margin_transfer_enabled|available|frozen|locked
```

Earn product response fields:

```text
redemption_fee_rate
maturity_profit_fee_rate
early_redeem_fee_basis
early_redeem_fee_rate
```

### 3. Contracts

- Every `/margin/transfers` request sends a unique `idempotency_key` and keeps
  `asset_symbol`, `from`, `to`, and decimal `amount` unchanged.
- Treat `/margin/wallets` as both the margin balance response and the backend-owned inbound asset catalog. A spot-to-margin picker intersects spot wallets with rows whose `marginTransferEnabled !== false`; a margin-to-spot picker keeps all returned margin rows so disabling inbound eligibility cannot hide withdrawable balances.
- Assets renders separate spot and margin scopes from the same wallet read cycle. The all-account total may aggregate them, but selecting Margin must display only the real margin `available`, `frozen`, `locked`, Logo, asset count, and USDT estimate. Scope changes are presentation-only and never trigger an extra financial request.
- An enabled margin asset may arrive with three zero buckets before lazy account creation. Keep it available in the inbound picker but omit it from positive-holding rows; do not synthesize a wallet mutation or a non-zero balance.
- The returned spot and margin snapshots are authoritative. Upsert both into
  their respective stores immediately; preserve an already-known asset logo as
  presentation metadata only.
- Do not issue an unconditional account refresh after a successful transfer.
  A later user-driven refresh may replace the snapshots, but it must not erase
  success feedback or reopen duplicate submission.
- A missing source wallet is `null`/unavailable, not a zero balance. Disable
  submission and display `--` until the wallet API returns that account.
- Earn fee fields are optional. Render a localized unavailable value when a
  field is absent; never default it to zero. Percentage-based "all" input is
  `min(real wallet available, product maxSubscribe when present)`.
- Prediction and earn confirmation dialogs use the shared modal helper. Their
  API data remains in component state; the visual sheet never becomes a second
  mutation or refresh owner.

### 4. Validation & Error Matrix

| Condition | Required behavior |
| --- | --- |
| Transfer source account is absent | Show `--`, disable confirm, and make no request |
| Spot asset is absent from the enabled margin catalog | Omit it from spot-to-margin choices and make no request |
| Margin asset eligibility is later disabled | Keep an existing margin wallet visible and available for margin-to-spot transfer |
| Amount is non-finite, non-positive, or above real available | Show localized validation feedback; make no request |
| Transfer succeeds | Upsert both returned wallets and retain success feedback |
| Transfer response omits a wallet snapshot | Reject the response as a submission failure; do not synthesize a wallet |
| Earn fee value is absent or invalid | Preserve `undefined` and show localized unavailable copy |
| Earn "all" exceeds product maximum | Clamp to the real product maximum |
| Follow-up read fails after another confirmation mutation | Keep the returned mutation object and expose refresh-specific feedback |

### 5. Good / Base / Bad Cases

- **Good**: transfer 25 USDT from spot to margin; one idempotent request
  returns both wallets, both lists update immediately, and the success message
  remains visible.
- **Base**: the margin wallet for the selected asset does not exist; the sheet
  shows no available value and keeps Confirm disabled.
- **Bad**: treat a missing wallet as `0`, fire the request anyway, then call a
  refresh that clears the success state or replaces the returned snapshots.

### 6. Tests Required

- Adapter/source tests assert the transfer idempotency key and exact mapping of
  `transfer_id`, `spot_wallet`, and `margin_wallet`.
- View-flow tests assert null account handling, positive finite amount checks,
  both wallet upserts, retained success feedback, and no unconditional refresh.
- Assets adapter/view tests assert `margin_transfer_enabled` mapping, inbound picker intersection, outbound visibility after disable, separate spot/margin estimates and holdings, zero-balance catalog rows, and account-scope changes without refetching.
- Earn adapter tests assert all four optional fee fields and preserve absent
  values as `undefined`.
- Earn view tests assert the "all" clamp and localized unavailable fee copy.
- Modal source/browser tests assert Teleport-owned viewport overlay, Escape,
  focus trapping/restoration, body scroll restoration, and 44px controls.

### 7. Wrong vs Correct

```ts
// Wrong: ignore the mutation result and guess that a missing wallet has zero.
await transferWalletFunds(symbol, from, to, amount)
await loadAccounts()
const available = account?.available || 0

// Correct: use the response snapshots and keep absence explicit.
const result = await transferWalletFunds(symbol, from, to, amount)
spotAccounts.value = upsertWalletAccount(spotAccounts.value, result.spotWallet)
marginAccounts.value = upsertWalletAccount(marginAccounts.value, result.marginWallet)
const available = account?.available ?? null
```

## 9. Market Favorites, Convert Pairs, and Backend Logo Contract

### Scope / Trigger

- Apply this contract when a mobile market, wallet, margin, or convert surface
  consumes backend-owned image metadata.
- For Swap, `GET /convert/pairs` owns from/to visual metadata while protected
  wallet accounts own only authenticated balances and holding filters.

### Signatures

```text
GET    /user/market-favorites
PUT    /user/market-favorites/:normalizedSymbol
DELETE /user/market-favorites/:normalizedSymbol
```

```ts
interface MarketFavoriteRecord {
  market_id: number
  symbol: string
  logo_url?: string | null
  base_logo_url?: string | null
  quote_logo_url?: string | null
}

interface MarketFavoritesResponse {
  favorites: MarketFavoriteRecord[]
}

interface BackendConvertPair {
  from_asset_logo_url?: string | null
  to_asset_logo_url?: string | null
  min_amount: string | number
  max_amount?: string | number | null
  target_min_amount?: string | number | null
  target_max_amount?: string | number | null
}

interface ConvertPair {
  fromAssetLogoUrl?: string
  toAssetLogoUrl?: string
}

mapDirectionalConvertPairs(pairs: readonly BackendConvertPair[]): ConvertPair[]
swapPairSelectionKey(pair: {
  id: number
  fromAssetId: number
  toAssetId: number
}): string
```

### Contracts

- The mobile market mapper retains `logo_url`, `base_logo_url`, and
  `quote_logo_url`. Market-pair marks try the pair image first, the backend
  base-asset image second, and the existing accessible initial last. Do not
  derive asset image paths from symbols or add an external coin-image service.
- `fetchMarginWallets()` maps backend `logo_url` to `WalletAccount.logoUrl`.
  Assets keeps the spot-wallet image first and the margin-wallet image second
  when combining real wallet rows.
- `fetchConvertPairs()` trims `from_asset_logo_url` and `to_asset_logo_url` at
  the adapter boundary. Missing, `null`, empty, and whitespace-only values map
  to `undefined`; no default URL is derived. A present non-string Logo is a
  contract error rather than another spelling of absence. Pair-side symbols
  are trimmed and uppercased before they become selection or deduplication
  keys.
- Swap pay/receive marks read only the selected pair's direction-specific Logo.
  Picker rows are built from the corresponding pair direction and retain the
  first non-empty pair API Logo when a symbol repeats. Wallet accounts supply
  only balances and the holding filter, never Swap Logo metadata; the Swap
  wallet lookup stores normalized symbol-to-number entries rather than whole
  wallet objects.
- One enabled backend convert row is bidirectional. The adapter emits its
  configured direction and a reverse direction that swaps IDs, symbols, and
  Logos while applying `target_min_amount/target_max_amount` as the reverse
  source limits. If the response contains an explicit row for that reverse
  direction, the explicit row wins so its own fee and limits remain
  authoritative.
- Selection identity is `${configId}:${fromAssetId}:${toAssetId}` rather than
  the config ID alone because the two projected directions intentionally share
  one backend row ID. The selection key is client state only; quote requests
  continue to send exact `from_asset_id`, `to_asset_id`, and `from_amount`.
- A direction click keeps the typed amount but clears the old quote and stale
  error/success feedback before rendering the new pay/receive assets, balance,
  Logo, and limits. Clicking a second time resolves the opposite directional
  pair and returns to the original selection.
- A missing or failed convert-pair image continues through `AssetMark`'s
  accessible symbol-initial fallback.
- One Pinia store owns authenticated favorites for Home, Markets, Spot Trade,
  and Market Detail. App startup loads it when the session is authenticated;
  logout and session expiry reset favorites, pending symbols, and request
  state. Late reads or mutations from an older session must not repopulate the
  current session.
- Favorite path symbols are normalized and URL encoded. GET requests and
  same-symbol mutations are deduplicated. Add/remove may update optimistically,
  but failures restore the previous state and clear the pending state so the
  action can be retried.
- The mobile client neither reads nor writes the retired
  `hippo-mobile-market-favorites` local-storage key. A guest star action pushes
  Login with the current internal `route.fullPath` as `redirect` and never
  creates an anonymous favorite.
- Star controls retain a 44px target, `aria-pressed`, pending `aria-busy`, and
  disabled semantics while the same symbol is being saved.

### Validation & Error Matrix

| Condition | Required behavior |
| --- | --- |
| Convert Logo is a non-empty string | Trim once at the adapter and retain it |
| Convert Logo is missing, `null`, empty, or whitespace | Map to `undefined`; let `AssetMark` use its local initial |
| Convert Logo is another JSON type | Reject the pair response as a contract error |
| Pair asset symbol contains outer whitespace or lowercase | Trim and normalize to uppercase before matching/deduplication |
| Same directional symbol appears on multiple pairs | Keep one picker row and the first non-empty API Logo |
| Wallet account has a different Logo | Ignore it for Swap visuals; consume only its available balance |
| Selected/reversed pair changes | Read the new pair's direction-specific Logo reactively |
| Only one configured direction exists | Project the supported reverse direction using target-side limits |
| An explicit reverse row also exists | Use that exact row; do not overwrite it with a projection |
| Forward and reverse share one config ID | Distinguish them with config ID plus both directional asset IDs |
| Direction changes after a quote or message | Keep amount; clear quote, error, and success; send no request until the user asks for another quote |

### Good / Base / Bad Cases

- Good: the selected BTC→USDT pair renders its exact from/to API images, then
  switching to USDT→BTC renders the reverse pair's two distinct API images.
- Good: one BTC→USDT row with target minimum 10 projects USDT→BTC with minimum
  10 USDT; the quote request sends USDT/BTC asset IDs and no synthetic key.
- Base: a pair Logo is `null`; `AssetMark` receives `undefined` and renders the
  accessible symbol initial without inventing an image URL.
- Base: both directions are configured explicitly; each direction keeps its
  own backend row ID, fee, limits, and images.
- Bad: the picker obtains images from `wallet_accounts.logo_url`, groups raw
  `" btc "` separately from `BTC`, or silently accepts a numeric Logo value.
- Bad: store only `pair.id` as selected state, look for a second physical row,
  and silently do nothing when the backend exposes one bidirectional rule.

### Tests Required

- Adapter tests cover normalized paths, the GET envelope, mutation response,
  pair/base/quote Logo mapping, and nullable/blank convert-pair Logo mapping.
- Store tests cover shared loading, same-symbol mutation deduplication,
  optimistic rollback, session reset, and stale in-flight response isolation.
- Source/view tests assert all four market surfaces use the shared store, guest
  redirects remain internal, the retired local-storage path is absent, and
  Assets still renders wallet-owned Logo metadata. Swap tests execute
  direction-specific picker deduplication, assert selected pair Logo bindings,
  and prove wallet metadata is limited to balances/holding filters. They also
  execute single-row bidirectional projection, target-side limits, explicit
  reverse precedence, direction-aware selection keys, reverse/picker
  reactivity, quote/message clearing, and the `AssetMark` source-exhaustion
  path instead of relying only on source-text guards.
- Run the focused market/favorites tests and `npm run type-check`; include the
  full mobile suite and PWA build at the final task gate.

### Wrong vs Correct

```ts
// Wrong: user wallet metadata substitutes for the trading product contract.
const logoUrl = walletBySymbol.get(symbol)?.logoUrl

// Correct: pair DTO owns visuals; wallet state owns only balance.
const logoUrl = side === 'from' ? pair.fromAssetLogoUrl : pair.toAssetLogoUrl
const balance = availableBySymbol.get(symbol) ?? 0
```

```ts
// Wrong: both directions share an ID, so setting the ID cannot change the UI.
pairId.value = reverse.id

// Correct: preserve the backend ID but include the real request direction.
pairSelectionKey.value = swapPairSelectionKey(reverse)
```

## 10. Home and Assets Today Return Contract

```ts
interface TodayReturn {
  scope: 'realized'
  reportingAsset: 'USDT'
  amount: number
  basisAmount: number
  rate: number
  periodStartAt: number
  calculatedAt: number
  status: 'complete' | 'partial'
  missingPriceAssets: string[]
}

fetchTodayReturn(): Promise<TodayReturn>
```

- Home and the authenticated Assets member Hero consume only protected
  `GET /wallet/today-return` for today return. Guests do not start that read;
  both surfaces render amount and `rate * 100` only for `status=complete`.
- `partial`, request error, loading, guest, and privacy-hidden states remain
  non-numeric and distinct. Privacy hiding takes precedence over loading/error/
  partial detail so missing-price asset symbols cannot reveal account activity.
  Complete zero renders zero and `0.00%`, not `--`.
- The adapter validates realized scope, USDT reporting, finite decimals,
  supported status, an exact UTC-day period, and normalized missing-price
  symbols. Decimal strings must use decimal notation, timestamps must be safe
  integer seconds or milliseconds, and `complete` cannot carry missing assets.
  It must not coerce malformed financial fields to zero. Signed decimal zero
  is canonicalized to positive zero so a complete zero never renders `-0`.
- Today-return reads are keyed by the exact authenticated session token, are
  latest-request-wins, and are invalidated on token replacement, logout, and
  component unmount. A late response from another login/session must not write
  into the current Home or Assets state. Assets wallet/margin reads and pending
  transfer mutations follow the same exact-token boundary so a truthy token
  replacement cannot expose or commit the previous account's state.
- Today-return and wallet/margin reads remain independent: failure in one does
  not clear or relabel a successful result from the other. Positive and
  negative values use existing semantic tones; zero is neutral. The Assets
  Hero constrains long amount/status text inside its existing Pencil grid.

Required tests cover complete/partial mapping, seconds/milliseconds timestamp
normalization, malformed response rejection, true zero, privacy precedence,
positive/negative/neutral tone guards, delayed latest-request/login/unmount
isolation, the Home complete-status display guard, executable Assets
presentation states, wallet/today-return request independence, exact-token
account/transfer invalidation, and narrow-grid overflow guards.

## 11. Home Return History Contract

- Home fetches only protected `GET /wallet/return-history?days=1|7|30|180` for
  its chart. The API adapter owns the period whitelist and strict mapping;
  guests make no request.
- The mapper validates realized/USDT, period echo, exactly N UTC-midnight
  points, 86,400,000ms continuity, point/top complete-partial consistency,
  nullable partial fields, sorted missing-price unions, rates, cumulative sums,
  and complete summary equality. Malformed decimals are rejected rather than
  coerced to zero.
- Geometry consumes only complete mapped history and plots
  `[0, ...daily cumulativeAmount]` in the existing 358x153 SVG. Its y-domain
  includes zero; all-zero history is centered at y=76.5; the final x is 358.
- Period changes, retries, exact-token changes, logout, and unmount reuse
  `createSessionRequestLifecycle`. Clear the old DTO/path before every load so
  period ABA and late responses cannot restore stale account data.
- Hidden, loading, partial, and error states render no path, endpoint, or
  accessible financial values. Complete visible history supplies a localized
  summary and UTC-day table; partial/error remain retryable.

Required tests cover all four periods, strict DTO failures, one-day baseline,
zero/positive/negative/cross-zero geometry, partial cumulative propagation,
guest/latest-request/ABA/token/unmount isolation, privacy, source contracts,
44px period/retry controls, and symmetric locale keys.

## 12. Wallet Ledger Account And Category Contract

```ts
type WalletLedgerAccountType = 'spot' | 'margin'
type WalletLedgerAccountFilter = 'all' | WalletLedgerAccountType

type WalletLedgerCategory =
  | 'funding' | 'spot' | 'margin' | 'seconds' | 'convert'
  | 'earn' | 'new_coin' | 'loan' | 'prediction' | 'other'

interface WalletLedgerEntry {
  id: number
  accountType: WalletLedgerAccountType
  // remaining authoritative ledger fields
}

fetchWalletLedger(options?: {
  limit?: number
  offset?: number
  accountType?: WalletLedgerAccountFilter
  category?: WalletLedgerCategory
  changeType?: string
}): Promise<WalletLedgerPage>
```

- Both account and category unions must match the Rust API exactly. Omitted
  account means `all`, while omitted category means every business category;
  `change_type` remains an optional exact compatibility filter and may be
  combined with both dimensions.
- The adapter strictly validates authoritative entry account source and
  category, decimal amount/fee/balance fields, timestamps, and pagination
  totals. A selected account/category response containing another value is a
  contract error and must not enter page state.
- Amount, post-change balance, and positive fee presentation uses one
  locale-aware ledger formatter with the entry's authoritative
  `precisionScale` in `0..=18`. Insignificant trailing zeroes are removed, the
  smallest positive unit at the declared scale remains visible, and negative
  zero is normalized to neutral zero.
- Initial load, cached refresh errors, load-more errors, empty state, and
  exhaustion remain distinct. Pagination advances its offset by response rows
  consumed, not by the deduplicated visible-entry count, and an empty page is
  exhausted even if inconsistent metadata claims more pages.
- Reads are keyed by exact session token, selected account, and selected
  category. Account/category changes, token replacement/logout, and unmount
  invalidate older reads before they can mutate entries, errors, loading state,
  or pagination.
- Two physical ledgers may emit the same numeric ID. Merge/deduplication and
  rendered list keys therefore use `accountType:id`, while pagination still
  advances by the raw number of response rows consumed.
- Local adapter/contract diagnostics are not user copy; the view renders the
  localized ledger failure message instead of exposing internal English error
  strings.

Required tests cover exact account/category unions, strict response mapping,
account/category mismatch, overlapping numeric IDs, row-based offset/exhaustion,
local-date ordering, account/filter/session/unmount stale responses,
known/unknown type presentation, pluralization, signed zero, declared-scale
amount/balance/positive-fee precision, state branches, 44px controls, and 320px
horizontal-overflow guards.

## 13. Margin Contract Trading Selection Contract

### 1. Scope / Trigger

Apply this contract when changing the mobile contract branch of `TradeView`,
its leverage/margin-mode/pair sheets, or the user margin-setting adapter. It
prevents Pencil samples from becoming fabricated order capabilities and keeps
saved user settings authoritative across reloads and pair changes.

### 2. Signatures

```ts
interface MarginUserSetting {
  leverage: number | null
  longLeverage: number | null
  shortLeverage: number | null
  marginMode: 'cross' | 'isolated' | null
}

type MarginOrderType = 'market' | 'limit'

interface MarginOrderCapabilities {
  orderTypes: MarginOrderType[]
  pricePrecision: number | null
}

fetchMarginSetting(productId: number): Promise<MarginUserSetting>
updateMarginLeverage(
  productId: number,
  leverage: number | { longLeverage: number; shortLeverage: number },
): Promise<void>
updateMarginMode(
  productId: number,
  mode: 'cross' | 'isolated',
): Promise<void>

placeMarginOrder(input: {
  productId: number
  side: 'long' | 'short'
  marginMode: 'cross' | 'isolated'
  leverage: number
  marginAmount: number
  orderType: MarginOrderType
  price?: string
  idempotencyKey: string
}): Promise<void>

closeMarginPosition(positionId: string, intent?: {
  percentage: number
  idempotencyKey: string
}): Promise<void>
closeAllMarginPositions(productId?: number): Promise<MarginBatchActionResult>
```

The selected production surfaces are `cjzfi/p6GfgT` for the main contract
workspace, `NTiiS/CulR4` for the current directional-leverage sheet, and
`aNuw6/PKAcD`, `Crw8v/YuKtQ` for margin mode and pair selection. The prior
`f0L8yf/R8t0p` leverage pair remains declared as historical source metadata but
must not override the current dual-direction structure. The mock status bar in
those frames is native OS chrome and is not rendered by the web application.

### 3. Contracts

- `GET /margin/settings/:product_id` is protected and returns nullable legacy
  `leverage`, directional `long_leverage` / `short_leverage`, and `margin_mode`.
  Each recognized saved value overrides the product default only when it still
  exists in that product's exact `leverageLevels` or `marginModes` set.
- A missing setting row is HTTP 404 and maps to `{ leverage: null,
  longLeverage: null, shortLeverage: null, marginMode: null }`; every other
  HTTP/network failure remains observable. Against an older response that lacks
  directional fields, both directions fall back to the valid legacy value.
- The current Mobile client atomically PATCHes both directional values. It keeps
  the numeric legacy adapter overload only for older call sites; it never issues
  a partial directional request. Local state changes after a successful
  response, never optimistically before it, and a failed request keeps both
  drafts visible for explicit retry.
- `TradeView` owns `longLeverage` and `shortLeverage`; the active order/review
  leverage is derived from `side` (`buy -> long`, `sell -> short`). Opening a
  review freezes that direction's current setting so later side/settings changes
  cannot rewrite the confirmed request.
- The pair sheet renders only real `/margin/products` rows and Market Store
  tickers. Missing ticker image, price, or change remains an asset-letter
  fallback or `--`; no Pencil sample value is copied into production.
- The margin-product adapter retains only the exact `/margin/products`
  `capabilities.order_types` values it recognizes and retains pair
  `price_precision`; it never inserts a local market/limit fallback into an
  empty capability set. A valid current selection survives refresh, otherwise
  choose advertised limit first to match the selected Pencil initial state,
  then the first real capability, or `null`.
- The contract order-type trigger opens a dedicated sheet. Opening, backdrop,
  close, and Escape preserve the current value; only an explicit advertised
  option commits and closes. Pair and order-type sheets are public after an
  exact product/capability is available; leverage and margin-mode sheets remain
  protected because they persist a user setting. The amount input and
  contract percentage range remains margin amount, not base quantity or notional.
- The selected input visual contract from `IpirH/mcfEf` uses one outer field
  shell and a two-row information hierarchy. Price is 138x54 with a 9px
  `价格 (QUOTE)` label and a 17px/22px numeric value; margin is 202x48 with a
  9px `保证金 (ASSET)` label,
  a 15px/20px numeric value, and a trailing settlement asset. Idle shells use
  a transparent 1px border; `:focus-within` owns the complete accent ring while
  the nested input keeps border, outline, and box-shadow at zero. Percentage
  control is one native `0..100` range with `1%` steps, a 4px progress track,
  one movable thumb, a visible current value, and a 44px interaction height;
  it renders no fixed interval dots.
- Market keeps the price field read-only on the live ticker and sends no
  `price`. Limit makes the field editable and may fill from long ask/short bid,
  falling back to the latest ticker. The entered plain decimal must be positive
  and use no more than pair `pricePrecision` effective fractional digits; never
  round an invalid user limit into range.
- The margin-product adapter retains `min_margin` and optional `max_margin` as
  `minMargin` and `maxMargin`. A missing, null, non-finite, zero, or negative
  maximum maps to `null`; it must never become a fabricated zero cap.
- Every contract range percentage, including the 100% endpoint, uses
  `min(real margin-wallet available, positive product maxMargin)` as their
  percentage base. With no product maximum they use the real available balance.
  Spot percentage behavior remains unchanged.
- One pure margin-amount validation result owns the inclusive minimum/maximum
  decision for the field state, opening the review, and the guard immediately
  before `placeMarginOrder`. A value equal to either configured endpoint is
  valid; an out-of-range value never reaches the order adapter.
- The stable backend diagnostics `margin amount is below product minimum` and
  `margin amount exceeds product maximum`, including a validation-error prefix,
  map to localized race feedback. Keep the contract review open, reload current
  product limits, and preserve retry; unknown backend messages retain the normal
  API error contract.
- Only the contract branch uses the dedicated `.contract-order-confirm`
  review surface. The spot branch keeps its existing confirmation information
  architecture and `placeSpotOrder` payload. Both branches continue through
  the same validated review state rather than introducing a second mutation
  owner.
- The contract review reads its pair Logo and reference price from the current
  Market Store ticker, its mode and leverage from the selected product/user
  setting state, and its committed margin/order type/optional limit from the
  current form. On open it freezes reference price, order type, exact limit
  string, mode, leverage, margin, product, direction, and one idempotency key.
  Estimated
  notional is `marginAmount * leverage`; estimated opening quantity is that
  notional divided by the positive live reference price. It must not substitute
  available wallet balance, a Pencil sample, or another product.
- A rejected `placeMarginOrder` call leaves the review open and exposes the
  mapped API error inside its fixed action region. A retry invokes the same
  real mutation with the exact frozen order type/price and the same idempotency key;
  asynchronous setting/product refreshes must not rewrite the open review.
  The submitting guard blocks duplicate calls and every dismissal path until
  the in-flight call settles.
- At 390px, the production frame after removing the mock OS status bar keeps a
  58px Header, a 500px module, the exact `14 + 202 + 10 + 150 + 14` horizontal
  track, 490px console/book columns, six asks/seven bids, and a 44px position
  rail. The left console uses Pencil's `IpirH/mcfEf` absolute vertical tracks:
  margin mode and leverage share a 98/6/98 row, order type owns a separate
  202x40 row, price owns 138/6/58 at 54px, and margin owns 202x48. The
  continuous slider keeps one thumb inside a 44px accessible hit area. At
  448px only the book expands; at 320px the console and book contract without
  document overflow.
- The positions tab renders the count of the currently visible filled-position
  collection. Each position card keeps the Pencil action order `TP/SL`,
  `Close`, `Market close all`. The ordinary `Close` action opens the selected
  Pencil sheet and submits one frozen integer percentage plus idempotency key
  through `closeMarginPosition(position.id, intent)`. `Market close all`
  remains a separate 100% card shortcut through the legacy-compatible
  `closeMarginPosition(position.id)` call. Both actions affect only that card;
  card actions must never call the batch endpoint. The top `Close all` is the
  sole batch owner and calls
  `closeAllMarginPositions(currentPairOnly ? selectedProduct.id : undefined)`.
- Destructive confirmations are mutually exclusive. Arming a card action clears
  the top batch intent, arming the top batch action clears the card intent, and
  changing the current-pair scope clears both. Scope controls remain disabled
  while any close request is in flight so the confirmed target set cannot
  change during submission.
- TP/SL stays visible in its Pencil slot but is disabled with localized
  unavailable copy while the exact position product reports
  `takeProfitStopLossSupported=false`; the disabled control has no click or
  request handler. The three card actions are independent equal-width controls
  with a 10px gap, 12px radius, 44px hit area, 42px inset visual face, and no
  horizontal overflow at 320px in either theme.
- The directional-leverage, mode, order-type, and pair sheets are 840px, 446px,
  338px, and 620px high at their full reference sizes. Directional leverage
  pins its Header and 52px confirmation action while only the middle row
  scrolls; mode and pair keep their start-aligned content tracks. At 340px and
  below, wrapped notices use intrinsic height without overlapping an action.

### 4. Validation & Error Matrix

| Condition | Required behavior |
| --- | --- |
| Settings GET returns 404 | Keep product defaults; do not show an error |
| Settings GET fails otherwise | Keep safe product defaults and show localized feedback |
| Saved leverage no longer exists | Ignore it and keep a configured level |
| Saved margin mode no longer exists | Ignore it and keep a configured mode |
| No exact product for the route symbol | Disable settings/order actions; never fall back to another product |
| Product capability list is empty | Render no fabricated options and disable confirmation |
| Current order type disappears after capability refresh | Prefer advertised limit, otherwise first real capability; use `null` when none remain |
| Ticker fields are missing | Render fallback mark/`--`; do not use design samples |
| Spot order enters review | Keep the existing generic spot confirmation content and payload |
| Contract market order enters review | Show the current pair, direction, market semantics, setting values, committed margin, derived notional, and derived quantity |
| Contract limit order enters review | Show frozen limit and live-reference estimate; send `order_type=limit` plus exact frozen `price` |
| Market request is assembled | Send `order_type=market` and omit `price` entirely, even if a stale local limit input remains |
| Limit is empty, non-positive, non-decimal, or over pair precision | Mark/announce the localized field error; open no review and send no request |
| Margin amount is below `minMargin` or above positive `maxMargin` | Mark the field invalid, announce the localized boundary, and open no review or request |
| Product has no usable `maxMargin` | Display the minimum and no-product-maximum state; base shortcuts on real wallet available |
| Product limits change after review opens | Localize the known backend boundary failure, keep review/retry state, and reload current product limits |
| Contract submission fails | Keep the review open, show the mapped error inside it, and allow retry after busy clears |
| Contract submission is in flight | Ignore duplicate submission and keep overlay, close button, and Escape dismissal inactive |
| Risk copy wraps at 320px | Grow the notice/body; keep submit visible and non-overlapping |
| Position product does not advertise TP/SL | Keep the Pencil action visible and disabled; send no request |
| Card `Close` or `Market close all` is confirmed | Close only that position ID through the single-position endpoint |
| Sheet ratio changes | Update proportional preview only; send no request and do not mutate the authoritative position |
| Final sheet confirmation reaches its threshold | Submit the frozen position ID, integer percentage, and idempotency key exactly once |
| A partial-close request has an uncertain/failing response | Keep the same frozen ratio and key for retry; never generate a second settlement intent |
| Top `Close all` is confirmed while current-pair scope is on | Send the selected product ID to the batch endpoint |
| Top `Close all` is confirmed while current-pair scope is off | Omit `product_id` so the backend batch covers all visible account positions |
| Another destructive intent is already armed | Replace it with the newly selected intent; never display two active confirmations |
| A close request is in flight | Lock both scope controls and all other position mutations until it settles |

### 5. Good / Base / Bad Cases

- Good: open BTC contract, load its saved 10x cross setting, choose 20x from a
  configured sheet, PATCH successfully, then render 20x.
- Good: choose an advertised limit, fill long from best ask, open review, then
  receive newer tickers while both review and retry keep the exact frozen limit,
  reference, and idempotency key.
- Good: arm `Market close all` on one card, confirm it, and send exactly one
  `closeMarginPosition` request for that card ID while every sibling position
  remains untouched.
- Good: select 37%, slide the independent confirmation control, and submit
  `{ percentage: 37, idempotency_key: <frozen> }`; after success reconcile the
  remaining position and wallet from REST instead of subtracting locally.
- Base: a new user receives 404 for settings and continues with the product's
  first supported mode and configured leverage level.
- Base: TP/SL is not advertised, so its slot remains understandable but disabled
  while both supported close actions stay available.
- Bad: tapping the leverage field cycles local values without a sheet or PATCH.
- Bad: showing an order type not present in backend capabilities, using bid for
  long or ask for short, sending a market `price`, or rebuilding the request
  from live form/ticker state during retry.
- Bad: wiring the card-level `Market close all` label to
  `closeAllMarginPositions(productId)`, or retaining a card confirmation after
  the user changes the batch scope.
- Bad: treating the ratio rail as static decoration, deriving settlement from
  JavaScript floating-point amounts, or creating a new idempotency key after an
  uncertain response to the same frozen intent.

### 6. Tests Required

- Source/adapter tests lock the GET/PATCH paths, 404-only fallback, cross and
  isolated types, backend-only order capability parsing/fallback, pair precision,
  market payload price omission, and limit payload exact-price inclusion.
- Confirmation source tests lock the spot/contract branch boundary, ticker and
  form-derived values, `margin * leverage / referencePrice` quantity, unchanged
  `placeMarginOrder` input, in-panel failure, duplicate guard, and busy-state
  dismissal lock.
- Executable financial-boundary tests cover positive/null/invalid `max_margin`,
  wallet-below-cap and wallet-above-cap percentages, inclusive endpoints,
  below/above rejection, no-maximum behavior, shared review/request guards, and
  known backend minimum/maximum error classification.
- Executable order tests cover positive/invalid/precision limit values,
  trailing-zero precision, long-ask/short-bid/latest fallback, nullable-entry
  holding/order classification, and immutable review/retry requests.
- UI contract tests lock all eight Pencil IDs, 24px real asset mark, six book
  asks/seven bids, exact 390px geometry, two-row field typography, shell-owned
  focus ring, 12px slider faces inside 44px targets, sheet tracks, localized
  copy, dialog semantics, safe area, and reduced motion.
- Position-action source tests lock the visible-count tab, exact Pencil action
  order, per-position capability lookup, independent card intents, one
  single-position close call, the conditional batch product ID, mutually
  exclusive confirmations, scope-lock behavior, and the absence of a TP/SL
  request handler. CSS contracts lock 10px gaps, 12px radii, 44px hit/row
  geometry, 42px visual faces, both theme token sets, and 320px no-overflow.
- Browser checks cover light/dark 390x920 main and all four sheets, then
  320x760 horizontal overflow, wrapped notice, focus trap, Escape dismissal,
  body scroll lock, trigger focus restoration, guest order-type opening,
  neutral pair-search initial state, editable/read-only price states, BBO fill,
  complete field-shell focus rings, and frozen market/limit confirmation details.

### 7. Wrong vs Correct

#### Wrong

```ts
leverage.value = nextLevel
marginMode.value = 'isolated'
router.push('/markets?mode=contract')

// Wrong: a card-level action must not close sibling positions.
await closeAllMarginPositions(position.productId)
```

#### Correct

```ts
await updateMarginLeverage(product.id, nextLevel)
leverage.value = nextLevel

await updateMarginMode(product.id, nextMode)
marginMode.value = nextMode

router.replace({
  name: 'trade',
  params: { symbol: selectedSymbol.replace('/', '_') },
  query: { mode: 'contract' },
})

// Card action: close one position only.
await closeMarginPosition(position.id)

// Top batch action: preserve the user's current-pair scope.
await closeAllMarginPositions(currentPairOnly.value ? selectedProduct.value?.id : undefined)
```

## 14. Agent-Routed Online Support Contract

### 1. Scope / Trigger

Apply this contract when changing the mobile help entry, first-party support
chat, support adapter, polling lifecycle, unread state, or message pagination.
The mobile client never chooses an agent; it only renders the backend's current
assignment snapshot.

### 2. Signatures

```ts
fetchCurrentSupportConversation(): Promise<{
  conversation: SupportConversation | null
}>

fetchSupportConversationMessages(options?: {
  limit?: number
  beforeId?: number
}): Promise<{
  messages: SupportMessage[]
  has_more: boolean
  next_before_id: number | null
}>

postSupportConversationMessage(input: {
  body: string
  clientMessageId: string
}): Promise<{
  conversation: SupportConversation
  message: SupportMessage
  replayed: boolean
}>
```

Production paths are `/support/conversation`,
`/support/conversation/messages`, `/support/conversation/read`, and
`/support/conversation/status` through the authenticated user API client.

### 3. Contracts

- `/profile/help` opens the internal `/profile/help/chat` route. The retired
  `VITE_SUPPORT_CHAT_URL` path must not return.
- Guests render `LoginRequiredState` and make no protected support request.
  Token replacement, logout, and unmount invalidate earlier conversation,
  message, read, status, and send responses before they can mutate the new
  session.
- The backend owns `assigned_agent_id`/`assigned_agent_code`. Null assignment
  is a usable platform-admin fallback state, not a disabled chat channel.
- Initial load reads the conversation and newest bounded message page. The
  “load older” action follows `next_before_id`, merges by immutable message ID,
  keeps chronological order, and exposes distinct loading/error/retry states.
- Poll every five seconds while an authenticated chat is mounted. Polling is
  single-flight, stops on unmount/logout, merges rather than replaces older
  loaded history, and treats REST as authoritative after process restart or a
  missed WebSocket hint.
- A send attempt trims the body, validates at most 2,000 Unicode scalar values,
  creates one 8-64 character safe `client_message_id`, and reuses that exact
  attempt on retry. A successful response is merged immediately, then REST is
  reconciled.
- Rendered agent/admin messages advance the user read cursor monotonically.
  User messages display the backend `read_by_recipient` snapshot when present.
- A closed conversation remains visible with history. Explicit reopen works,
  and sending a valid new customer message may reopen it per backend contract.
- Chinese and English locales cover loading, empty, cached refresh error,
  older-history loading/error, sending/retry, read/unread, assignment,
  unassigned, closed, and reopen states. Controls use Lucide icons only.

### 4. Validation & Error Matrix

| Condition | Required behavior |
| --- | --- |
| Guest opens chat | Render login-required state; issue no support API request |
| Conversation is null | Render first-message empty state and assignment pending copy |
| Assignment is null | Keep composer usable and explain platform support fallback |
| Body is blank or over 2,000 scalars | Show localized validation; make no request |
| Send fails | Keep immutable pending attempt and reuse its client ID on retry |
| Newest-page refresh fails with cached messages | Keep history visible and show retryable refresh notice |
| Older-page request fails | Keep all loaded messages and expose an older-page retry only |
| Poll response repeats IDs | Deduplicate by message ID and retain chronological order |
| Token changes while a request is in flight | Ignore the stale result and reset chat state |

### 5. Good / Base / Bad Cases

- Good: the user sends once, loses the response, retries the same attempt, and
  receives the existing backend message without a duplicate bubble.
- Good: after more than 100 messages, the user loads an older page without
  losing the newest page or jumping to the thread bottom.
- Base: the user has no assigned agent; the same composer sends successfully
  and administrators can answer.
- Bad: a local agent selector or environment URL bypasses the backend referral
  assignment.
- Bad: each poll replaces the array with the newest page and silently deletes
  older history already loaded by the user.

### 6. Tests Required

- Adapter tests lock exact paths/body field names, page query names, envelopes,
  Unix-millisecond timestamps, and nullable assignment.
- Core tests cover scalar validation, safe client IDs, retry reuse, immutable
  merge/order, page-cursor progression, grouping, and latest staff-read target.
- View/source tests cover guest no-request behavior, internal navigation,
  loading/empty/error/retry, unassigned/assigned/closed copy, load older,
  single-flight polling cleanup, session invalidation, and i18n symmetry.
- Browser checks cover light/dark 390x844 and 320x720 with no horizontal
  overflow, a visible sticky composer above safe-area inset, 44px actions,
  keyboard/IME Enter behavior, and preserved scroll position after older-page
  insertion.

### 7. Wrong vs Correct

```ts
// Wrong: a retry generates another key and can append a duplicate message.
await send(body, createSupportClientMessageId())
await send(body, createSupportClientMessageId())

// Correct: freeze one attempt and replay the same identity until resolved.
const attempt = createSupportSendAttempt(body)
await executeSupportSendAttempt(attempt, send)
await executeSupportSendAttempt(attempt, send)
```

## 15. Margin Private Socket and Account Reconciliation Contract

### 1. Scope / Trigger

Apply this contract when a margin account can change outside the mounted
`TradeView`, especially after an asynchronous liquidation worker settles a
position. It covers the mobile user's private WebSocket, the five-second REST
fallback, wallet/position/risk reconciliation, and request-lifecycle guards.

### 2. Signatures

```ts
resolvePrivateUserWebSocketUrl(
  config: BackendRuntimeConfig,
  accessToken: string,
  pageOrigin?: string,
): string | null

createPrivateUserStream(options: {
  getAccessToken(): string
  getUrl(accessToken: string): string | null
  onOpen?(): void
  onEvent(event: { type: string; [key: string]: unknown }): void
}): { start(): boolean; stop(): void; isRunning(): boolean }

createMarginAccountReconciliationLifecycle(options): {
  refreshForeground(): Promise<MarginAccountReconciliationResult>
  refreshBackground(options?: { queueIfBusy?: boolean }): Promise<MarginAccountReconciliationResult>
  invalidate(): void
  startPolling(): void
  stop(): void
}
```

Wire and REST endpoints:

```text
GET /api/v1/ws/private?token=<encoded-current-access-token>
event -> {"type":"margin.position.liquidated", ...notificationContext}
GET /api/v1/margin/wallets
GET /api/v1/margin/positions/{position_id}/risk
```

### 3. Contracts

- Start one private stream only while `TradeView` is mounted, the user is
  authenticated, and the route is in contract mode. Stop it on logout, token
  replacement, spot mode, or unmount; a restarted stream must read the latest
  persisted access token and URL-encode it.
- The server binds `private:user:<user_id>` during the authenticated handshake.
  Mobile sends no subscribe command; it sends only heartbeat `ping` frames.
  Current-socket identity guards prevent late open/message/close handlers from
  an old connection affecting the replacement connection.
- Initial open, reconnect, page visibility restoration, and
  `margin.position.liquidated` request a silent REST reconciliation. The event
  payload itself never adds/subtracts balances or removes a position.
- `GET /margin/wallets` is the account authority for both wallet rows and the
  current `opened` position list. Commit those two arrays from the same response,
  prune risk snapshots to eligible surviving IDs, then fetch risk only for
  surviving filled positions whose product supports position risk.
- Keep a five-second, visible-page, single-flight background reconciliation so
  a dropped hint, failed socket, or restarted API still converges. Repeated
  hints while a request is busy coalesce into at most one following refresh.
- Foreground loads and post-mutation refreshes preserve loading/error feedback
  and may supersede an older poll. Background errors keep the last successful
  account and do not flash first-load loading/error UI; a later cycle retries.
- Every request binds its generation, access-token session, contract mode,
  visibility requirement for background work, and mounted lifecycle. Logout,
  account replacement, contract/spot ABA, hide/invalidate, or unmount makes an
  older result stale and unable to commit.

### 4. Validation & Error Matrix

| Condition | Required behavior |
| --- | --- |
| Empty access token | Return no private URL; do not create a socket or margin request |
| Guest or spot route | Keep private stream stopped and skip background reconciliation |
| Text/JSON pong, confirmation, error, unknown, or malformed frame | Ignore as a business event |
| Matching liquidation event during an active request | Queue one follow-up REST reconciliation |
| Socket closes or errors | Clear heartbeat and reconnect with bounded exponential backoff |
| Access token changes before reconnect | Build the reconnect URL from the latest persisted token |
| Background `/margin/wallets` fails | Retain the last successful wallets, positions, risks, and quiet UI |
| Risk read fails for one surviving position | Keep its last successful risk; apply other fulfilled risks |
| Successful account snapshot omits a former position | Remove that position and its cached risk |
| Old request returns after token/mode/lifecycle change | Return `stale`; commit nothing |

### 5. Good / Base / Bad Cases

- Good: a private liquidation hint arrives, one silent `/margin/wallets` read
  replaces wallet and opened positions, and the removed position's risk cache
  disappears without re-entering the page.
- Good: the socket misses the event during an API restart; the next visible
  five-second poll reaches the same authoritative result.
- Base: the socket opens with no account change; the immediate reconciliation
  is idempotent and leaves the rendered account unchanged.
- Bad: delete the position directly from event fields, increment the wallet by
  `payout_amount`, poll only `/positions/{id}/risk`, overlap unlimited timers,
  or let a late response from another token restore stale account data.

### 6. Tests Required

- URL tests assert same-origin/remote `ws`/`wss`, API prefix, token encoding,
  and null for a blank token.
- Transport tests assert no-token skip, no subscribe frame, heartbeat,
  protocol-frame filtering, latest-token reconnect, bounded backoff, current
  socket identity, and idempotent timer/socket cleanup.
- Lifecycle tests assert five-second scheduling, background single-flight,
  busy-hint coalescing, hidden/guest/spot/inactive skips, foreground
  supersession, recovery after transient failure, and token/mode/unmount stale
  protection.
- TradeView tests assert the exact liquidation discriminator is a REST hint,
  `/margin/wallets` commits wallets before risk reads, only opened/filled/
  supported IDs receive risk reads, removed IDs are pruned, and explicit
  mutations still call foreground reconciliation.

### 7. Wrong vs Correct

```ts
// Wrong: risk polling cannot discover the authoritative active-position set.
setInterval(() => loadMarginPositionRisks(localPositions.value), 5_000)

// Correct: the private event is an accelerator and REST remains authoritative.
privateStream.onEvent((event) => {
  if (event.type === 'margin.position.liquidated') {
    void accountLifecycle.refreshBackground({ queueIfBusy: true })
  }
})
accountLifecycle.startPolling()
```

## 16. Public Market Socket Silence Recovery Contract

### 1. Scope / Trigger

Apply this contract to the shared multi-symbol ticker stream and the dedicated
market-detail depth/trade/K-line stream. Browser WebSockets can remain `OPEN`
after a proxy, NAT, radio transition, or suspended PWA loses the actual data
path, so close/error events alone are not a complete recovery signal.

### 2. Signatures

```ts
interface MarketTickerStreamOptions {
  heartbeatMs?: number
  inboundIdleTimeoutMs?: number
}

interface MarketDetailStreamOptions {
  heartbeatMs?: number
  inboundIdleTimeoutMs?: number
}
```

Defaults:

```text
client text ping interval = 25_000 ms
inbound idle timeout = 65_000 ms
reconnect delay = existing bounded exponential backoff
```

### 3. Contracts

- Arm one inbound-silence watchdog only after the current socket opens and its
  subscription commands are sent successfully.
- Refresh the watchdog before parsing every inbound frame. Text/JSON pong,
  subscription confirmation, backend error, unknown, malformed, and valid
  market frames all prove that the transport remains readable; only valid
  market frames may mutate business state.
- Sending `ping` does not refresh the watchdog. If no frame returns before the
  timeout, close the exact current socket, clear its heartbeat/render work,
  schedule the existing reconnect, and resend the current authoritative
  subscription set after open.
- A re-armed watchdog invalidates already-queued callbacks through a generation
  token. A stale timeout or an event from an old socket must not close, clear
  timers for, or dispatch through a newer connection.
- Releasing the final ticker lease and stopping a detail stream clear heartbeat,
  watchdog, reconnect, and pending animation-frame work idempotently.
- The watchdog is transport recovery only. It does not fabricate prices,
  advance candles, merge a stale REST row, or alter ticker newest-observation
  authority.

### 4. Validation & Error Matrix

| Condition | Required behavior |
| --- | --- |
| Current socket receives any frame | Re-arm the 65-second watchdog before protocol parsing |
| Current socket stays `OPEN` but silent for 65 seconds | Close it and schedule bounded reconnect |
| A valid pong arrives after a ping | Refresh liveness; mutate no ticker/depth/trade/K-line state |
| Superseded watchdog callback runs late | Ignore it through generation mismatch |
| Old socket emits message/close/error after replacement | Ignore it through current-socket identity checks |
| Ticker reconnects with multiple active leases | Subscribe to the exact union of current normalized symbols |
| Detail reconnects | Restore the selected depth/trade/K-line channels, symbol, and interval |
| Final lease or stream stop | Leave no timeout, interval, reconnect, or render callback |

### 5. Good / Base / Bad Cases

- Good: a mobile radio transition leaves the socket `OPEN` but silent; the
  watchdog closes it, reconnects, and restores BTC and ETH ticker leases.
- Good: an illiquid market returns only backend `pong` frames; transport stays
  healthy without inventing a market update.
- Base: normal ticker/depth/trade/K-line traffic continuously refreshes the
  watchdog while existing render coalescing remains unchanged.
- Bad: reset the deadline when sending `ping`, depend on `readyState === OPEN`,
  or let page components implement independent reconnect timers.

### 6. Tests Required

- Ticker transport tests simulate an open silent socket, assert close/reconnect,
  and assert every active lease is re-subscribed.
- Ticker tests retain an old watchdog callback, receive pong, then prove the old
  callback cannot close the refreshed socket.
- Detail transport tests simulate the same silence and assert depth, trade, and
  the selected K-line interval are all restored.
- Existing close/error, exponential backoff, old-socket, stop, render coalescing,
  REST race, and session-generation tests must remain green.

### 7. Wrong vs Correct

```ts
// Wrong: send success does not prove that the peer or reverse path is alive.
setInterval(() => socket.send('ping'), 25_000)

// Correct: outbound heartbeat and inbound proof have independent timers.
heartbeatTimer = setInterval(() => socket.send('ping'), 25_000)
inboundWatchdog.arm(() => closeAndReconnectCurrentSocket())
socket.addEventListener('message', (event) => {
  inboundWatchdog.arm(() => closeAndReconnectCurrentSocket())
  dispatchOnlyValidatedMarketFrame(event.data)
})
```

## 17. Mobile Margin Partial-Close Confirmation Contract

```text
POST /api/v1/margin/positions/:id/close
explicit request body = { percentage: 1..100, idempotency_key: string }
legacy full-close body = {}
response authority = refreshed GET /api/v1/margin/wallets plus supported risk snapshots
```

- The sheet ratio is an independent native range from 1% through 100%, defaulting
  to 100%. It changes only quantity/PnL previews until the final confirmation;
  price, quantity, client-computed settlement, and preview amounts are never sent.
- Opening or dragging below the UI confirmation threshold is local-only and
  sends no HTTP request. Crossing the bottom confirmation threshold freezes the
  current position id, integer percentage, and idempotency key, then calls
  `closeMarginPosition(position.id, intent)` exactly once; an in-flight guard
  rejects additional pointer, keyboard, or card actions.
- Mark price, position quantity, and estimated PnL shown before confirmation
  come from the server risk snapshot. The current ticker may fill only a
  missing positive mark-price display; it does not become a settlement input.
- A successful mutation triggers foreground wallet/position reconciliation.
  The sheet closes after success even if a private position event arrives
  first. A failed or uncertain mutation keeps the sheet open, displays the
  normalized API error, resets only the confirmation slider, and reuses the
  frozen percentage and idempotency key for an explicit retry.
- The backend allocates the percentage from the row-locked remaining exposure,
  persists one immutable close execution, and commits wallet delta, ledger,
  position remainder/terminal state, and cross-account version together. Same
  key and request replays without another financial mutation; same key with a
  different position or percentage conflicts.
- The existing card-level Market close all action remains a separate explicit
  two-step 100% shortcut using the legacy empty request. The workspace-level
  Close all action remains the only batch endpoint consumer.

## 18. In-Memory Reference Request Deduplication Contract

```ts
interface ReferenceRequestOptions {
  force?: boolean
}

interface MemoryRequestRegistry {
  request<T>(
    key: string,
    ttlMs: number,
    loader: () => Promise<T>,
    options?: ReferenceRequestOptions,
  ): Promise<T>
  invalidate(key?: string): void
}
```

- The registry is process-memory-only. It must not write API responses to
  localStorage, IndexedDB, the service worker, or a cross-session browser cache.
- TTL begins when a loader succeeds. Errors and cancellations never populate
  the cache. Equal keys share one in-flight promise; every caller receives an
  isolated clone so one view cannot mutate another view's cached DTO.
- `force` bypasses a completed cache value while still sharing the same current
  in-flight request. Key or global invalidation must prevent an already-running
  stale loader from repopulating the cache after it resolves.
- Cache keys contain the resolved API URL plus stable, sorted parameters. Add
  locale when mapping depends on localized fallback copy. Wallet directories
  additionally contain the current access-token scope because regional/account
  policy can change the available assets and networks.
- Whitelist only slow-changing reference/catalog calls with explicit TTLs at
  their call sites: countries, public auth configuration, market-pair metadata,
  convert pairs, margin/seconds/earn/loan product catalogs, prediction config,
  new-coin project catalogs, and wallet deposit/withdraw directories.
- Never cache wallet balances, Today Return, KYC state, orders, positions,
  ledger pages, market tickers, candles, depth, trades, one-time quotes, private
  profile/security state, or any POST/PATCH/DELETE mutation. Those calls remain
  network-authoritative on every required reconciliation.
- A route remount within TTL must reuse the whitelisted result; explicit manual
  refresh or a business mutation that changes a catalog may pass `force` or
  invalidate the exact key before authoritative reload.

Required tests cover successful TTL timing, expired reload, concurrent
single-flight behavior, failure behavior, cloning, force, key/global
invalidation, stable parameter ordering, whitelist presence, and strong-data
exclusions. A browser route-away/route-back check must prove a stable endpoint
is requested once within its TTL.

## 19. Shared Session, Request, Realtime, and Decimal Execution Contract

### 1. Scope / Trigger

- Trigger: changing mobile authentication, API transport, public/private live
  data, order refresh, or any calculation that can reach a financial mutation.
- This contract prevents late refresh responses from restoring logged-out
  sessions, route remounts from multiplying sockets/requests, and IEEE-754
  rounding from changing an order intent.

### 2. Signatures

```ts
createApiHttpClient(config?: CreateAxiosDefaults): AxiosInstance
composeAbortSignals(signals: Array<AbortSignal | null | undefined>, timeoutMs: number): ComposedAbortSignal

createSessionOwner(options?: SessionOwnerOptions): SessionOwner
SessionOwner.capture(): SessionLease
SessionOwner.commitRefresh(lease, tokens): SessionSnapshot | null

createSharedMarketLifecycle(options: SharedMarketLifecycleOptions): SharedMarketLifecycle
SharedMarketLifecycle.refresh(force?: boolean): Promise<void>
SharedMarketLifecycle.acquire(consumerId: string): void

createPrivateUserStreamManager(options: PrivateUserStreamManagerOptions): PrivateUserStreamManager
PrivateUserStreamManager.acquire(options: PrivateUserTopicLeaseOptions): PrivateUserTopicLease

createSpotOrderReviewSnapshot(input: SpotOrderReviewInput): SpotOrderReviewSnapshot
createSecondsOrderReviewSnapshot(input: SecondsOrderReviewInput): SecondsOrderReviewSnapshot
```

### 3. Contracts

- HTTP defaults to a 12-second deadline and composes caller cancellation with
  timeout cancellation. Error presentation is code-first and must not expose
  raw server diagnostics as user copy.
- `PersistedSessionEnvelope.version` is `1`; every login, refresh, logout, and
  external transition advances `epoch`/`revision`. `scope` is an opaque random
  identity boundary and never contains an access token. A refresh commits only
  when its captured `scope + epoch + accessToken` still matches; logout wins.
- Public market REST refresh is single-flight and TTL-aware. Views acquire a
  stable consumer lease; one shared connection owns `idle | connecting | live |
  stale | offline`, `lastFrameAt`, silence detection, and reconnect lifecycle.
- Margin and support lease topics on one private connection per authenticated
  generation. Topic release cannot close another topic's lease. Private events
  invalidate/reconcile REST-owned account state; they do not directly invent a
  wallet balance or settlement.
- Price, quantity, amount, fee, rate, percentage-derived value, notional, PnL,
  and mutation payload values remain canonical decimal text. A compatibility
  `number` passed to a display-only component is a terminal one-way conversion
  and must never feed validation, branching, review snapshots, or payloads.
- Visible financial precision is a presentation boundary, not the storage or
  mutation precision. `USDT`, `USDC`, common fiat-like amounts use at most two
  fraction digits; other asset quantities use at most eight and may be
  tightened by a lower authoritative asset precision. Formatting performs
  deterministic decimal half-up rounding without `Number`. A non-zero value
  below the smallest visible unit renders a threshold such as `<0.01` or
  `>-0.00000001`, never a false zero. Rendered text must not flow back into a
  request, review snapshot, comparison, ledger row, or calculation.
- Order loaders capture request/session generations and commit only the latest
  authoritative response. Batch cancel uses the batch endpoint and preserves
  per-order failures instead of reporting false all-or-nothing success.

### 4. Validation & Error Matrix

| Condition | Required behavior |
|-----------|-------------------|
| Caller aborts or 12-second deadline expires | Abort transport; classify without a second mutation |
| Refresh returns after logout/new login | CAS rejects it; current session and caches stay unchanged |
| Persistence is unavailable | Keep a memory session and expose `persistence: memory` |
| Public/private stream is silent | Mark `stale`, reconnect with bounded backoff, retain last valid REST data |
| Session generation changes | Abort stale requests, close old private transport, invalidate scoped caches |
| Decimal text is missing/invalid/non-positive | Reject locally with typed state; send no mutation |
| Older order response arrives last | Discard it; do not replace the current list |
| Batch cancel partially fails | Return successful and failed IDs independently, then reconcile |

### 5. Good / Base / Bad Cases

- Good: two views join during one market cold start, share the same promise and
  socket, then one leaves without interrupting the other.
- Good: a refresh is in flight when another tab logs out; the persisted
  tombstone is applied before CAS and the response is discarded.
- Base: a display widget receives `legacyTradeDisplayNumber(decimal)` after all
  financial decisions are frozen as decimal text.
- Bad: `Number(amount)`, `parseFloat(price)`, or `toFixed()` contributes to an
  order body, balance check, fee, return, or PnL decision.

### 6. Tests Required

- Session tests assert logout-wins-refresh, external generation ordering,
  storage failure fallback, scoped cache invalidation, and abort propagation.
- Market/private-stream tests assert single-flight/ref-counting, topic
  isolation, silence watchdog, backoff, online/offline transitions, and cleanup.
- Trade/seconds behavior tests use more precision than JavaScript safely
  represents and assert exact review/payload strings and boundary decisions.
- Order tests assert stale-response suppression and mixed batch results.
- Required gate: `npm --prefix mobile run release:gate`.

### 7. Wrong vs Correct

```ts
// Wrong: late refresh can resurrect a session and the amount is rounded.
const amount = Number(form.amount)
session.accessToken = (await refresh()).accessToken
await createOrder({ amount })

// Correct: freeze decimal intent and commit refresh only against its lease.
const review = createSpotOrderReviewSnapshot({ ...input, quantity: form.amount })
const lease = sessionOwner.capture()
sessionOwner.commitRefresh(lease, await refresh(lease.signal))
await createOrder({ quantity: review.quantity })
```

## 20. Mobile Transaction Records Read Model

### 1. Scope / Trigger

- Apply this contract when changing the `/orders` + `/assets/ledger`
  transaction-record workspace (user-facing name: `交易记录` /
  `Transaction Records`), its record geometry, three ledger filters, ledger
  mapping, precision-aware formatting, asset logos, or infinite pagination.
- Frames `kcP5D/A85if` remain the declared source for the 58px Header, four-tab
  navigation, filter bars, and flush divider-only record rows. Pencil's
  exported `h-[…]` values are content heights under `box-sizing: content-box`,
  not visible outer row heights. Frames `y6Y7TW/m25xr0` remain obsolete.

### 2. Signatures

```ts
type WalletLedgerDirection = 'all' | 'credit' | 'debit'
type WalletLedgerDatePreset = 'all' | 'today' | 'last7Days' | 'last30Days'

interface WalletLedgerFetchOptions {
  limit: number
  offset: number
  assetSymbol?: string
  direction: WalletLedgerDirection
  startTime?: string
  endTime?: string
}

interface WalletLedgerEntry {
  symbol: string
  precisionScale: number // required integer, 0..18
  amount: DecimalText
  balanceAfter: DecimalText
  fee: DecimalText // authoritative non-negative fee
  createdAt: number
}

createWalletLedgerPaginationController(input): WalletLedgerPaginationController
createWalletLedgerAssetDirectoryRequestLifecycle(input): WalletLedgerAssetDirectoryRequestLifecycle
formatWalletLedgerDecimal(value, locale, precisionScale, assetSymbol?): string
```

- Transport names are `asset_symbol`, `direction`, `start_time`, `end_time`,
  `limit`, and `offset`. UTC boundaries are emitted as MySQL-safe
  `YYYY-MM-DD HH:mm:ss.SSS` text.

### 3. Contracts

- The route renders a 58px header (`16px` horizontal padding,
  `26px minmax(0,1fr) 26px` tracks), a 52px four-tab record navigation, a
  58px filter bar, then the full-width record canvas. The app does not
  draw Pencil's 28px operating-system status bar; `.page` owns the safe-area
  inset. The sticky Header's white/black chrome must cover that dynamic inset;
  the record-canvas color must not leak into the status-area band, and no fixed
  mock status-bar height may replace `env(safe-area-inset-top)`.
- Visible filters are Currency, Transaction type, and a 24px ListFilter date
  trigger. Their sheets retain authenticated asset, direction, and date server
  filters; default date sheet/ARIA copy is All dates, not `Date: Date`.
- Production rows retain `border-box` and responsive width to prevent
  horizontal overflow. Their minimum outer advances are calculated from the
  exported content-box nodes as `content height + vertical padding + 1px
  border - 0.5px overlap`: current order 238px, historical spot order 174px,
  historical margin order 214px, current position 334px, margin asset 228px,
  historical position 398px, ledger row 190px, and associated execution 218px.
  Never copy Pencil's fixed 390px content width plus horizontal padding.
- Three-column metric labels remain one line at 320px with a local
  `min-width: 0`/ellipsis boundary and an exact `title`; a long translated
  label must not wrap and increase the converted outer advance. Values use the
  same local shrink boundary and retain their complete text in `title`/ARIA.
- Current-position maintenance margin rate consumes only
  `risk.maintenance_margin_rate`. `risk.margin_ratio` is equity divided by
  maintenance margin and must never be relabelled as that rate. Historical
  average close price is weighted from immutable execution notional/exit-price
  slices plus a real legacy terminal residual; if execution-history loading
  fails, execution-derived quantity, return, and average fields render `--`
  instead of treating the terminal residual row as the whole position.
- Each current-position direction/mode/leverage chip owns the semantic
  `.margin-position-record__chip` element. It is content-width `inline-flex`,
  centered on both axes, single-line, and locally shrink-bounded; its exact
  Pencil face is `4px 7px` padding, `6px` radius, `13px` Noto Sans SC text at
  weight `650`. The wrapping 7px-gap row and per-chip `max-width: 100%` must
  retain zero horizontal overflow from 320px through 448px without a broad
  descendant `span` selector.
- The current-position card may replace the server fallback tuple
  `(mark_price, unrealized_pnl, return_rate)` only when the already-shared
  matching ticker provides an exact positive `lastPriceText`, both observations
  are positive safe millisecond timestamps, and the ticker observation is not
  older than `risk.observed_at`. Calculate the three live fields as one coherent
  projection with `DecimalText` only: `pnl = notional * directional(mark -
  entry) / entry`, then `return = pnl / margin`, truncating each division to the
  backend's 18-decimal scale. Long uses `mark - entry`; short uses `entry -
  mark`. Missing risk, stale/missing ticker time, absent/invalid exact ticker
  text, non-positive mark or entry, or an invalid/non-positive margin returns
  the server tuple unchanged. Numeric ticker compatibility fields never enter
  this calculation.
- This live display projection reuses the route's existing shared market ticker
  lease and Vue dependency tracking. A ticker frame issues no REST request and
  does not project maintenance margin rate, margin ratio, liquidation price,
  account equity, close-sheet settlement inputs, or any backend liquidation
  decision.
- Margin wallet buckets directly authorize Balance, Available, and Frozen.
  Cross-account `equity` authorizes Currency equity only when that object is
  present. Until the backend defines portfolio `occupied` and isolated equity,
  neither wallet `locked` nor total balance may be relabelled as those fields;
  render `--` rather than client-computing a product definition.
- The ledger list has no horizontal gutter or inter-row gap. Every record remains a
  semantic full-width `article[role=listitem]` with square corners, no shadow,
  and only a one-pixel bottom divider. Scrolling rows never use
  `backdrop-filter` or floating-card decoration.
- A ledger row uses the exact 190px minimum outer height and may grow only when
  the <=340px footer wraps for accessibility. Its structure is an asset/signed-total
  header, a two-column detail grid (item/quantity and account plus
  direction/meta/fee), and a time/balance footer grid. Every data cell uses a
  `minmax(0, 1fr)` shrink boundary rather than sharing one crowded flex row.
  The row itself must explicitly reset the legacy global `.ledger-list article`
  layout with a single `minmax(0, 1fr)` track and
  stretch alignment so content width cannot vary with record length.
- Light uses white Header/filter/row canvas, ink `#111714`, row muted
  `#8A948F`, and tab muted `#7B8680`. Dark uses black chrome/row canvas, ink
  `#F3F7F5`, and muted
  `#8F9B94`. Active is `#18D38D`; negative is `#FF5878`; positive is
  `#0DBE7B` light and `#45EFAE` dark. Do not restore the retired `#0b1811` /
  `rgba(11, 24, 17, ...)` family.
- Asset options and row logos come only from the authenticated wallet directory.
  Directory requests are latest-wins and must match both exact session token and
  session generation before symbols or logo URLs enter view state.
- Asset, direction, and date selections are server filters, never filters over
  the currently loaded page. Every change invalidates the previous generation,
  clears rows, and reloads offset zero.
- Initial and append errors are separate. Append retry reuses the failed raw
  offset; next offset advances by response row count before deduplication.
  Identity is `accountType:id`, and an empty response exhausts pagination.
- `precision_scale` is required and maps only from an integer in `0..18`. It
  remains the authoritative storage/input limit but is not a command to expose
  all 18 digits in the UI. Ledger rendering applies the shared asset display
  cap after validation while preserving the exact `DecimalText` in the model
  and exact-value title/ARIA fields. No `Number`, `parseFloat`, or `toFixed` is
  allowed in the financial path. Total retains the authoritative signed net
  account delta. The current API has no gross execution quantity, so the
  Pencil Quantity field and its title remain `--`; the absolute net delta must
  not be relabelled as a fill quantity.
- The API has no pair or buy/sell side. Row two therefore shows the localized
  real change type; row three shows account type and amount-derived Income /
  Expense. Non-zero, non-negative API fee is presented as a DecimalText debit.
- At 390px and 448px, footer time starts at the fixed left row inset while the
  balance stays right-aligned in its own grid cell. At 340px and below, the
  footer becomes two rows so time and balance cannot squeeze one another.
  Amount, quantity, fee, time, and balance retain mono/tabular presentation;
  each may ellipsize only inside its own cell while exact `title` and row ARIA
  text retain the complete value.
- Filter sheets are labelled modal dialogs with focus trap, Escape/overlay
  close, focus restoration, body scroll lock, and at least 44px touch targets.
  The 320px, 390px, and 448px layouts must not create horizontal overflow.

### 4. Validation & Error Matrix

| Condition | Required behavior |
| --- | --- |
| Missing/invalid `precision_scale` | Throw `WalletLedgerContractError`; render localized initial/append error |
| Negative API fee | Reject as a contract error; do not double-negate malformed data |
| Invalid direction/date/asset/time range | Reject before transport; send no malformed request |
| Guest or logout during request | Clear protected rows and classify result as guest/stale |
| Older ledger or logo-directory response arrives last | Discard it without changing current rows, logos, errors, or loading state |
| Initial page fails | Show retryable full-page error and no stale rows |
| Append page fails | Keep existing rows and failed offset; show append retry |
| Empty first page | Show localized empty state and mark pagination exhausted |
| Response total/page metadata is inconsistent | Reject as a contract error |

### 5. Good / Base / Bad Cases

- Good: select BTC, Income, and Last 7 days; offset zero reloads with all three
  server predicates, later pages retain them, and the latest authenticated BTC
  logo/precision control the row.
- Base: all filters are `all`; the filter bar says Currency / Transaction type,
  the date sheet says All dates, and full-width divider rows render without date
  group headings.
- Good: an expense amount `-1.25` renders signed total `-1.25`, Quantity `--`,
  Expense, and a known non-zero `0.01` fee as `-0.01`.
- Bad: treat Pencil's 165.5px ledger content height as its outer height, restore
  floating cards/gutters, place all details in one flex line, invent a
  trading pair/side, use a stale wallet-directory logo, format through
  IEEE-754, or filter only loaded rows.

### 6. Tests Required

- Adapter tests assert exact query names/time encoding, strict
  `precision_scale`, non-negative fee, page metadata, and DecimalText retention.
- Lifecycle tests assert ledger filter/session ABA isolation and wallet-directory
  out-of-order/token/generation/logout/unmount isolation, plus error separation,
  exact-offset retry, row-count offset progression, and exhaustion.
- View/source tests assert `kcP5D/A85if`, unchanged Header/tab/filter geometry,
  dynamic safe-area chrome, the exact converted outer row advances, full-width
  divider rows, two-column detail/footer grids, the <=340px two-row footer,
  light/dark row tokens, valid routes, localized title/default filter copy,
  exact titles, 44px modal interaction, real logo use, explicit reset of the
  legacy `.ledger-list article` declarations, and no forbidden number
  conversion or backdrop filter (including prefixed forms).
- Browser verification covers 320px, 390px, and 448px in light/dark themes,
  exact row advances, stable footer alignment, modal focus/scroll behavior,
  title centering, and zero horizontal overflow.
- Required gate: `npm --prefix mobile run release:gate`.

### 7. Wrong vs Correct

#### Wrong

```ts
const amount = Number(entry.amount).toFixed(8)
const pair = `${entry.symbol}/USDT` // API never supplied a pair
walletAssetLogoUrls.value = staleDirectory.logoUrls
```

#### Correct

```ts
const amount = formatWalletLedgerDecimal(
  entry.amount,
  locale,
  entry.precisionScale,
  entry.symbol,
)
const quantity = '--' // net account delta is not a gross execution quantity
const directory = await directoryLifecycle.load()
if (directory.state === 'loaded') {
  walletAssetLogoUrls.value = directory.value.logoUrls
}
```
