# Mobile PWA, Theme, and Application Shell Contract

## 1. Scope / Trigger

Apply this contract when changing `mobile/` build modes, Vite public base,
manifest metadata, service-worker behavior, install/update prompts, theme
tokens, the root application shell, bottom navigation, or the announcement
message center. It also applies when a selected Pencil secondary page needs a
theme selector or canvas override that crosses a Vue SFC scoped-style boundary,
or when Loan changes its collateral-asset selection surface.

This cross-layer contract prevents financial responses from entering browser
caches, PWA code from leaking into Tauri bundles, root navigation drift, and
fabricated account or transaction messages.

## 2. Signatures

Build commands:

```text
npm run build:pwa   -> vue-tsc --noEmit && vite build --mode pwa
npm run build:tauri -> vue-tsc --noEmit && vite build --mode tauri
```

Runtime and theme signatures:

```ts
isTauriRuntime(globalObject?: object): boolean
initializePwa(): Promise<void>
promptPwaInstall(): Promise<'accepted' | 'dismissed' | 'unavailable'>
applyPwaUpdate(): Promise<boolean>
resolveServiceWorkerLocation(base: string, origin: string): {
  scope: string
  scriptUrl: string
}
resolveAppTheme(value: unknown, prefersDark?: boolean): 'light' | 'dark'
applyAppTheme(theme: 'light' | 'dark'): void
```

Persisted keys:

```text
hippo_mobile_theme
hippo_pwa_install_dismissed_at
hippo_mobile_message_read_ids
```

Selected-page CSS boundary and 390px Product Hub geometry:

```text
global layer: mobile/src/styles/pencil-selected-pages.css (import exactly once)
wallet/deposit canvas: #ffffff light / #000000 dark
contract, seconds, product-hub, prediction canvas: #ffffff light / #000000 dark
spot canvas: excluded from those selected-page overrides
ProductHub final cascade: display:block; gap:0
ProductHub header/body/first-row y at 390px: 0..60 / 60 / 68
```

Spot order-type picker signatures in `TradeView.vue`:

```ts
type SpotOrderType = 'limit' | 'market'
openSpotOrderTypeSheet(): void
closeSpotOrderTypeSheet(): void
selectSpotOrderType(type: SpotOrderType): void
```

Spot account-surface signatures:

```ts
spotVisibleBalances: ComputedRef<WalletAccount[]> // current base/quote, total > 0
openOrders(tab: 'spot' | 'positions' | 'history' = 'spot'): void
openAssets(): void
```

Loan collateral-picker signatures in `LoanView.vue`:

```ts
selectedCollateral: ComputedRef<WalletAccount | undefined>
modalOpen: ComputedRef<boolean> // order-action dialog OR collateral picker
openCollateralPicker(): void
closeCollateralPicker(): void
selectCollateralAsset(account: WalletAccount): void
```

## 3. Contracts

### Build and environment

- `VITE_PWA_BASE` is `/` for a dedicated origin or one permanent slash-wrapped
  prefix such as `/mobile/`.
- PWA is enabled only in Vite mode `pwa` when no `TAURI_ENV_PLATFORM` is
  present. The compile-time `__PWA_ENABLED__` flag and
  `isTauriRuntime()` are both required before registration.
- Tauri uses mode `tauri`, `publicDir: false`, strips `data-pwa-only` metadata,
  and must not emit `manifest.webmanifest`, `sw.js`, Workbox code, or PWA icons.
- `src-tauri/tauri.conf.json` must keep
  `beforeBuildCommand: "npm run build:tauri"`.
- Product PWA and Tauri builds use
  `https://hipoex.cllbmz.kdns.fr` when
  `VITE_BACKEND_API_DOMAIN` is missing or whitespace-only. Non-empty validated
  environment values retain priority; browser development still calls the
  Vite origin and uses its independently configurable proxy.

### Manifest and service worker

- Manifest identity is `Hippo Mobile` / `Hippo`, display is `standalone`,
  orientation is `portrait-primary`, and 192, 512, maskable 512, and Apple 180
  brand assets must remain valid PNG files.
- Workbox uses `generateSW`, prompt updates, a base-aware navigation fallback,
  and static compile-output precaching only.
- `runtimeCaching` stays empty. Do not add CacheFirst, NetworkFirst,
  StaleWhileRevalidate, Background Sync, or financial request queues.
- Navigation fallback must deny `/api/`, `/ws/`, `/health/`, and download
  endpoints. Authentication, wallet, order, KYC, market, and WebSocket data
  must always stay network-owned.
- Offline readiness means the application shell can render. It never implies
  that prices, balances, orders, or trading actions are available offline.

### Runtime UI and theme

- `PwaStatus` is mounted exactly once in `App.vue`. Install and update prompts
  appear only on explicitly allowed safe routes; offline and registration
  errors may remain global.
- Updates stay user-controlled: a waiting worker receives `SKIP_WAITING`, then
  the page reloads on `controllerchange`.
- The root theme is `data-theme="light|dark"` on `<html>`, persists through
  `hippo_mobile_theme`, updates `color-scheme` and every `theme-color` meta tag,
  and falls back to the system preference when storage is invalid.
- Canvas-backed widgets that copy CSS tokens into an imperative renderer must
  resolve tokens from their nearest `.app-stage`, observe that stage's `class`
  and `<html data-theme>`, and reapply renderer colors without resetting user
  viewport state. Disconnect the shared observer on unmount. Watching only
  `<html data-theme>` is insufficient because the prototype token family is
  selected by `.app-stage.theme-light|theme-dark`.
