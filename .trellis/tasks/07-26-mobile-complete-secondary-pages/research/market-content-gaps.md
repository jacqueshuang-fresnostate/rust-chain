# Research: Production mobile market/content/trade secondary-page gaps

- Query: Compare production mobile market/content/trade secondary-page workflows with `mobile/sites-prototype/app/page.tsx`, covering market detail, search/picker, news list/detail, orders, trade pair picker/settings, and notifications/messages; recommend prototype pages and acceptance checks.
- Scope: internal
- Date: 2026-07-26

## Findings

### Files found

- `mobile/src/router/index.ts` — production mobile route inventory, depth, bottom-nav visibility, and direct-open back fallbacks.
- `mobile/src/views/HomeView.vue` — production entry points for market detail, market search, announcements, news detail/list, and the currently inert notification bell.
- `mobile/src/views/MarketsView.vue` — shared normal market browser and trade-purpose pair picker.
- `mobile/src/views/MarketDetailView.vue` — quote, K-line, depth, recent trades, interval selection, back behavior, and trade handoff.
- `mobile/src/views/NewsView.vue`, `mobile/src/views/NewsDetailView.vue`, `mobile/src/api/news.ts` — locale-aware news list/detail loading and rendering.
- `mobile/src/views/TradeView.vue` — trade pair picker entry, mode replacement, contract leverage setting, and order-center links.
- `mobile/src/views/OrdersView.vue` — authenticated spot orders, margin positions, history, refresh, cancel, close, and bulk actions.
- `mobile/src/stores/navigation.ts`, `mobile/src/core/navigation.ts` — persisted trade symbol/mode and safe direct-open back fallback.
- `mobile/src/App.vue`, `mobile/src/components/PageHeader.vue` — secondary-page bottom-nav suppression, route transitions, and shared back behavior.
- `mobile/sites-prototype/app/page.tsx` — current six-view, in-memory prototype and all relevant inert/toast-only entries.
- `mobile/sites-prototype/tests/rendered-html.test.mjs` — current prototype contract coverage, limited to shell/navigation/design invariants and spot/contract separation.

### Route / interaction / gap matrix

