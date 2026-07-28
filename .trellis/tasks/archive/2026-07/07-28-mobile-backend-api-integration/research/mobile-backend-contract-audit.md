# Research: Mobile–Backend Contract Audit

- Query: Audit the real mobile API layer against Rust public/authenticated routes and DTOs, prioritizing actionable route, method, DTO, auth/refresh, WebSocket, and screen-level mismatches.
- Scope: internal
- Date: 2026-07-28

## Findings

### Result

No nonexistent HTTP path or wrong HTTP method was found in the mobile API wrappers after applying the shared `/api/v1` prefix. The actionable failures are configuration, auth-boundary, response-adapter, and screen-to-contract mismatches.

### Prioritized actionable mismatches

#### P0 — Native/production builds silently target device loopback

- Mobile falls back to `http://127.0.0.1:8080` whenever `VITE_BACKEND_API_DOMAIN` is absent outside dev: `mobile/src/config/app.ts:3-6`.
- The same value drives every HTTP request through `backendApiUrl`: `mobile/src/config/app.ts:10-12`.
- `.env.example` acknowledges that iOS/Android builds must inject a real backend, but the runtime still silently accepts the invalid fallback: `mobile/.env.example:1-4`.
- Impact: an unconfigured PWA/Tauri production build sends all API and WebSocket traffic to the end-user device, so no route can work.
- Mobile action: remove the non-dev loopback fallback; allow same-origin for PWA and fail with a diagnostic configuration error for native builds without an explicit reachable HTTPS origin. Generate HTTP, health, and WebSocket URLs from one validated origin.

#### P0 — Mandatory first-time 2FA produces an unresolvable login challenge

- Under mandatory login 2FA, a user without TOTP receives `requires_2fa_setup` plus `setup_challenge_id`: `src/modules/auth/application.rs:426-452`, serialized by `src/modules/auth/presentation.rs:117-122`.
- Mobile recognizes the response and routes it to the 2FA page: `mobile/src/api/auth.ts:45-60`, `mobile/src/views/LoginView.vue:59-82`.
- The setup branch only displays a message and returns to login; it never consumes the challenge: `mobile/src/views/LoginTwoFactorView.vue:16-17`, `mobile/src/views/LoginTwoFactorView.vue:138-142`.
- The backend public auth router has login verification/reset routes but no setup-challenge completion route: `src/modules/auth/routes.rs:36-51`. Existing `/user/2fa/setup` requires an already-authenticated `UserAuth`: `src/modules/user/routes.rs:49-52`, `src/modules/user/routes.rs:170-183`.
- Impact: users affected by mandatory 2FA enter a permanent login loop.
- Action: this cannot be completed by mobile alone. Define a public setup-challenge contract in the backend, then add the QR/secret/confirmation flow in mobile while preserving the original redirect. Until that exists, mobile should surface an explicit unsupported-flow error instead of inviting repeated login attempts.

#### P1 — Dev WebSocket connects to Vite, but Vite proxies only HTTP `/api/v1`

- In dev, `APP_CONFIG.backendDomain` is empty, so the WebSocket URL falls back to the page origin and root `/ws/public`: `mobile/src/config/app.ts:3-6`, `mobile/src/api/marketSocket.ts:24-29`.
- Vite proxies only `[apiPrefix]` (normally `/api/v1`) and has no `/ws` proxy or `ws: true`: `mobile/vite.config.ts:117-126`.
- Rust does expose both root and nested event routes: `src/lib.rs:54-60`, `src/modules/events/routes.rs:31-45`. The wire subscription sent by mobile—`{op:"subscribe",channel:"ticker",symbol}`—matches the backend protocol: `mobile/src/api/marketSocket.ts:49-52`.
- Impact: dev HTTP succeeds through Vite while ticker WebSocket attempts `ws://<vite-host>:1611/ws/public` and fails before reaching Rust.
- Mobile action: use `/api/v1/ws/public` in same-origin mode and configure that proxy with `ws: true`, or add a dedicated `/ws` WebSocket proxy. Cover URL generation and the upgrade path with tests.

#### P1 — 401 retry incorrectly refreshes auth bootstrap requests

- The request interceptor adds a stale Bearer token to every request, including login/register/2FA/password-reset calls: `mobile/src/api/client.ts:47-50`.
- The response interceptor excludes only `/auth/refresh`; every other 401 can refresh and replay once: `mobile/src/api/client.ts:79-96`.
- The project contract explicitly excludes login, register, 2FA, and refresh routes from retry: `.trellis/spec/backend/auth-sessions.md:36-39`, with required tests at `.trellis/spec/backend/auth-sessions.md:67-69`.
- Impact: invalid credentials, expired 2FA challenges, or reset failures can trigger an unrelated session refresh and replay; stale authenticated state can also be retained during a new login attempt.
- Mobile action: classify protected requests explicitly. Do not attach Bearer or run refresh for `/auth/login`, `/auth/register`, `/auth/login/2fa*`, `/auth/password/*`, and `/auth/refresh`; retain the singleton one-shot retry only for protected user routes.