- Locale changes must update an already-mounted imperative chart in place and
  must not remount it or reset its viewport. Full-history refreshes preserve a
  timestamp-anchored window rather than raw logical indexes so prepended or
  trimmed rows keep the same visible candles.
- Market charts use two npm/Vite-bundled renderers: `klinecharts@10.0.0` is the
  default and `lightweight-charts@5.2.0` is the selectable TradingView mode.
  PWA, Tauri, and Android artifacts must contain both local packages and must
  not load chart scripts, frames, widgets, Pro modules, or market data from a
  remote runtime. Disable the optional Lightweight Charts attribution logo in
  both create and theme-apply options so the rendered local chart contains no
  external anchor.
- Persist the chart choice under `hippo_mobile_market_chart_engine`, default
  invalid or unavailable storage to KLineChart, expose a 44px keyboard/touch
  radio group, and mount exactly one engine. Switching is presentation-only and
  must not mutate or reconnect the parent market-data session.
- Shared text uses stable pixel sizes and `letter-spacing: 0`; do not scale
  font size with viewport width.
- The selected visual bottom navigation has exactly five entries in this order:
  Home, Markets, Trade, Assets, Profile. Trade is the raised center action and
  resolves the persisted pair plus the persisted spot/contract mode; this
  visual consolidation must not merge the independent spot, contract, or
  seconds business routes. Seconds stays reachable from the selected Home
  shortcut and its direct route while using the secondary motion/back
  contract. All dock icon targets remain at least
  44x44px from 320px through 448px.
- The raised Trade face is one uninterrupted 56px mint circle with a 24px
  Lucide ArrowLeftRight. Its computed `background-image` must be `none`.
  Earlier legacy active-item selectors must explicitly exclude
  `.trade-nav-action`; otherwise their centered 28px gradient wins by
  specificity and turns the circle into a square color patch.
- The selected Pencil spot references are `yzOPc` (light) and `bo8k5` (dark).
  Their production default is a 64px spot-owned secondary header, a continuous
  left order form plus right 148px five-ask/mid/five-bid book, a truthful account
  state, and a collapsed local-chart entry. The spot route keeps the five-entry
  dock but must not mount `RootHeader`; `RootHeader` is limited to Home,
  Markets, Assets, and Profile. Contract mode may retain its dedicated existing
  workspace and must not be merged into the Pencil spot branch.
- Spot fields put the visible focus border and ring on the complete field shell.
  The nested input must have no border, outline, or inset focus shadow. Live
  market status text uses the global `.sr-only` utility and must remain clipped
  to 1x1px instead of entering the visual layout.
- The spot order-type field is a dialog trigger, not a cyclic toggle. Opening
  it must preserve the current `limit|market` value; only selecting an explicit
  option may mutate `orderType`. The two choices expose `aria-pressed` and a
  visible selected state, then close immediately after selection. Backdrop,
  close-button, and Escape dismissal preserve the current value.
- The spot order-type sheet reuses `useModalDialog` for body scroll lock, Tab
  wrap, initial focus on the current choice, and trigger focus restoration. It
  Teleports to `body`, includes viewport and safe-area bounds, suppresses the
  scroll chain, and owns a reduced-motion rule that does not depend on the
  non-Teleported `.trade-view` ancestor. It and the order confirmation dialog
  are mutually exclusive; one dialog's cleanup must not overwrite the other's
  saved `body.style.overflow`.
- Changing the spot order type is presentation/form state only. Limit continues
  to use the entered price; market continues to use the live current price;
  both submit through the existing `placeSpotOrder` type/price/quantity
  contract. Contract mode remains forced to market and closes any open spot
  order-type sheet.
- The spot account workspace is a holdings summary, not an open-order list.
  Its current navigation marker reads localized Positions/Holdings and is a
  non-interactive `aria-current` label for the region that renders wallet
  loading, error, base/quote holdings, and empty branches. The Orders action
  navigates to `/orders?tab=spot`; History navigates to
  `/orders?tab=history`. The spot template must not route this holdings marker
  to `/orders?tab=positions` or import order-query/cancel APIs.
- The 34px holdings context row may state that the wallet preview is limited to
  the current pair and link to the authenticated Assets surface for the full
  list. It must not expose Cancel all without loading authoritative current
  orders. Preserve the selected Pencil account geometry as one border plus
  48px navigation plus 34px context plus at least 198px content: 281px total.
- Production routes mapped from the currently selected Pencil secondary
  screens declare their exact light/dark frame IDs on the page root through
  `data-pencil-source`. Reusable geometry lives in the single imported
  `pencil-selected-pages.css` layer; page-scoped CSS owns only route-specific
  composition. Assets and Profile own a 60px root-form Pencil `PageHeader` plus
  the five-entry dock, so `App.vue` must not also mount `RootHeader`. Orders
  owns the same 60px secondary header while retaining the dock. Auth pages own
  their full-page identity header. News, Swap, Earn, Loan, and New Coin flows
  remain secondary pages with their real APIs, loading/guest/error states, and
  back fallbacks intact.