| Workflow | Production route and interaction | Prototype now | Gap and recommended prototype surface |
|---|---|---|---|
| Market detail | Normal market selection pushes `/markets/:symbol`; the route hides bottom nav and falls back to `/markets` on direct open (`mobile/src/router/index.ts:42-45`, `mobile/src/views/MarketsView.vue:41-50`). Detail loads K-line, depth, and recent trades in parallel, supports `1m/15m/1h/4h/1d`, and replaces into the selected trade pair (`mobile/src/views/MarketDetailView.vue:33-69`, `mobile/src/views/MarketDetailView.vue:72-94`). | The only view IDs are six top-level columns (`mobile/sites-prototype/app/page.tsx:54`). Market rows set `selectedMarket` and immediately switch to spot/contract (`mobile/sites-prototype/app/page.tsx:688-719`, `mobile/sites-prototype/app/page.tsx:1281-1296`). | Add `market-detail` as a secondary view with back, selected symbol, price/24h stats, timeframe controls, chart, depth/recent trades, favorite/share affordances, and explicit “现货交易/合约交易” handoffs. Do not copy production's currently inert overview/data/updates/grid/alert controls unless they receive prototype behavior. |
| Search / market picker | `/markets` is both browser and picker. `purpose=trade&mode=spot|contract` changes title/back behavior; filtering matches symbol, categories sort popular/gainers/losers; picker selection remembers symbol/mode and replaces the trade route, while normal selection pushes detail (`mobile/src/views/MarketsView.vue:18-50`, `mobile/src/views/MarketsView.vue:60-88`). Pair-picker mode also hides bottom nav (`mobile/src/App.vue:8-12`). | Search, favorites, categories, and empty state work, but the filter icon is inert; all row selections go straight to trade and there is no browse-vs-picker purpose or back context (`mobile/sites-prototype/app/page.tsx:614-731`). | Reuse one `market-browser` surface in two contexts: normal browse opens `market-detail`; `market-picker` shows a back header, preserves spot/contract purpose, and returns to the originating trade column with the chosen pair. Make the filter button functional or remove it. |
| News list / detail | Home loads locale-aware announcements, opens a detail directly, and links to all announcements (`mobile/src/views/HomeView.vue:53-71`, `mobile/src/views/HomeView.vue:123-128`). `/news` provides refresh/loading/error/empty/list states and pushes `/news/:id`; detail shows banner, category, title, time, and localized body (`mobile/src/views/NewsView.vue:17-29`, `mobile/src/views/NewsDetailView.vue:15-25`). API selection prefers exact locale, language family, default locale, then first item (`mobile/src/api/news.ts:22-65`). | There are no news list/detail views. `Newspaper` is used for a market brief that opens the prediction product rather than news, and message actions only toast (`mobile/sites-prototype/app/page.tsx:528-529`, `mobile/sites-prototype/app/page.tsx:1214-1335`). | Add `news-list` and `news-detail`. Put a compact announcement module on Home; list needs refresh/loading/empty/error mock states; detail needs category, publish time, banner, text/image body treatment, and back to list. |
| Orders | `/orders?tab=spot|positions|history` is opened from Trade and maps `positions` to margin (`mobile/src/views/TradeView.vue:93-95`, `mobile/src/views/TradeView.vue:219`). It requires login, loads each tab independently, sorts records, supports per-item and bulk spot cancellation, pending-margin cancellation, position close/close-all, feedback/error states, and combined spot/margin history (`mobile/src/views/OrdersView.vue:27-185`, `mobile/src/views/OrdersView.vue:207-231`). | Profile lists “订单中心”, but every list item only emits an “已打开” toast; Trade has no order/position/history handoff (`mobile/sites-prototype/app/page.tsx:1083-1134`, `mobile/sites-prototype/app/page.tsx:734-965`). | Add `orders` with entry context `spot | positions | history`, three tabs, representative open/history cards, empty/loading/error states, and simulated cancel/close confirmations. Link it from both Trade and Profile. |
| Trade pair picker / settings | Pair selector pushes `/markets?purpose=trade&mode=...`; selection replaces back into the chosen pair and preserves contract mode (`mobile/src/views/TradeView.vue:75-86`, `mobile/src/views/MarketsView.vue:41-48`). Symbol/mode survive tab changes and reload through dedicated storage keys (`mobile/src/stores/navigation.ts:5-47`). Contract leverage cycles through backend-supported levels with auth/error/saving feedback; current production UI keeps margin mode fixed to isolated (`mobile/src/views/TradeView.vue:25-42`, `mobile/src/views/TradeView.vue:97-120`, `mobile/src/views/TradeView.vue:193-219`). | Pair selector, refresh, global settings, timeframe, and chart-settings controls have no handlers (`mobile/sites-prototype/app/page.tsx:768-819`). Inline contract margin mode and 10x/20x leverage toggles work locally, but have no confirm/cancel, capability validation, or persistence (`mobile/sites-prototype/app/page.tsx:832-850`). | Connect the selector to `market-picker`; add a `trade-settings` bottom sheet/page for contract margin mode and supported leverage, with current-value summary, cancel/apply, and persistence per pair/mode. Keep the existing independent spot/contract columns and return to the originating column. |
| Notifications / messages | Production exposes a Home notification bell label but no click handler, route, view, or mobile API; news/announcements are the only implemented content workflow (`mobile/src/views/HomeView.vue:74-88`, `mobile/src/router/index.ts:40-75`). | The global bell is inert, and the Home message icon only shows “消息中心已打开” as a toast (`mobile/sites-prototype/app/page.tsx:368-382`, `mobile/sites-prototype/app/page.tsx:442-456`). | Production has no workflow to mirror. Define a prototype-only `message-center` with “通知/公告” tabs, unread/read and empty states, mark-all-read, and message detail or inline expansion. Announcement items should deep-link to `news-detail`; both bell and message icons should open the same center. Label this as proposed UX, not production parity. |

### Code patterns

- Secondary routes carry `depth`, `showBottomNav: false`, and `backFallback`; `PageHeader` uses history when available and replaces with the fallback for direct opens (`mobile/src/router/index.ts:43-56`, `mobile/src/components/PageHeader.vue:15-31`, `mobile/src/core/navigation.ts:24-42`).
- Main-tab navigation replaces history, while secondary drill-downs push; trade mode and picker completion replace to avoid polluting the back stack (`mobile/src/components/AppBottomNav.vue:12-35`, `mobile/src/views/TradeView.vue:75-95`).
- The production pair picker is a context mode of the market browser, not a second duplicated list (`mobile/src/views/MarketsView.vue:26-50`).
- The prototype has no route or secondary-view stack: `activeView` is one of six top-level values, and `setView` only updates local state and scroll position (`mobile/sites-prototype/app/page.tsx:54`, `mobile/sites-prototype/app/page.tsx:1214-1245`).
- Existing prototype tests assert the six main columns and visual contracts but do not exercise any requested secondary workflow (`mobile/sites-prototype/tests/rendered-html.test.mjs:26-62`, `mobile/sites-prototype/tests/rendered-html.test.mjs:103-120`).

