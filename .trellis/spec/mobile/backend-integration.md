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
- Seconds reconciliation requests are generation-isolated. An older list or
  wallet response must not overwrite state produced by a newer open/reconcile
  cycle. Keep locally committed create responses until an authoritative list
  contains the same ID; when it does, the server row wins so settlement status
  can advance. Public products, ticker fallback, and K-lines start independently
  of private order/wallet refresh, so one protected endpoint failure does not
  hide otherwise available public market data.
- The dedicated Seconds history page reuses `fetchSecondsOrders(100)` and the
  shared `isActiveSecondsOrder` boundary after DTO mapping. It renders only
  non-active rows, preserves unknown result/status source values, and reads
  entry and settlement prices only from their optional API fields; a missing or
  invalid price stays unavailable and is never replaced with a live ticker.
  History reads are latest-request-wins and are invalidated on logout or
  unmount; guest, loading, error, list, and empty states remain mutually
  exclusive, and a failed read remains retryable.
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
| Seconds history price is missing or malformed | Render the unavailable placeholder; do not coerce it to zero or substitute market data |
| Seconds history result/status is unknown | Show the trimmed backend source value instead of an incorrect known translation |
| Seconds history request settles after logout, retry, or unmount | Ignore the stale response and preserve the newer guest/request state |
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
- Seconds history tests must execute delayed request promises to prove guest
  isolation, latest-request-wins retry, logout/unmount invalidation, and error
  recovery. They must also exercise shared active filtering, optional invalid
  prices, known translations, and visible unknown result/status source values.
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
}

interface ConvertPair {
  fromAssetLogoUrl?: string
  toAssetLogoUrl?: string
}
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

### Good / Base / Bad Cases

- Good: the selected BTC→USDT pair renders its exact from/to API images, then
  switching to USDT→BTC renders the reverse pair's two distinct API images.
- Base: a pair Logo is `null`; `AssetMark` receives `undefined` and renders the
  accessible symbol initial without inventing an image URL.
- Bad: the picker obtains images from `wallet_accounts.logo_url`, groups raw
  `" btc "` separately from `BTC`, or silently accepts a numeric Logo value.

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
  execute reverse/picker pair reactivity and the `AssetMark` source-exhaustion
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

## 12. Wallet Ledger Category Contract

```ts
type WalletLedgerCategory =
  | 'funding' | 'spot' | 'margin' | 'seconds' | 'convert'
  | 'earn' | 'new_coin' | 'loan' | 'prediction' | 'other'

fetchWalletLedger(options?: {
  limit?: number
  offset?: number
  category?: WalletLedgerCategory
  changeType?: string
}): Promise<WalletLedgerPage>
```

- The category union must match the Rust API exactly. Omission means all rows;
  `change_type` remains an optional exact compatibility filter and may be
  combined with category.
- The adapter strictly validates authoritative entry categories, decimal
  amount/fee/balance fields, timestamps, and pagination totals. A selected
  category response containing another category is a contract error and must
  not enter page state.
- Amount, post-change balance, and positive fee presentation uses one
  locale-aware ledger formatter with up to 8 fractional digits. The smallest
  supported positive unit (`0.00000001`) must remain visible rather than round
  to zero; negative zero is normalized to neutral zero.
- Initial load, cached refresh errors, load-more errors, empty state, and
  exhaustion remain distinct. Pagination advances its offset by response rows
  consumed, not by the deduplicated visible-entry count, and an empty page is
  exhausted even if inconsistent metadata claims more pages.
- Reads are keyed by exact session token and selected category. Category
  changes, token replacement/logout, and unmount invalidate older reads before
  they can mutate entries, errors, loading state, or pagination.
- Local adapter/contract diagnostics are not user copy; the view renders the
  localized ledger failure message instead of exposing internal English error
  strings.

Required tests cover the exact category union, strict response mapping,
category mismatch, row-based offset/exhaustion, local-date ordering,
filter/session/unmount stale responses, known/unknown type presentation,
pluralization, signed zero, 8-place amount/balance/positive-fee precision,
state branches, 44px controls, and 320px horizontal-overflow guards.