- The saved selected-state additions are New Coin Records `A9It6g/h4gfd`,
  Assets Transfer `v6phV/TuWXq`, Help `UouET/FM5tp`, Orders Empty
  `e5Qs1/hxe8l`, Wallet Ledger Empty `Bcug6/IVMAO`, Message Empty
  `t7j6n/eSMHf`, Prediction Bet `CzpTv/ZvGMv`, and Earn Subscribe
  `nqP6W/aXxul`. Append these IDs to the owning production root rather than
  replacing its existing base-state IDs.
- A modal nested under a transformed route host must Teleport its fixed overlay
  to `document.body`; otherwise `position: fixed` is trapped by the route's
  containing block and the sheet cannot reach the visual viewport edge. The
  Teleported node keeps a route-specific class for scoped theme/focus styling
  and remains above the Dock and route transitions.
- A confirmation sheet whose content can exceed a handset viewport uses a
  three-row `auto minmax(0, 1fr) auto` grid. The sheet itself has
  `overflow: hidden`; only the middle detail region may scroll, while the
  header and action row remain fully visible inside dynamic-viewport and safe
  area bounds. Do not put submit/cancel actions inside the scrolling region.
- The login Turnstile uses Cloudflare explicit rendering with `size: flexible`,
  the current application light/dark theme, and the current mobile locale. Its
  centered stage may widen to 302px inside a 320px viewport so the official
  300px minimum challenge remains usable, but it must stay inside the viewport
  and leave `documentElement.scrollWidth` unchanged. The application must not
  wrap the widget in a decorative card or duplicate Cloudflare branding. Do
  not scale, clip, cover, or disable pointer events on the Cloudflare iframe.
  A lightweight loader may occupy the centered stage before `render` returns;
  later states use Cloudflare's native surface plus an `aria-live` message. A
  successful `reset` retains its widget ID, including numeric ID `0`.
- Shared selected-page light/dark tokens and every selector rooted at
  `html[data-theme='dark']` belong in global `pencil-selected-pages.css`. Do not
  place `:global(html[data-theme='dark']) .local-class` in a scoped SFC: this
  project's Vue compilation path may omit that emitted rule, leaving dark pages
  on light tokens. Scoped styles may still own route-local layout beneath the
  globally themed root.
- The global layer owns the flat wallet/deposit canvas (`#ffffff` / `#000000`)
  and the flat Contract, Seconds, Product Hub, and Prediction canvases. Its
  selector allowlist is `.wallet-pencil-page`, `.contract-trade`,
  `.seconds-page`, `.product-hub`, and `.prediction-page`; it must not match
  `.spot-trade` or another spot root.
- Product Hub's scoped composition must override the earlier legacy
  `.product-hub { display: grid; gap: 14px; }` with
  `.product-hub { display: block; gap: 0; }`. Otherwise the legacy grid and page
  min-height distribute free space and stretch the two rows away from the
  selected geometry. At a 390px viewport, the final rendered body starts at
  `y=60` and its 8px top padding places the first product row at `y=68`.
- Route-local visual controls must be checked against earlier global
  `prototype-parity.css` selectors before relying on source order. If a scoped
  base selector is strengthened to preserve material styles, every modifier
  that changes geometry or interaction state (for example `.is-expanded`,
  `:active`, or `:focus-visible`) must compile to specificity equal to or
  greater than that base selector. Source-contract tests must compile the SFC
  CSS, compare the competing selectors, and pair that check with runtime
  computed geometry in both themes; isolated declaration checks are not enough.
- The message center calls `fetchNews(40)` and may persist only local read IDs.
  It must not invent account, order, wallet, security, or transaction events.
- The selected Message Center references are `FkZ6j` (light) and `bRz9K`
  (dark). The Pencil status bar belongs to native OS chrome and is not rendered
  inside the web body. Production therefore starts with a custom 56px sticky
  header: a 40px Lucide ArrowLeft at `x=20, y=12`, the 22px title in the
  selected center grid, and the 49px Read-all action at `x=321`. The four-tab
  filter starts at `y=56`, is exactly 38px high, and the list starts at `y=94`
  with `6px 20px 0` padding.
- Message rows are flat, continuous 64px rows with a 40px circular icon plate,
  12px internal column gap, no card fill, and no row separator. Light icon
  plates use `--surface-elevated` plus `--line`; dark plates resolve to
  `#0c100e` plus `#29342e`. Legacy `prototype-parity.css` message button/card
  rules and its shared `.message-icon` surface override must not participate in
  the selected page cascade.
- Message Center is a secondary surface: `/messages` has
  `meta.showBottomNav: false`, mounts neither `RootHeader` nor `AppBottomNav`,
  and its custom ArrowLeft delegates to `goBackOr` with the Home fallback.
  Loading, empty, and error rows reuse the same geometry but remain truthful to
  the live announcement response.
- A collateralized Loan application uses a button-triggered, body-Teleported
  bottom sheet instead of a native `select`. The trigger and every option pass
  the authoritative `WalletAccount.logoUrl` to `AssetMark`, show the exact
  wallet symbol and available balance, and retain the symbol fallback when the
  image is absent or fails.
- Selecting a collateral option changes only `collateralAssetId`, clears stale
  page feedback, and closes the sheet. `selectedCollateral`, available-balance
  validation, `collateralAmount`, and the existing `applyLoan` payload remain
  authoritative; the picker never invents an account, logo, or balance.