#### P1 — New-coin purchase UI contradicts the purchase DTO’s authoritative quote asset

- Backend contract makes `pair_id` authoritative; purchase body is only `pair_id`, `price`, `quantity`, and `idempotency_key`: `.trellis/spec/backend/new-coin-mobile-contract.md:19-24`.
- Mobile sends those correct fields: `mobile/src/api/newCoin.ts:163-169`.
- The screen nevertheless lets the user choose any wallet account and validates/displays payment against that selected account: `mobile/src/views/NewCoinDetailView.vue:50-62`, `mobile/src/views/NewCoinDetailView.vue:221-245`. That account ID is not sent in the purchase request: `mobile/src/views/NewCoinDetailView.vue:125-139`.
- Percentage shortcuts treat quote balance as token quantity, although purchase `amount` is sent as `quantity`; they should divide spendable quote balance by execution price: `mobile/src/views/NewCoinDetailView.vue:54-60`, `mobile/src/views/NewCoinDetailView.vue:106-114`.
- Impact: mobile can show/validate BTC (or another wallet) while Rust debits the pair’s actual quote asset; “100%” is also wrong whenever price is not 1.
- Mobile action: in purchase mode lock the payment account to the backend pair’s quote asset, label it read-only, compute percentage quantity as `(quote available × percentage) / execution price`, and keep free asset selection only for subscription mode.

#### P2 — Public spot trade screen calls authenticated margin metadata

- `TradeView` always requests `fetchMarginProducts()` on mount, including guest spot mode: `mobile/src/views/TradeView.vue:205-220`.
- `GET /api/v1/margin/products` requires `UserAuth`: `src/modules/margin/routes.rs:94-102`.
- The exception is swallowed and products become empty, so spot can continue but guest traffic receives an avoidable 401 and contract mode appears unavailable without an explicit login state.
- Mobile action: request margin products only when authenticated and contract mode needs them, or render the login-required contract state before making the request.

#### P2 — News detail parser drops supported rich-text structure

- Rust returns `content_json` as structured JSON: `src/modules/news/presentation.rs:37-53`.
- The contract permits rich-text marks and image blocks: `.trellis/spec/backend/public-news-contract.md:11-29`.
- Mobile selects the correct locale but flattens only `children[*].text`, discarding image URLs, links, formatting, and non-text blocks: `mobile/src/api/news.ts:47-65`. The view renders the result as plain text: `mobile/src/views/NewsDetailView.vue:62-79`.
- Mobile action: add a safe rich-text adapter/renderer that preserves supported text marks and image blocks; keep the current locale/default fallback.

#### P2 — “Message Center” is public news with device-local read state

- The screen loads `fetchNews(40)`, stores read IDs only in local storage, and opens the public news detail route: `mobile/src/views/MessageCenterView.vue:27-75`.
- The backend route is public news only—`GET /news` and `GET /news/:id`: `src/modules/news/routes.rs:9-27`. No user notification/message route was found in the mounted user routers.
- Impact: the screen cannot represent account-specific messages, server unread state, or cross-device read state even though its UI claims a message center.
- Mobile action: if the intended feature is announcements, rename/relabel it accordingly. If it is a personal inbox, keep it unavailable until a backend notification contract exists rather than adapting `/news` as user messages.

#### P3 — Mobile drops backend prediction order number

- Backend user order DTO exposes `order_no`: `src/modules/prediction/presentation.rs:220-240`.
- Mobile maps only raw numeric `id` and omits `order_no`: `mobile/src/api/prediction.ts:39-48`, `mobile/src/api/prediction.ts:100-111`.
- The order list uses raw `id` only as the Vue key and displays no business order number: `mobile/src/views/PredictionView.vue:243-252`.
- Mobile action: map `order_no` to `orderNo` and show it in order history; retain `id` only as the internal key.

#### P3 — Closed/inactive margin positions can lose their pair label

- Position DTO contains `pair_id` but no symbol: `src/modules/margin/presentation.rs:273-303`.
- Mobile probes nonexistent `symbol`/`pair_symbol` fields and otherwise stores the numeric pair ID as `symbol`: `mobile/src/api/trading.ts:240-255`.
- `OrdersView` can recover the label only when the position’s product remains in the active product list; otherwise it falls back to a generic contract number: `mobile/src/views/OrdersView.vue:59-61`.
- Mobile action: join positions to a stable pair/product lookup by `product_id` or `pair_id`; do not treat `pair_id` as a symbol string.