### Recommended prototype pages

1. `market-detail` — selected pair detail with chart/depth/trades and spot/contract CTAs.
2. `market-picker` — reusable market browser in picker context, preserving origin column and providing back.
3. `news-list` and `news-detail` — announcement discovery and readable content hierarchy.
4. `orders` — spot orders, contract positions, and history tabs with simulated management states.
5. `trade-settings` — modal/bottom-sheet secondary surface for applicable pair/contract settings.
6. `message-center` and optional `message-detail` — proposed notification/announcement inbox because production has no implemented equivalent.

### Acceptance checks

- Navigation: every secondary surface has a visible back action; direct-open/state-reset fallback is deterministic; bottom navigation is hidden while a secondary surface or picker is active; returning restores the originating top-level view.
- Market browse vs picker: a normal market row opens `market-detail`; the same row in picker context returns to the originating spot/contract page without switching columns; selected pair and trade mode survive leaving and returning to Trade.
- Market detail: selected symbol, price, stats, chart, depth, and recent trades are internally consistent; each timeframe visibly changes active state/data; spot and contract CTAs land on the same pair.
- News: Home announcement, all-news entry, list-to-detail, and detail-back-list paths work; loading, empty, error, banner-present, and text/image content cases are represented without `[object Object]`.
- Orders: Trade and Profile open the correct default tab; tab changes show distinct spot/position/history data; cancel/close require confirmation, update the visible mock state once, and expose success/failure feedback; unauthenticated/empty states remain legible.
- Trade settings: pair selector, global settings, timeframe, and chart settings are no longer inert; contract settings support cancel/apply and only show declared options; spot and contract settings do not leak into each other.
- Messages: both bell and message icons open the center; unread count/read state changes are visible; mark-all-read is idempotent; announcement rows open the matching news detail; empty state is reachable.
- Responsive/accessibility: at 390×844 there is no horizontal overflow or bottom-nav occlusion; secondary headers and sheets respect safe areas; controls have accessible names, selected tabs expose `aria-selected`, dialogs expose `aria-modal`, and keyboard Escape/backdrop close sheets without losing the origin context.
- Regression automation: extend source/SSR tests to assert all secondary view IDs and wired handlers, then add browser interaction coverage for each route chain above while retaining the existing six-column, no-Web3, Lucide, reduced-motion, and spot/contract-separation contracts.

### External references / versions

- No external web sources were needed; this is an internal production-to-prototype comparison.
- Production mobile: Vue `^3.4.0`, Vue Router `^4.3.0`, Vite `^5.2.0` (`mobile/package.json:28-38`).
- Prototype: Next `16.2.6`, React `19.2.6`, vinext `0.0.50`, Vite `8.0.13` (`mobile/sites-prototype/package.json:19-38`).

### Related specs

- `.trellis/spec/mobile/navigation-and-localization.md` — authoritative push/replace, picker query, trade persistence, direct-open back, bottom-nav, and locale contracts.
- `.trellis/spec/mobile/index.md` — mobile quality-gate commands and package scope.
- `.trellis/spec/backend/public-news-contract.md` — locale families and rich-text news content shape.
- `.trellis/spec/backend/spot-orders.md` — spot cancellation/idempotency and order-state constraints represented by Orders.
- `.trellis/spec/backend/margin-trading-actions.md` — supported margin settings and close/cancel/bulk-action behavior.
- `.trellis/spec/guides/cross-layer-thinking-guide.md` — API/state/display boundary checks for later implementation.

## Caveats / Not Found

- `task.py current --source` reported no active task; the user supplied the exact task directory and output path, and `task.json` confirms this task exists with `planning` status.
- Production has no notification/message route, view, or mobile API, so the proposed message center is a new prototype workflow rather than a parity port.
- Several production market-detail affordances are visual only (favorite, share, overview/data/updates, grid, alert). They should not be treated as functioning production requirements.
- Production news currently flattens rich-text blocks to text and omits image blocks in the body (`mobile/src/api/news.ts:47-65`), while the backend spec permits rich-text images; prototype acceptance should cover the intended content shape without claiming the current mobile renderer is complete.
- Production contract UI currently fixes margin mode to isolated, while the backend spec describes isolated/cross capability and the prototype toggles 全仓/逐仓 locally. Implementation should resolve the intended capability source instead of copying either behavior blindly.