- The Loan collateral sheet and order-action dialog share one modal-open owner
  for body scroll locking and trigger focus restoration. They are mutually
  exclusive, trap Tab, close on Escape/backdrop/close-button, and keep every
  control at least 44px with bottom safe-area padding. Guests cannot open the
  picker; an authenticated empty wallet opens a truthful localized empty state.
- Loan does not render an authenticated-account readiness summary between its
  Hero and product categories. Authenticated users proceed directly to product
  categories; guests receive only one 48px localized login CTA that preserves
  the `/products/loan` redirect. Do not restore `loan-access-pencil__summary`,
  its decorative status icon, or its retired ready/login explanation copy.

## 4. Validation & Error Matrix

| Condition | Required behavior |
|-----------|-------------------|
| Vite mode is not `pwa` | PWA plugin disabled and no registration |
| Runtime exposes `__TAURI_INTERNALS__` | Return before service-worker registration |
| Service-worker registration fails | Set a localized retryable error; keep app usable |
| Browser has no install prompt | Hide Chromium action; show iOS instructions only on iOS browser |
| Install prompt was dismissed | Suppress it for seven days |
| Worker update is waiting | Offer update on a safe route; reload only after user accepts |
| Browser is offline | Render cached shell and truthful unavailable/error states |
| Stored theme is invalid or inaccessible | Use system preference, then light |
| Announcement API fails | Show retry and empty/error state; do not synthesize messages |
| Message Center opens from Home | Hide Root Header and Dock; Back returns through history to Home |
| Message Center is opened directly | Back replaces with its Home fallback |
| Message Center has no announcements | Render one truthful 64px empty row; keep Read-all disabled |
| Message Center changes theme | Resolve the root to white/black and the dark icon plate to `#0c100e` with `#29342e` border |
| Fixed sheet is mounted inside a transformed route | Teleport the overlay to `body`; its layer rect must equal the visual viewport |
| Confirmation details exceed a short viewport | Scroll only the middle detail region; keep header and every action button fully inside the safe viewport |
| Spot route has `showBottomNav` | Keep the five-entry dock, hide `RootHeader`, and render the spot-owned 64px header |
| Trade is the active dock item | Keep one 56px mint circle with no inherited 28px active gradient |
| Spot market stream has no rows yet | Show a truthful loading/unavailable state; never synthesize book or trade rows |
| Nested spot input receives focus | Apply one ring to `.spot-field-shell`; child input keeps `box-shadow: none` |
| Spot order-type trigger is selected | Open the Teleported sheet without changing `orderType` |
| Spot order-type option is selected | Set that exact value, close the sheet, and retain the existing price/submission contract |
| Spot order-type sheet is dismissed or contract mode activates | Close without changing the spot selection; contract remains market-only |
| Spot order-type and confirmation dialogs compete | Keep them mutually exclusive and restore the body overflow/focus owner exactly once |
| Spot account wallet rows are visible | Mark Holdings/Positions current and associate all wallet states with that labelled region |
| Spot user selects Orders or History | Navigate to `/orders?tab=spot|history`; keep OrdersView authoritative for order reads/actions |
| Spot account context offers a secondary action | Open Assets/View all; never show Cancel all without current-order data |
| Spot holdings marker is rendered | Keep it non-interactive and never route it to the futures `positions` tab |
| Loan is a credit product | Do not render collateral controls or change the existing application payload |
| Loan is collateralized and user is authenticated | Open the asset sheet without mutating the current asset until an explicit option is selected |
| Wallet account has `logoUrl` | Pass that URL to `AssetMark` in both the trigger and option row |
| Wallet account has no usable image | Keep the exact symbol fallback; do not guess or import another logo |
| Authenticated wallet list is empty | Open a localized empty sheet; keep submission disabled through existing collateral validation |
| Loan collateral sheet is dismissed | Preserve `collateralAssetId` and amount; restore body overflow and trigger focus exactly once |
| Loan user is authenticated | Skip the account-access summary and continue from Hero directly to product categories |
| Loan user is a guest | Render one login-limit CTA with the existing `/products/loan` redirect; do not render a duplicate summary |
| Assistive live status is rendered | `.sr-only` remains absolute, clipped, 1x1px, and visually absent |
| Turnstile renders at 320px | Keep a centered 302px stage and 300px challenge viewport within the device width; no decorative wrapper or horizontal scroll |
| Turnstile theme or locale changes | Remove and explicitly re-render the widget with the new app theme/language, clearing the previous token |
| Turnstile reset returns successfully | Keep the existing widget ID and expose the ready state; hard remove only when reset fails |
| A selected-page dark rule needs `html[data-theme='dark']` | Define it in global `pencil-selected-pages.css`; do not rely on scoped `:global(...)` output |
| Wallet/deposit page changes theme | Resolve the root canvas to `#ffffff`/`#000000` from the global layer |
| Contract, Seconds, Product Hub, or Prediction changes theme | Resolve the root canvas to `#ffffff`/`#000000` with no background image |
| Spot changes theme | Preserve the spot-owned canvas; selected secondary selectors must not match it |
| Legacy Product Hub grid rule is present | Later scoped composition must win with computed `display: block` and `gap: 0` |
| A scoped visual base selector is strengthened above a global bridge | Compile the scoped CSS and prove each geometry/state modifier outranks the base and competing global selector |
| Product Hub renders at 390px | Header ends/body starts at y=60; first row starts at y=68 |

## 5. Good / Base / Bad Cases