### Confirmed matching contracts

- Route prefix/mounting is correct: mobile builds `/api/v1/*` and Rust nests all user routers under `/api/v1`: `mobile/src/config/app.ts:5-12`, `src/lib.rs:17-36`, `src/lib.rs:54-60`.
- Paths, methods, request names, and top-level envelopes matched for auth (except setup flow), countries, market REST, wallet/deposit/withdrawal/ledger/quick recharge, spot orders, margin writes, convert, seconds contracts, earn, loan, new-coin lifecycle, prediction quote/order writes, news, profile/KYC/security/referral.
- Envelope conventions correctly handled include `{markets}`, bare ticker/depth, bare kline array, `{trades}`, `{accounts}`, `{entries}`, `{orders}`, `{products}`, `{positions}`, `{subscriptions}`, `{projects}`, `{purchases}`, `{unlocks}`, `{allowed_assets}`, and `{news}`.
- Public ticker WebSocket message parsing matches the backend’s direct payload shape; no extra response envelope is required. Mobile currently has no private WebSocket consumer, so `/ws/private?token=<access_token>` was verified as a backend contract but is not exercised by a mobile screen: `.trellis/spec/backend/auth-sessions.md:15-25`, `src/modules/events/routes.rs:37-45`.

### Files found

- `mobile/src/config/app.ts` — HTTP/backend-origin composition and invalid production fallback.
- `mobile/vite.config.ts` — dev HTTP proxy without WebSocket forwarding.
- `mobile/src/api/client.ts` — Bearer injection, refresh, replay, and API URL helper.
- `mobile/src/api/marketSocket.ts` — public ticker WebSocket URL and subscription frame.
- `mobile/src/api/*.ts` — real mobile route, body/query, and response-envelope adapters.
- `mobile/src/views/*.vue` — screen-level auth guards and DTO usage.
- `src/lib.rs` — root and `/api/v1` router mounting.
- `src/modules/*/routes.rs` — Rust public/authenticated route methods and extractors.
- `src/modules/*/presentation.rs` — Rust request/response DTO field and envelope definitions.
- `.trellis/spec/backend/auth-sessions.md` — refresh retry and private WebSocket contract.
- `.trellis/spec/backend/realtime-websockets.md` — public/private WebSocket path and frame contract.
- `.trellis/spec/backend/new-coin-mobile-contract.md` — authoritative pair and purchase-body contract.
- `.trellis/spec/backend/public-news-contract.md` — localized rich-text response contract.
- `.trellis/spec/backend/prediction-markets.md` — prediction order-number display contract.
- `.trellis/tasks/07-28-mobile-backend-api-integration/prd.md` — task URL, proxy, auth, smoke-test, and build acceptance criteria.

### External references / versions

- No external web references were required; the live repository is the source of truth for this audit.
- Relevant declared versions: Axios `^1.6.0`, Vue `^3.4.0`, Vue Router `^4.3.0`, Vite `^5.2.0` in `mobile/package.json:23-42`; Axum `0.7` with `ws` support in `Cargo.toml:17`.

### Related specs

- `.trellis/spec/backend/auth-sessions.md`
- `.trellis/spec/backend/realtime-websockets.md`
- `.trellis/spec/backend/user-authentication.md`
- `.trellis/spec/backend/new-coin-mobile-contract.md`
- `.trellis/spec/backend/public-news-contract.md`
- `.trellis/spec/backend/prediction-markets.md`
- `.trellis/spec/mobile/index.md`
- `.trellis/spec/mobile/pwa-and-shell.md`

## Caveats / Not Found

- `python3 ./.trellis/scripts/task.py current --source` reported no current task pointer. This report uses the exact task directory explicitly supplied by the user; `.trellis/tasks/07-28-mobile-backend-api-integration/task.json:1-25` exists and is still in `planning`.
- This was a static contract audit. No backend service, database-backed flow, browser WebSocket upgrade, or authenticated financial mutation was executed.
- No dedicated mobile API/refresh/WebSocket contract tests were found under `mobile/tests`; current repository route tests are predominantly backend-side. The task PRD already requires URL, proxy, WebSocket, and key contract automation: `.trellis/tasks/07-28-mobile-backend-api-integration/prd.md:24-43`.
- No mobile endpoint was found for `GET /api/v1/prediction/markets/:id`; the current mobile prediction screen is list/ticket based, so this is a feature gap rather than a call to a nonexistent endpoint.
- No code or files outside this research document were modified.