- Good: `build:pwa` produces manifest, service worker, brand icons, and an
  offline-reloadable shell while cache inspection contains no API or WebSocket
  endpoint.
- Good: selecting Contract opens the persisted pair with
  `?mode=contract`; selecting Seconds opens `/seconds`.
- Good: direct-open Spot renders the `yzOPc`/`bo8k5` split default with live
  depth, no root logo header, and the global dock still reachable.
- Base: Safari on iOS receives localized Add to Home Screen instructions
  because `beforeinstallprompt` is unavailable.
- Base: the local chart and latest trades stay behind the collapsed chart entry
  until the user explicitly expands it.
- Good: a user opens the order-type sheet while Limit is selected, sees Limit
  marked, selects Market, and the price field becomes market-read-only while
  `placeSpotOrder` still receives `type: 'market'` and the live effective price.
- Base: a user opens the order-type sheet and dismisses it with Escape; Limit
  remains selected and focus returns to the order-type field.
- Bad: clicking the order-type field directly flips Limit to Market, or closing
  the sheet changes the order type.
- Good: BTC/USDT wallet balances render below the current Holdings marker;
  tapping Orders opens the spot current-order page and View all opens Assets.
- Base: a guest or zero-balance user sees the holdings empty state under the
  same labelled region without any fabricated order state.
- Bad: wallet balances appear while Orders is styled current, Cancel all is
  shown without an order request, or the spot Holdings label opens futures
  positions.
- Good: a collateralized Loan shows the backend wallet logo in its trigger;
  opening the sheet shows every real wallet account and selecting one submits
  that account's numeric `assetId` through the unchanged application request.
- Base: the authenticated wallet has no accounts or an image fails; the picker
  shows a localized empty state or `AssetMark` symbol fallback and creates no
  placeholder financial data.
- Bad: Loan uses a native `select`, derives a logo from the symbol, changes the
  selected asset when the sheet merely opens, or lets the sheet and order-action
  dialog own `body.style.overflow` independently.
- Good: an authenticated Loan page transitions from its risk Hero directly to
  product categories; a guest sees one login-limit CTA in that position.
- Bad: Loan restores a signed-in/readiness summary, decorative ready icon, or
  explanatory account card above the product categories.
- Bad: a Tauri bundle contains `sw.js` or `manifest.webmanifest`.
- Bad: the first `OrderBookPanel` is assumed to be the split variant after the
  Pencil mini-book is added; tests must inspect all explicit layout instances.
- Bad: a nested input gets its own inset focus rectangle inside the field-shell
  ring, or `.sr-only` text becomes a visible row above the dock.
- Bad: a message row claims a deposit, login alert, or order event without a
  real backend source.
- Good: Message Center renders its 56px return Header, 38px tabs, and list at
  `y=94` without either application Header or Dock.
- Bad: Message Center inherits the old active tab pill, 78px card row,
  separator, or shared `.message-icon { background: var(--surface) }` rule.
- Good: toggling Wallet, Contract, Seconds, Product Hub, and Prediction switches
  their global canvas from white to pure black while Spot keeps its own tokens.
- Base: Product Hub loads only its two static route rows; the 60px Header is
  followed by body y=60 and first row y=68 without min-height stretching.
- Bad: dark canvas rules live in scoped `:global(...)`, Spot is included in the
  allowlist, or the legacy Product Hub grid remains the computed winner.

## 6. Tests Required

- Unit/source contract: build modes, Tauri double guard, manifest fields,
  `runtimeCaching: []`, denied fallback routes, single `PwaStatus`, safe prompt
  routes, theme normalization, and complete `zh-CN`/English keys.
- Build: `npm run build:pwa` and inspect generated manifest/service worker;
  `npm run build:tauri` and assert no PWA artifacts remain.
- Browser: theme switch survives reload; all five dock entries and the
  independent spot, contract, and seconds routes are reachable;
  320x720, 390x844, and 448x900 have no horizontal overflow; focused inputs
  retain a complete visible outline.
- Dock cascade: on the active Trade route assert a 56x56 circular icon face,
  mint computed background color, `background-image: none`, and a successful
  `elementFromPoint` hit within the Trade button. Source tests must fail when a
  legacy `.active:not(.seconds-nav-action)` selector does not also exclude
  `.trade-nav-action`.
- Spot Pencil parity: at 390px assert 64px header, 442px split workspace,
  196px form, 148px mini book, 281px account state, 48px chart entry, no
  `RootHeader`, and no PWA/status text entering the visual layout. At 320px the
  mini book contracts to 124px without document overflow. Repeat in both themes.
- Selected secondary parity: assert every mapped page root carries its recorded
  Pencil frame IDs; the shared stylesheet is imported exactly once; Assets,
  Profile, and Orders retain the dock without a duplicate Root Header; 320px
  and 390px light/dark browser passes have zero document overflow, sticky
  headers remain at z-index 70, and visible enabled controls are at least 40px
  in each dimension (44px for primary/icon controls).
- Viewport confirmation sheet: at 320x568, 320x720, 390x667, 390x844, and
  448x900 assert the Teleported overlay is a direct `body` child with no
  transformed route ancestor, every action button rect stays within the
  viewport, and scrolling an overflowing detail region does not move the
  action row. Also exercise Escape, Tab wrap, focus return, body scroll lock,
  both themes, and zero horizontal overflow.
- Message Center parity: at 390px assert header `0..56`, back button
  `20,12,40,40`, title `y=16,h=32`, Read-all `x=321,w=49`, filter `56..94`,
  list `y=94`, first row `x=20,y=100,w=350,h=64`, no row border, no Root
  Header/Dock, and no horizontal overflow. Repeat in dark mode and at 320px;
  click a category and exercise Home -> Message Center -> Back.
- Global selected-page cascade: inspect built/runtime CSS, not only source
  regexes. Assert light/dark computed canvases for Wallet, Contract, Seconds,
  Product Hub, and Prediction; assert the canvas selector group does not match
  the Spot root and Spot's computed canvas stays unchanged; and fail if a
  scoped SFC owns `:global(html[data-theme='dark'])` for these roots.
- Product Hub parity: load the complete stylesheet order including the legacy
  prototype rule, then assert computed `display === 'block'` and `gap === '0px'`.
  In a 390px browser viewport, assert body `getBoundingClientRect().y === 60`
  and first row `y === 68`; a source-only check for both declarations is not
  sufficient.
- Spot interaction: expanding the chart mounts exactly one local renderer and
  exposes real order-book/latest-trade tabs without iframe, remote chart script,
  or external chart anchor. Focusing a price/quantity/amount input leaves its
  own border/outline/shadow clear while the parent shell carries the only ring.
- Spot order type: prove the trigger only opens, both explicit options update
  the exact `limit|market` value, and backdrop/close/Escape do not mutate it.
  Assert `aria-haspopup`, `aria-expanded`, labelled dialog semantics,
  `aria-pressed`, initial focus, Tab wrap, focus return, body scroll lock,
  dialog mutual exclusion, contract-mode cleanup, and unchanged effective-price
  and `placeSpotOrder` wiring. Compile the scoped SFC CSS to prove Teleported
  nodes retain their scope selector; inspect 320px/390px light and dark layouts
  for 44px controls, safe-area padding, reduced motion, and zero overflow.
- Spot account ownership: isolate the spot template and assert Orders and
  History route only to `spot|history`, the current Holdings marker is
  non-interactive and labels the wallet region, every wallet state branch is
  inside that region, and no `positions` route or order-query/cancel import is
  present. Assert current-pair/View-all copy, the exact base/quote positive-total
  filter, 44px actions, and the `1 + 48 + 34 + 198 = 281` geometry. Preserve the
  prior Pencil digest through exact, unique normalization of only the approved
  account-workspace and order-type changes.
- Loan collateral picker: assert the collateral application contains no native
  `select`; trigger and rows bind `WalletAccount.logoUrl`; the Teleported sheet
  exposes labelled dialog semantics, `aria-pressed`, current-option initial
  focus, Escape/backdrop/close dismissal, Tab wrap, shared body lock, focus
  restoration, localized empty state, 44px targets, safe-area padding, and no
  horizontal overflow in both themes. Preserve the exact `assetId`, collateral
  amount validation, and `applyLoan` payload assertions.
- Loan access hierarchy: assert production source contains no
  `loan-access-pencil` summary/icon selectors or retired readiness locale keys;
  the guest-only login CTA remains at least 48px and retains `openLogin()` with
  the `/products/loan` redirect.
- Canvas theme behavior: switch both the stage class and root `data-theme`,
  assert the renderer receives the new background/text/grid/series colors,
  and assert no theme callback runs after component unmount.
- Offline: load the production preview once, stop the server, and reload the
  current route successfully from the service worker.
- Native: Android Debug APK after build-contract changes; iOS simulator or
  device archive when the installed Xcode SDK supports the configured target.

## 7. Wrong vs Correct

### Wrong

```ts
VitePWA({ runtimeCaching: [{ urlPattern: /\/api\// }] })
navigator.serviceWorker.register('/sw.js')
messages.value.push({ title: 'Your withdrawal succeeded' })
```

### Correct

```ts
VitePWA({ runtimeCaching: [], strategies: 'generateSW' })
if (__PWA_ENABLED__ && !isTauriRuntime()) void initializePwa()
messages.value = await fetchNews(40)
```

For the spot shell boundary and field focus:

```vue
<!-- Wrong: duplicates the root brand header and gives the child input a ring. -->
<RootHeader v-if="showBottomNav" />
<input class="spot-field" />

<!-- Correct: the spot route owns its header; the complete shell owns focus. -->
<RootHeader v-if="showRootHeader" />
<label class="spot-field-shell"><input /></label>
```

For the spot order-type picker:

```ts
// Wrong: one trigger click silently mutates financial form state.
function toggleSpotOrderType(): void {
  orderType.value = orderType.value === 'limit' ? 'market' : 'limit'
}

// Correct: opening preserves state; an explicit option commits the choice.
function openSpotOrderTypeSheet(): void {
  if (confirmOpen.value) return
  spotOrderTypeOpen.value = true
}
function selectSpotOrderType(type: 'limit' | 'market'): void {
  orderType.value = type
  spotOrderTypeOpen.value = false
}
```

For the spot account workspace:

```vue
<!-- Wrong: wallet holdings are visually owned by Orders and expose a fake action. -->
<button class="active">{{ t('trade.orders') }}</button>
<button @click="openOrders('positions')">{{ t('orders.positions') }}</button>
<button @click="cancelAllSpotOrders">{{ t('orders.cancelAll') }}</button>
<WalletBalances />

<!-- Correct: Orders delegates to its authoritative route; wallets own Holdings. -->
<button @click="openOrders('spot')">{{ t('trade.orders') }}</button>
<span id="spot-holdings-label" aria-current="true">{{ t('orders.positions') }}</span>
<section aria-labelledby="spot-holdings-label">
  <button @click="openAssets">{{ t('common.viewAll') }}</button>
  <WalletBalances />
</section>
```

For Loan collateral selection:

```vue
<!-- Wrong: native selection cannot show the server-owned image and may tempt
     callers to derive presentation metadata from a symbol. -->
<select v-model="collateralAssetId">
  <option v-for="account in accounts" :value="account.assetId">
    {{ account.symbol }}
  </option>
</select>

<!-- Correct: opening is presentation-only; explicit option selection commits
     the real wallet asset ID and the server-owned logo stays optional. -->
<button type="button" aria-haspopup="dialog" @click="openCollateralPicker">
  <AssetMark v-if="selectedCollateral" :symbol="selectedCollateral.symbol" :src="selectedCollateral.logoUrl" />
</button>
<button v-for="account in accounts" @click="selectCollateralAsset(account)">
  <AssetMark :symbol="account.symbol" :src="account.logoUrl" />
</button>
```

For imperative chart theming:

```ts
// Wrong: misses the prototype stage class and leaves a white chart in dark UI.
observer.observe(document.documentElement, {
  attributes: true,
  attributeFilter: ['data-theme'],
})

// Correct: one observer covers both token-selection boundaries and is cleaned up.
const stage = canvas.closest('.app-stage')
if (stage) observer.observe(stage, { attributes: true, attributeFilter: ['class'] })
observer.observe(document.documentElement, {
  attributes: true,
  attributeFilter: ['data-theme'],
})
onUnmounted(() => observer.disconnect())
```

For selected-page theme ownership and Product Hub cascade:

```vue
<!-- Wrong: the scoped transform may swallow the html-rooted dark selector. -->
<style scoped>
:global(html[data-theme='dark']) .wallet-pencil-page { background: #000; }
.product-hub { display: grid; gap: 14px; }
</style>
```

```css
/* Correct: cross-boundary theme rules are global; local composition wins later. */
/* pencil-selected-pages.css */
html[data-theme='dark'] .wallet-pencil-page { background: #000000; }
html[data-theme='dark'] .app-stage .mobile-canvas .product-hub {
  background-color: #000000;
  background-image: none;
}

/* ProductHubView.vue <style scoped> */
.product-hub { display: block; gap: 0; }
```

## 8. Production Background and Route Motion

### 1. Scope / Trigger

Apply this contract when changing `App.vue`, root-route classification,
`SignalField.vue`, route-transition CSS, or any route whose shell tier or
expressive/protected surface classification changes.

### 2. Signatures

```ts
type RouteDirection = 'forward' | 'back' | 'still'
type RouteTransitionTier = 'root' | 'secondary'

const ROOT_ROUTE_ORDER = [
  'home', 'markets', 'spot', 'contract', 'assets', 'profile',
] as const

resolveRootRouteKey(
  routeName: unknown,
  mode: unknown,
  purpose?: unknown,
): RootRouteKey | null

classifyRootRouteDirection(
  from: RootRouteKey | null,
  to: RootRouteKey | null,
): RouteDirection

updateRouteTransition(
  toDepth: unknown,
  fromDepth: unknown,
  toRoot?: RootRouteKey | null,
  fromRoot?: RootRouteKey | null,
  routeChanged?: boolean,
): void
```

`SignalField.vue` accepts only `light?: boolean`. Its runtime constants are
`MAX_SIGNAL_DPR = 2`, `MAX_SIGNAL_PIXELS = 2_200_000`, 28 deterministic
particles, four waveform lines, and a 34px grid.

### 3. Contracts

- Mount `.ambient-layer > .signal-field-shell > canvas.signal-field` only on
  expressive Home, Markets, Assets, and Profile root pages. Never mount it on
  Spot, Contract, Seconds, `markets?purpose=trade`, or secondary pages.
- Keep the static fallback below the Canvas. Canvas internal pixels use the
  minimum of device DPR, DPR 2, and the total-pixel cap ratio.
- Preserve the approved light/dark waveform colors, fixed reduced-motion
  timestamp, pointer easing, resize coalescing, hidden-document pause, and
  complete listener/animation-frame cleanup.
- Root-to-root navigation uses the six operational root keys, a
  continuously rendered re-keyed `.route-veil-root`, and
  `route-forward|back` plus `transition-root`. The visual bottom navigation
  exposes five entries because its raised Trade action restores either the
  spot or contract key. Seconds resolves to no root key and uses
  `transition-secondary`, whose veil is hidden.
- Keep the incoming route component as the `.view-stack`. Do not put
  `.view-stack` on the persistent route host, because that traps sticky headers
  in the wrong stacking context.
- Shell layers use the shared order content 0, route transition 30, root
  navigation 40, sticky root/secondary header 70, overlay 80, and launch 120.
  This order applies even if Vue leaves an entering-route class attached longer
  than expected. Root navigation and its items remain hit-testable while shaped
  pseudo-elements, ambient layers, and veil layers use `pointer-events: none`.

### 4. Validation & Error Matrix

| Condition | Required behavior |
|-----------|-------------------|
| Canvas box is zero-sized | Skip buffer allocation until a valid resize |
| Total internal pixels would exceed 2,200,000 | Lower the effective DPR |
| `prefers-reduced-motion: reduce` | Draw timestamp 1800 once; schedule no loop; hide route veil |
| `document.hidden` | Cancel the current frame; resize and restart when visible |
| Home → Markets / Markets → Home | `forward/root` / `back/root` |
| Root → secondary / secondary → root | `forward/secondary` / `back/secondary`, no root veil |
| Raised Seconds action | Resolve no root key; secondary tier, no root veil |
| `markets?purpose=trade` | Protected secondary tier despite the `markets` route name |
| Component unmount | Cancel animation/resize frames and remove every listener |

### 5. Good / Base / Bad Cases

- Good: 390x844 Home has one viewport-sized Canvas whose internal buffer
  follows capped DPR, and two normal-motion frames differ.
- Base: direct-open Seconds has no ambient Canvas, no bottom navigation, and
  retains the existing secondary PageHeader and real API states.
- Bad: including Seconds in `ROOT_ROUTE_ORDER` plays a root veil from the
  raised center action even though the prototype treats it as a secondary
  route.
- Bad: classifying `markets?purpose=trade` as a root Markets switch plays a
  360ms veil over the pair picker.
- Bad: applying `.view-stack` to `.app-route-host` lets its stacking context
  trap an entering sticky header below navigation.

### 6. Tests Required

- Unit: exact six-item motion root order, route-key resolution including
  Spot/Contract, the Seconds and market-picker secondary exceptions,
  forward/back/still classification, and tier state.
- Source/runtime contract: Canvas colors, drawing primitives, DPR/pixel caps,
  reduced-motion timestamp, visibility/resize handling, and cleanup symmetry.
- Shell DOM: ambient mount allowlist, continuously rendered keyed route veil,
  exact `route-* transition-*` classes, and no ambient Canvas on protected
  surfaces.
- Browser: 320x720, 360x745, 390x844, and 448x900; both themes; no horizontal
  overflow; 44px targets; Canvas frame change; root directions; secondary
  sticky-header z-index; `document.elementFromPoint` over every bottom-nav item
  resolves to the nav/item even during a route enter; clean console.
- Build: `npm run type-check`, `npm test`, `npm run build:pwa`,
  `npm run build:tauri`, and Android aarch64 debug APK.

### 7. Wrong vs Correct

#### Wrong

```vue
<div class="app-route-host view-stack">
  <component :is="Component" />
</div>
```

```ts
resolveRootRouteKey('markets', undefined) // used for the trade pair picker too
resolveRootRouteKey('seconds', undefined) // incorrectly treated as a motion root
```

#### Correct

```vue
<div class="app-route-host">
  <component
    :is="Component"
    :class="['app-route-layer', 'view-stack', ...routeMotionClasses]"
  />
</div>
```

```ts
resolveRootRouteKey('markets', undefined, 'trade') // null: secondary tier
resolveRootRouteKey('seconds', undefined) // null: raised secondary action
```

## Launch Intro Contract

- The production mobile shell may mount one decorative `LaunchIntro` above all
  shell layers. It must use the official GSAP package and the existing HIPPO
  brand asset rather than a duplicated animation engine or remote media.
- Use a versioned `sessionStorage` key so the intro plays once per app session.
  Mark the session when playback starts; route changes must never replay it.
  Storage access failures must not block the application.
- The application, router, authentication, stores, and API requests initialize
  behind the intro without awaiting it. The intro is visual presentation only,
  uses `aria-hidden="true"`, and adds no focusable controls.
- Normal motion lasts about two seconds and exits by revealing the live route.
  `prefers-reduced-motion: reduce` skips immediately. Completion and unmount
  must both kill the GSAP timeline, revert its context, and remove scroll lock.
- The fixed launch layer must use the highest shell layer, cover safe areas,
  avoid horizontal overflow from 320px through 448px, and remain independent
  of light/dark application surfaces.

## Root Scroll Boundary Contract

- The document remains the root vertical scroll owner. Apply
  `overscroll-behavior: none` to both `html` and `body` so browser and PWA
  surfaces do not stretch or chain at the top and bottom boundaries.
- CSS scroll-boundary policy does not replace the Android host policy.
  Android 12+ WebView may keep `scrollY` clamped while its native EdgeEffect
  temporarily stretches the composed page. The Tauri `MainActivity` must
  override `onWebViewCreate`, call its parent implementation, and set
  `webView.overScrollMode = View.OVER_SCROLL_NEVER`.
- `src-tauri/gen/` is generated and ignored. Keep the application-owned
  `MainActivity` template under tracked source and synchronize it before
  Android build/dev commands and after a successful Android init. Never make a
  generated Wry or Tauri base class the source of truth for this policy.
- Do not replace the root policy with global `touch-action`, JavaScript
  `touchmove` cancellation, fixed document heights, or `overflow-y: hidden`.
  Normal vertical and inertial scrolling, sticky headers, input interaction,
  and chart gestures must remain browser-owned.
- Nested sheets may keep their local `overflow-y: auto` and
  `overscroll-behavior: contain` contracts. The root rule must not convert
  those workflow surfaces into document scrolling.
- Android verification must inspect the compiled `MainActivity` callback and,
  when a device is available, test the visual state during an in-progress
  boundary drag. Checking only the final JavaScript `scrollY` is insufficient.
