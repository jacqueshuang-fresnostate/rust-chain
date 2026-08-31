# Mobile PWA, Theme, and Application Shell Contract

## 1. Scope / Trigger

Apply this contract when changing `mobile/` build modes, Vite public base,
manifest metadata, service-worker behavior, install/update prompts, theme
tokens, the root application shell, bottom navigation, or the announcement
message center. It also applies when a selected Pencil secondary page needs a
theme selector or canvas override that crosses a Vue SFC scoped-style boundary,
or when Loan changes its collateral-asset selection surface, or when Assets
changes the Pencil-mapped transfer and transfer-asset-picker sheets.

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

type MarginOrderType = 'market' | 'limit'
openContractSheet(sheet: 'pair' | 'leverage' | 'marginMode' | 'orderType'): void
selectContractOrderType(type: MarginOrderType): void
fillContractLimitPrice(): void
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

Assets transfer-picker signatures in `AssetsView.vue`:

```ts
transferAssetPickerOpen: Ref<boolean>
filteredTransferAccounts: ComputedRef<WalletAccount[]>
openTransferAssetPicker(): void
closeTransferAssetPicker(): void
selectTransferAsset(account: WalletAccount): void
fillTransferAvailable(): void

type AssetAccountScope = 'all' | 'spot' | 'margin'
assetAccountScope: Ref<AssetAccountScope>
spotHoldingRows: ComputedRef<AssetHoldingRow[]>
marginHoldingRows: ComputedRef<AssetHoldingRow[]>
selectAssetAccountScope(scope: AssetAccountScope): void
```

Turnstile SPA lifecycle signatures in `src/core/turnstile.ts`:

```ts
type TurnstileWidgetId = string | number
loadTurnstileApi(): Promise<TurnstileApi>
createTurnstileLifecycle(options?: TurnstileLifecycleOptions): {
  render(request: TurnstileRenderRequest): Promise<TurnstileWidgetId | null>
  reset(): boolean
  remove(): void
  getWidgetId(): TurnstileWidgetId | null
}
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
- `PwaStatus` is a non-modal system island, not a viewport backdrop or bottom
  sheet. Its fixed root starts below the 64px application Header plus the top
  safe area, stays within the 448px application canvas, leaves detached side
  margins, and keeps `pointer-events: none`. Only each visible status card and
  its controls restore pointer events; the component must not lock body scroll
  or prevent the page outside a card from receiving pointer input.
- The Seconds settlement result is a body-Teleported modal dialog matching the
  selected Pencil frames `tFcTH` and `FBdqS`. Its fixed root covers the viewport,
  owns the light/dark backdrop, locks body scrolling through `useModalDialog`,
  traps focus, closes with Escape or backdrop selection, and restores focus.
- At 390×920 the dialog is 358×541 at x=16/y=176 with a 24px radius, 20/20/18
  padding, and 14px vertical gaps. The child geometry is status 34px, result
  176px, price comparison 68px, order summary 64px, note 39px, and History
  action 52px. The visible Close surface is 34px inside a 44px touch target.
- The dialog uses Lucide `CircleCheckBig`, `BadgeDollarSign`, `ArrowRight`,
  `Info`, `History`, and `X`. It renders only authoritative final-order fields:
  signed net profit/loss and percentage, entry and settlement prices, pair,
  direction, cycle, stake amount, and settlement asset. Market/K-line fallbacks
  must never replace missing settlement prices.
- Because the modal Teleports outside `.seconds-page`,
  `pencil-selected-pages.css` owns its exact Pencil light/dark palette on the
  `data-pencil-source="tFcTH FBdqS"` boundary. Theme changes update those CSS
  variables in place without remounting or replaying a queued result. Reduced
  motion removes the reveal transition while preserving final geometry.
- Each PWA state uses the same double-bezel structure: an outer
  `.pwa-status__card` supplies semantic ambient light and an inner
  `.pwa-status__panel` supplies the translucent, blurred surface and inset
  highlight. Use existing theme tokens with `color-mix`, Lucide icons, and the
  state mapping accent=install/update, positive=offline-ready, and
  negative=offline/error. Do not reintroduce the retired `#0b1811` family,
  remote art, emoji, or a second icon library.
- The live region remains polite and non-atomic. Offline may render together
  with exactly one primary card; the primary priority remains update, install,
  offline-ready, then error. Busy update/install/retry states expose
  `aria-busy`, disable duplicate actions, and every button/dismiss target is at
  least 44x44px with a visible focus ring.
- `pwa-status-reveal` may animate only opacity, transform, and presentation
  blur through the project motion curve. At 320px its actions may wrap but the
  document must not overflow horizontally. Under `prefers-reduced-motion`, all
  entry, spinner, and decorative breathing motion is disabled.
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
- Market charts use one npm/Vite-bundled renderer: `lightweight-charts@5.2.0`.
  PWA, Tauri, and Android artifacts contain that local package and must not load
  chart scripts, frames, widgets, Pro modules, or market data from a remote
  runtime. Keep the official `attributionLogo` enabled at chart creation so its
  built-in TradingView link satisfies the library attribution requirement.
- `MobileMarketChart` mounts exactly one `LightweightMarketChart`; do not expose
  an engine selector or persist an engine preference. The renderer accepts the
  normalized symbol plus interval as its dataset key. A key change marks the
  next dataset for fitting but waits for a new `points` value before rendering
  or fitting, so changing symbol at the same interval cannot reuse the old
  viewport or fit stale candles.
- The chart container is an accessible `region` or `group`, not an image. The
  built-in attribution link must retain its own interactive semantics.
- Shared text uses stable pixel sizes and `letter-spacing: 0`; do not scale
  font size with viewport width.
- The selected visual bottom navigation has exactly five entries in this order:
  Home, Markets, Trade, Assets, Profile. Trade is the raised center action and
  opens the selected `X0ux9F` chooser; it does not navigate before an explicit
  option selection. This visual consolidation must not merge the independent
  spot, contract, seconds, or convert business routes. Seconds stays reachable
  from the selected Home shortcut, chooser, and direct route while using the
  secondary motion/back contract. All dock icon targets remain at least 44x44px
  from 320px through 448px.
- The raised Trade face is one uninterrupted 56px mint circle with a 24px
  Lucide ArrowLeftRight positioned 12px above the 68px Dock surface. Its
  computed `background-image` must be `none`.
  Earlier legacy active-item selectors must explicitly exclude
  `.trade-nav-action`; otherwise their centered 28px gradient wins by
  specificity and turns the circle into a square color patch.
- The selected root trade chooser Teleports to `body` at overlay layer 80. At
  the 390px baseline it owns a 35% black viewport mask, the exact Pencil path
  in a 358x300 surface, four 330x58 rows at x30/y12 with 4px gaps, and a 54px
  circular Close control at viewport bottom 35px plus the safe area. The path
  bottom stays 4px above the Dock top; its centered concave notch surrounds the
  Close control. At 320px the surface is `viewport - 32px`; at 448px it remains
  capped at 358px. Both sizes must keep zero horizontal overflow.
- The chooser reuses `useModalDialog`: Close receives initial focus, Tab wraps
  through all four real options and Close, Escape/backdrop dismiss without route
  mutation, body scrolling stays locked only while open, and focus returns to
  the exact Trade trigger. The four destination buttons do not expose a current
  or `active` option state. Dark theme maps the surface/text roles without
  changing geometry; reduced motion removes every picker transition.
- The selected Pencil spot references are `yzOPc` (light) and `bo8k5` (dark).
  Their production default is a 64px spot-owned secondary header, a continuous
  left order form plus right 148px five-ask/mid/five-bid book, a truthful account
  state, and a collapsed local-chart entry. The spot route keeps the five-entry
  dock but must not mount `RootHeader`; `RootHeader` is limited to Home,
  Markets, Assets, and Profile. Margin/contract mode owns the separately
  selected `cjzfi` (light) and `p6GfgT` (dark) workspace and must not be merged
  into the Pencil spot branch.
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
  contract. Activating contract mode closes any open spot order-type sheet;
  contract order type is a separate backend-capability-driven state and never
  reuses or mutates the spot selection.
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
- Assets Transfer also owns the saved asset-picker pair `tPkL1/tPkD1`. The
  production root declares both the transfer and picker pairs. The main sheet
  is a 520px, three-row bottom sheet with a 30px numeric amount hero, one 52px
  glass route bar, one flat 52px asset row, a truthful wallet hint, and a 50px
  mint action. The asset row opens the picker inside the same dialog owner;
  the picker contains a glass search field, real wallet rows, current-option
  state, and a bottom wallet-source hint. Do not restore the native asset
  `select` or the old outlined From/To/Asset/Amount field stack.
- The Transfer overlay Teleports to `body`, so it no longer inherits selected
  Pencil variables declared on `.pencil-page`. Redeclare the exact light/dark
  structural, text, mint, line, and shadow roles on the Teleported sheet before
  consuming `--surface-2`, `--accent`, or related tokens. Relying on ambient
  body/prototype variables can render a dark hero and close face in light mode.
  The desktop stage positions the layer on the 448px mobile canvas; viewports
  at 820px or below use the full viewport width.
- Transfer logos and balances remain wallet-owned API values. `AssetMark`
  receives each `WalletAccount.logoUrl`; missing accounts and balances render
  `--`, not fabricated values. USDT is preferred only when it exists in the
  current source-account response. Search filters the current source list and
  never creates an asset row.
- Shared `AssetMark` presentation has two explicit states. A successfully
  loaded backend image is only clipped to a circle: it has no generated
  highlight, gradient, border, inner ring, shadow, padding, or symbol-hash
  color. The hash palette is presentation-only for the exact symbol-initial
  fallback after all backend image candidates are absent or fail; that fallback
  remains a flat themed circle and its typography scales within the existing
  24–54px geometry. Page-local selectors may position or flex the mark but must
  not add a product-accent ring or decorative material around the real image.
  Vue forwards a parent's scoped-style attribute to a child component root, so
  contract position badge material must target the explicit
  `.contract-position-badge` class rather than a broad
  `.contract-position-identity span` descendant that also matches the
  `AssetMark` root `<span>`.
- The authenticated Assets canvas stacks two full-width account cards for
  `现货账户` and `杠杆账户`, plus an explicit `全部账户` reset. Each card shows its
  own API-derived USDT estimate and positive-holding count; selecting it filters
  the holdings list without changing the hero's all-account estimate or making
  another request. A zero margin balance uses the dedicated transfer empty
  state and never disappears merely because no lazy wallet row exists yet.
- Opening the asset picker focuses search. Selecting or dismissing it restores
  focus to the asset trigger; Escape closes the picker before it can close the
  parent transfer dialog. The close, route-swap, and All controls retain 44px
  pointer targets even when their Pencil visual faces are 28–32px. Main and
  picker bodies contain overscroll while the action/hint remains visible.
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
- The Trade confirmation layer Teleports to `body` for both modes. Spot keeps
  its existing generic confirmation content; contract alone renders
  `.contract-order-confirm` with the approved 22px top radius, 38x4 grab,
  19px title, 44x44 circular close control, neutral data cards, warm risk
  notice, and 48px mint action. Its three-row grid keeps the combined
  grab/header, scrollable detail region, and action/error region separate so a
  failed submission stays visible without moving the safe-area action
  off-screen.
- The Teleported contract review owns explicit light and dark structural,
  text, line, mint, negative, and warning roles rather than inheriting tokens
  from `.contract-trade`. It remains at most 448px wide, uses dynamic viewport
  and all four safe-area insets, removes entry/press motion under reduced
  motion, and has a 320px composition that never introduces horizontal scroll.
- Contract review initially focuses its close control. Overlay, close, and
  Escape dismissal call the same guarded close path; Tab remains contained,
  body scroll is locked, and focus returns to the exact long/short trigger.
  While submitting, every dismissal target is disabled and a no-focusable Tab
  attempt remains on the dialog container.
- The contract order-type control is an enabled dialog trigger only when the
  current backend product advertises at least one recognized type. Its
  Teleported fourth contract sheet renders only those options, initially
  focuses the selected option, commits only explicit row clicks, and shares the
  existing scroll-lock, Escape, backdrop, close, focus-return, safe-area,
  light/dark, 44px, 320px, and reduced-motion contracts.
- Contract market price remains read-only. Contract limit price is an editable
  plain decimal field with a 44px BBO action: long uses best ask, short uses best
  bid, and only an absent side falls back to the latest ticker. Invalid or
  over-precision input has a visible localized error and cannot open review.
- The selected margin main frame is measured at 390px against the currently
  selected Pencil modules `IpirH/mcfEf`: a safe-area-aware 58px sticky header,
  a 500px module with `14 / 202 / 10 / 150 / 14` horizontal geometry, a 490px
  order console/book, and a 44px local workspace tab row.
  Header order is back, backend Logo plus pair/status/live quote, chart, and
  more; favorite lives in the more menu rather than consuming header width.
  The 390px geometry is exact, while widths above 390px keep the 202px form
  column and let the live book consume the remaining space; widths below 360px
  use the dedicated compact columns rather than clipping either side.
- The 202px margin console follows the selected absolute track: open/close
  `top 0 / h30`; two equal margin-mode/leverage triggers `36 / 38` with a 6px
  gutter; the independent order-type trigger `80 / 40`; price+BBO `126 / 54`;
  margin `186 / 48`; percentage `240 / 32`; available `278 / 13`; TP/SL
  `297 / 16`; long summary `319 / 28`; long action `353 / 42`; short summary
  `401 / 28`; and short action `435 / 42`. The parallel 150px book renders six
  asks, latest price, seven
  bids, B/S ratio, and precision using the existing live detail session. The
  ratio derives both sides from the rendered live levels, keeps their rounded
  sum at `100%`, and uses a compact two-row composition: semantic B/S values
  above one continuous split strength rail. Do not restore two disconnected
  short lines or decorative ratios unrelated to the current order book.
- Contract percentage is one native continuous range from `0` through `100`
  in `1%` steps. Its visual rail stays 32px high while the input owns a
  44px-high pointer/focus area; only one movable thumb is rendered, with no
  discrete interval dots or stop buttons. The current percentage remains
  visible and is announced by the range semantics. Manual input clears the
  slider selection and retains the localized min/max range, `aria-invalid`,
  `aria-errormessage`, visible field border, and announced failure reason.
- The local workspace defaults to Positions/Assets and keeps pending limit
  orders separate. Positions show backend Logo, direction/mode/leverage,
  service risk metrics, current-pair filtering, two-step single/bulk close, and
  a history route. Unsupported strategy and TP/SL states stay explicitly
  disabled. Batch HTTP success still inspects backend failures before showing
  an all-success message.
- While this margin workspace is mounted and authenticated, it owns one user
  private socket at `/api/v1/ws/private?token=<access-token>`. The server binds
  the user channel, so the client sends no subscribe command. Socket open,
  reconnect, and `margin.position.liquidated` are silent refresh hints only:
  they trigger `/margin/wallets` reconciliation and never edit balances or
  positions from event fields.
- A visible contract workspace also performs a five-second, single-flight REST
  reconciliation and one immediate refresh after returning to the foreground.
  A successful account snapshot replaces wallets and opened positions together
  before surviving risk rows refresh; a background failure keeps the last
  successful surface without a loading/error flash. Guest, spot, hidden, and
  unmounted states start no new account request.
- Private transport reconnects with the latest persisted token, bounded
  exponential backoff, heartbeat, and current-socket identity guards. Logout,
  account/token replacement, spot switching, and unmount close the socket and
  clear heartbeat/reconnect work; stale REST results from any prior lifecycle
  cannot write back after a contract/spot ABA transition.
- Selected, `:focus-visible`, pressed, disabled, and reduced-motion states stay
  structurally distinct in both themes. Programmatic scroll-to-positions uses
  `auto` under reduced motion; state feedback must not move the fixed module
  geometry or alter the selected Spot template. The Header more menu moves
  focus to its first item, supports Arrow/Home/End navigation, closes on
  Escape, and restores focus to the trigger; a translucent menu without those
  keyboard semantics is not an acceptable visual-only implementation.
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
- Turnstile API loading is module-scoped and reuses the exact explicit-render
  script URL. A dynamically created classic script must set `async = false`
  and `defer = false` before insertion, then wait for `turnstile.ready()` before
  rendering. If a host page already supplied an async/defer script and the API
  is available, reuse the API directly instead of calling `ready()` through an
  unsupported loading pattern. A failed loader clears its cached Promise and
  removes the failed script so a later login attempt can retry.
- Every explicit render owns a monotonically increasing generation. Validate
  the generation, `container.isConnected`, and the caller's current-container
  predicate after resolving the container, after loading the API, and after
  `render()` returns. `remove()` invalidates the generation before removing the
  current widget; every token, expiry, error, timeout, and interactive callback
  must carry the same current-generation guard.
- Unmounting, disabling Turnstile, entering the two-factor route, or rebuilding
  for theme/language changes removes the old widget before another render. The
  script remains singleton and reusable. Mobile widget language values use
  Cloudflare-supported casing: `en` or `zh-cn`; application locale IDs remain
  unchanged.
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
- The selected Seconds references are `VL8er` (light) and `g9agt` (dark), and
  `/seconds` records them as `data-pencil-source="VL8er/g9agt"`. Production does
  not duplicate the Pencil status bar: its visual canvas starts with a 60px
  Header, followed by a fixed 420px trading operation and an order workspace
  starting at `y=480` with a 362px minimum height. The Header keeps 40px left
  and right controls, a visually absolute-centered 140x22 pair/title track, and
  the named Seconds-history route on the right action.
- The Seconds pair-picker references are `vONcc` (light) and `kLXCs` (dark).
  The 140x22 Header identity sits inside one real 44px dialog trigger; never
  restore the transparent native `select`. Opening it Teleports a labelled
  modal root to `body`, focuses search, traps Tab, closes with Escape/backdrop/
  Close, locks body scrolling, and returns focus to that exact Header trigger.
- At 390x920 the pair-picker covers the viewport and owns a 390x840 sheet at
  `y=80`, with 24px top corners, `18px 20px 16px` padding, and 14px content
  gaps. Its title row is 350x34, visible Close face is 34px inside a 44px
  target, search is 350x46, and every product row is 350x64 with an 8px gap,
  30px backend Logo, formatted pair, current price, and Lucide Check only on
  the selected product. The sheet uses the exact selected light/dark palette
  from the global selected-page stylesheet because its root is outside the
  route theme boundary.
- Search filters only the current API product collection by raw pair,
  formatted pair, base, or quote without changing the active product. A row
  selection calls the existing product switch, closes the dialog, and leaves
  all current orders untouched. Loading, product-empty, and no-result states
  are localized and truthful. A long list scrolls inside the list with
  contained overscroll; 320px short screens contract safely, 448px centers the
  390px sheet, and reduced motion removes the reveal without changing geometry.
- At the 390px reference width, every Seconds content track is 350px with 20px
  side insets. The operation grid is `22px 53px 112px 202px` with 6px row gaps;
  the form grid is `30px 26px 38px 40px 44px`, also with 6px gaps. The chart
  remains 112px high, the active-order cards are 350x82 with 8px gaps, and the
  list grows naturally rather than clipping a fourth order. At narrower and
  wider widths these tracks remain fluid and never create document overflow.
- Seconds uses the selected flat white/pure-black canvas without the retired
  prototype grid. Light tokens are `#ffffff`, `#111714`, `#68736d`, `#dde4e0`,
  `#d9f9eb`, `#087b52`, `#43efa9`, and `#ff654a`; dark tokens are `#000000`,
  `#050806`, `#0c100e`, `#f2f7f4`, `#95a19a`, `#202923`, `#103326`, `#61f1b6`,
  `#43efa9`, and `#ff654a`. These route tokens live in the shared global
  selected-page stylesheet; scoped CSS owns only Seconds layout and states.
- Seconds period and filter controls intentionally have 30px visible geometry,
  and the heading action has 24px visible geometry. Explicit local `height`,
  `min-height`, and, where needed, transparent expanded hit areas must defeat
  legacy global button minimums without shrinking accessible pointer targets.
  Browser validation must inspect computed dimensions, not only source rules.
- The dedicated Seconds-history references are `vZy6U` (light) and `x29z7`
  (dark). At 390px its title and filter stay on the 16px inset track as a
  358x52 header and 358x38 direction-filter row, while list content starts at
  `x=0, y=134`; every subsequent card starts 156px later. Each filter button is
  a real 44px target starting at `y=82` and contains a top-aligned 33px pill
  surface with 16px radius and `7px 16px` padding; the Chinese active pill is
  59px wide.
- Seconds-history cards fill the current 320–448px phone canvas. At the 390px
  reference they are exactly `x=0, width=390, height=142`, with no radius,
  border, or shadow, `14px 16px` padding, and 8px internal gaps. The 16px card
  inset returns visible content to the same 358px text track as the title and
  filters. On that inner track the Chinese detail row measures direction
  `x=0/w=27`, status `x=140/w=40`, and time `x=293/w=65`; intrinsic edge
  columns plus a centered status in the flexible middle track own this geometry,
  not a fixed-width direction column. Keep 27px/40px item minima on the Chinese
  direction/status text so an available fallback font cannot shave either
  measured face by one pixel. Their three visible lines are pair plus page-specific compact
  duration and result-derived signed P&L; direction, authoritative lifecycle
  `status`, and API creation time; then stake, entry price, and settlement
  price. The status line never substitutes final `result`, missing API values
  stay `--`, and visible card values inherit the page Noto/PingFang family
  rather than the global `.numeric` font. Chinese history duration has no space
  (`{seconds}秒`).
- Pencil controls geometry and visual hierarchy, while products, cycles,
  limits, prices, K-lines, balances, Logos, active orders, countdowns, and
  results remain authoritative API/WebSocket data. Empty/loading/guest/error
  states preserve the selected tracks without inserting Pencil sample values.
  The current ticker percentage uses a valid live value first and otherwise
  falls back to the current market snapshot instead of flashing unavailable
  when a price-only live frame arrives.

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
| Offline and one primary PWA state are both active | Stack the offline island with update > install > offline-ready > error; do not collapse or reorder the truth states |
| Pointer is outside a visible PWA card | Let the underlying application receive the pointer; do not add a backdrop, body scroll lock, or full-root hit target |
| PWA action is running | Set `aria-busy`, disable its action, and preserve a 44px target without layout shift |
| Viewport is 320px or reduced motion is requested | Wrap actions without horizontal overflow; remove entry, spinner, and decorative breathing animations |
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
| Root Trade control is selected | Open the X0ux9F chooser above the Dock; preserve the current URL until explicit selection |
| Root chooser is open at 390x844 | Surface x16/y462, path bottom y762, Dock top y766, Close x168/y755, and zero horizontal overflow |
| Root chooser is dismissed | Restore body overflow and exact Trade-trigger focus; preserve the previous route/mode |
| Spot market stream has no rows yet | Show a truthful loading/unavailable state; never synthesize book or trade rows |
| Nested spot input receives focus | Apply one ring to `.spot-field-shell`; child input keeps `box-shadow: none` |
| Spot order-type trigger is selected | Open the Teleported sheet without changing `orderType` |
| Spot order-type option is selected | Set that exact value, close the sheet, and retain the existing price/submission contract |
| Spot order-type sheet is dismissed or contract mode activates | Close without changing the spot selection; contract uses its own advertised selection |
| Spot order-type and confirmation dialogs compete | Keep them mutually exclusive and restore the body overflow/focus owner exactly once |
| Contract order-type trigger opens | Preserve the current value and render only backend-advertised market/limit options |
| Contract order-type sheet is dismissed | Keep the current value; restore body scroll and exact trigger focus |
| Explicit contract limit is selected | Commit limit, enable the price field, and offer long-ask/short-bid/latest fill |
| Contract capability refresh removes the selection | Prefer advertised limit, otherwise first advertised type, otherwise disable review |
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
| Swap direction control is activated | Immediately exchange the API-owned pay/receive assets and Logos, keep amount, clear the old quote/messages, and remain within the viewport |
| Seconds Header pair trigger is selected | Open the `vONcc`/`kLXCs` body-Teleported picker, focus search, lock body scroll, and keep the route unchanged |
| Seconds pair search has no match | Render the localized no-result state; preserve the selected product and every active order |
| Seconds pair row is selected | Switch product/K-line through the existing selection path, close the picker, restore Header focus, and do not mutate active orders |
| Seconds pair picker is dismissed | Preserve product and search-independent trading state, unlock body exactly once, and restore exact trigger focus |
| Seconds history renders a resolved order | Match `vZy6U`/`x29z7`: 52px header, 38px filter track with `y=82` 44px targets around 33px pills, and full-canvas 142px three-line cards with 16px content insets and no radius/border/shadow; show lifecycle `status` only while deriving signed P&L from final `result`, with unknown values neutral `--` |
| Seconds settlement result is visible | Cover the viewport with the selected Pencil backdrop, expose one labelled modal dialog, lock body scroll, trap focus, and initially focus the 44px Close target |
| Multiple Seconds results settle together | Show one dialog at a time, announce the remaining count for assistive technology, and let Close/Escape/backdrop advance FIFO without losing later results |
| Viewport is 320px or reduced motion is requested for a Seconds result | Keep 16px safe gutters, zero horizontal overflow, one full-width History action, and remove reveal motion without changing final geometry |
| Selected Seconds page renders at 390px | Header is 60px, operation is 420px, orders start at y=480, inner tracks are 350px, and the center title has zero horizontal delta |
| Selected Seconds page renders at 320px or 448px | Fluid tracks stay inside the viewport, period/filter controls retain their visible geometry, and the document has zero horizontal overflow |
| A live Seconds ticker frame omits change percentage | Keep the live price and fall back to the current market snapshot percentage rather than displaying a transient unavailable value |
| More than three Seconds orders are active | Render every matching real order below the 362px minimum workspace; never cap or clip the list |
| Assistive live status is rendered | `.sr-only` remains absolute, clipped, 1x1px, and visually absent |
| Turnstile renders at 320px | Keep a centered 302px stage and 300px challenge viewport within the device width; no decorative wrapper or horizontal scroll |
| Turnstile theme or locale changes | Remove and explicitly re-render the widget with the new app theme/language, clearing the previous token |
| Turnstile reset returns successfully | Keep the existing widget ID and expose the ready state; hard remove only when reset fails |
| Turnstile script is dynamically inserted | Set `async=false` and `defer=false`, wait for API readiness, and keep exactly one explicit-render script |
| An existing async/defer Turnstile script already exposed the API | Reuse that API directly; do not invoke the incompatible `ready()` loading pattern |
| A render resolves after its container was replaced or disconnected | Return `null`, remove any synchronously created stale widget, and ignore every stale callback |
| Login leaves for two-factor or unmounts | Invalidate the generation, remove the widget, and clear the token before navigation or teardown |
| Browser logs one origin mismatch while a successful challenge iframe initializes | Treat it as provider-internal iframe navigation; do not intercept `postMessage`; investigate only repeated/persistent warnings, missing token, or Cloudflare error codes |
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
- Good: an install prompt appears as a detached double-bezel glass island below
  the Header; the user can still scroll and operate the visible page outside
  the island, while the install and dismiss controls remain 44px targets.
- Base: the browser is offline while an update is waiting, so the negative
  offline island and accent update island stack in that order without hiding
  either truthful state or overflowing a 320px viewport.
- Bad: `PwaStatus` becomes a modal backdrop, intercepts the full viewport,
  locks body scrolling, renders a flat full-width band, or animates after the
  user requests reduced motion.
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
- Good: a slow Turnstile load finishes after Login unmounts; no widget renders,
  no token callback mutates state, and returning to Login reuses one script.
- Base: Cloudflare emits one transient target-origin warning while its iframe
  changes from the application origin to the challenge origin, but the current
  widget still returns a valid token and no warning accumulates across routes.
- Bad: each mount appends another API script, an old callback restores a stale
  token, or application code patches `window.postMessage`/`console` to hide a
  persistent lifecycle defect.

## 6. Tests Required

- Unit/source contract: build modes, Tauri double guard, manifest fields,
  `runtimeCaching: []`, denied fallback routes, single `PwaStatus`, safe prompt
  routes, theme normalization, and complete `zh-CN`/English keys.
- PWA status island contract: assert the existing route allowlist and state
  priority, the five double-bezel panels, semantic tone/role/`aria-busy`
  mapping, pointer-transparent root and pointer-active cards, 44px controls,
  focus ring, no retired colors/emoji/remote assets, narrow-screen wrapping,
  custom reveal transition, and complete reduced-motion overrides.
- PWA status browser pass: force install, offline, and stacked states at
  320x720, 390x844, and 448x900 in both themes; assert document width equals
  viewport width, card edges stay detached, actions are fully visible, and
  `elementFromPoint` outside the card reaches the underlying page.
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
- Root trade chooser pass: at 390x844 assert the exact `X0ux9F` mask, path,
  358x300 surface, four 330x58 rows without an active selection state, 54px Close and 4px
  path-to-Dock gap. Repeat at 320x720 and 448x900 in both themes for safe-area,
  zero overflow, initial Close focus, Tab/Shift+Tab wrap, Escape/backdrop close,
  body unlock, trigger focus restoration, reduced motion, and all four real
  route outcomes.
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
- Swap direction browser pass: with a single backend config row, click the 44px
  direction control at 320px and 390px, assert pay/receive symbols and Logos
  exchange, the typed amount stays, the previous quote disappears, a second
  click restores the initial direction, and document width remains the viewport
  width.
- Seconds history Pencil/parity pass: at 320px, 390px, and 448px in both themes,
  assert the `vZy6U`/`x29z7` 52/38/142 hierarchy, `y=82` 44px filter hit boxes
  with nested 33px pill surfaces, and three-line cards with page-owned
  typography. At 390px cards must be `x=0 / width=390 / height=142` at
  `y=134/290/446`; at every tested width they fill the phone canvas, use a 16px
  inner text track, and have no radius, border, or shadow.
  A settled win/loss keeps the localized lifecycle status while its separate
  P&L renders the signed positive net amount/negative stake; a cancelled row
  renders the generic P&L label with `--`. Assert compact no-space Chinese
  duration, API-only prices, and document width equal to viewport width even
  for grouped large amounts.
- Seconds settlement-dialog pass: assert the Teleported root owns the selected
  Pencil backdrop and a labelled `aria-modal` dialog using `useModalDialog`.
  At 390×920 in both themes verify x=16/y=176, 358×541 geometry, the exact
  34/176/68/64/39/52 child stack, API-only entry/settlement prices, signed
  win/loss amount and rate, pair/direction/cycle, FIFO advancement, 44px Close,
  one History action, focus trap/restoration, and body lock. At 320px and 448px
  verify safe gutters and zero horizontal overflow; reduced motion must retain
  final geometry while removing transition.
- Seconds pair-picker pass: assert the Header uses one 44px dialog trigger and
  no native `select`; at 390x920 in both themes verify the `vONcc`/`kLXCs`
  390x840 sheet at y=80, 350x34 header, 350x46 search, 350x64 rows, 30px API
  Logos, live/snapshot prices, selected Check, localized note, focus trap/
  restoration, Escape/backdrop close, and body lock. Search must filter by
  pair/base/quote without mutating selection until a row is chosen. Repeat at
  320x640 and 448x920 for zero overflow, long-list scrolling, and reduced
  motion.
- Selected Seconds parity pass: at 390px in both themes assert the 60/420/362
  vertical geometry, 350px content tracks, 112px chart, 30/26/38/40/44 form,
  82px cards, exact selected tokens, absolute Header centering, and computed
  visible button heights. Repeat at 320px and 448px for zero document overflow,
  real pair switching, confirmation open/cancel, and named History navigation.
- Viewport confirmation sheet: at 320x568, 320x720, 390x667, 390x844, and
  448x900 assert the Teleported overlay is a direct `body` child with no
  transformed route ancestor, every action button rect stays within the
  viewport, and scrolling an overflowing detail region does not move the
  action row. Also exercise Escape, Tab wrap, focus return, body scroll lock,
  both themes, and zero horizontal overflow. Run this for the unchanged generic
  spot branch and the dedicated contract branch; contract additionally asserts
  real Logo/pair/direction/settings/form-derived values, in-panel API failure,
  busy dismissal lock, warm risk notice, and reduced-motion ownership.
- Margin Pencil parity: assert `cjzfi/p6GfgT` plus selected module provenance
  `IpirH/mcfEf`, the exact 390px header/module/console/book/tab geometry above,
  six asks/seven bids, one continuous slider, and backend
  Logo/rate/capability/risk bindings. Lock the two 98x38 settings triggers,
  independent 202x40 order-type trigger, 138x54 price and 202x48 margin shells,
  two-row 9px-label/17px-or-15px-value hierarchy, transparent idle border,
  shell-only focus ring, one continuous thumb inside a 44px hit target, visible
  percentage value, and leverage-bearing long/short copy. At
  320x720, 390x920, and 448x900 in both themes assert zero document overflow,
  sticky header z-index 70, visible long/short actions, reduced-motion scroll
  behavior, and no fabricated order, strategy, balance, or risk values.
- Contract order type: prove backend-only option rendering, limit-first/first-
  real fallback, trigger-only open, explicit selection, non-mutating backdrop/
  close/Escape, focus trap/return, body scroll lock, public guest opening after
  capability load, market read-only price, editable precision-safe limit price,
  BBO side choice, frozen confirmation, and exact market-without-price/limit-
  with-price request wiring. Pair selection initially focuses Close so its
  search shell stays neutral until Tab/user input. Inspect the order-type and
  confirmation sheets at 320px/390px in both themes with reduced motion and
  zero overflow.
- Assets Transfer parity: assert `v6phV/TuWXq/tPkL1/tPkD1`, a 520px sheet,
  140px amount hero with a 30px data input, 52px glass route, 52px API-logo
  asset row, 50px action, and absence of a native asset `select`. Exercise API
  balance rendering, USDT preference, All, swap, search/filter, explicit
  selection, picker-first Escape, trigger focus return, shared body lock,
  reduced motion, and zero horizontal overflow at 320px, 390px, and 448px in
  both themes. Runtime-computed light tokens must not resolve the hero or close
  face to a dark surface, and the desktop overlay rect must match the mobile
  canvas rect.
- Shared asset-mark presentation: assert image/fallback state classes, a plain
  circular image with no highlight/gradient/border/ring/shadow/padding,
  continuous backend-image failure progression, flat themed fallback, scaled
  initial typography, and no trade-header override that restores an accent
  border. Compile `TradeView` scoped CSS and reject the broad contract-position
  descendant-span selector while preserving direction, margin-mode, and
  leverage badges. Runtime-check a 24–54px sample in both themes and keep
  document overflow at zero.
- Assets account-scope parity: assert vertically stacked full-width account
  cards, separate spot/margin derived rows,
  backend Logo reuse, all/spot/margin switching, a truthful zero-margin state,
  balance hiding, and no additional wallet fetch on scope changes. Browser QA
  at 390x844 in both themes must keep both cards inside the canvas with no
  horizontal overflow and keep the Dock clear of the scrollable holdings area.
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
  exposes real order-book/latest-trade tabs without iframe or remote chart
  script. The only chart-owned external link is the official Lightweight Charts
  attribution logo. Focusing a price/quantity/amount input leaves its
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
- Turnstile lifecycle: assert one module-level loader Promise, one exact script,
  retry after load failure, ready-before-render for the owned synchronous
  script, direct API reuse for a pre-existing async/defer script, generation
  cancellation before and after each await, detached-container cancellation,
  synchronous stale-render removal, stale callback rejection, widget ID `0`,
  successful reset retention, unmount/two-factor cleanup, and supported
  `zh-cn` language casing. Browser QA must navigate away and back, switch both
  themes/locales, and confirm at most one API script and one current iframe.
- Canvas theme behavior: switch both the stage class and root `data-theme`,
  assert the renderer receives the new background/text/grid/series colors,
  and assert no theme callback runs after component unmount.
- Market chart runtime: assert the exact `lightweight-charts@5.2.0` package and
  absence of `klinecharts`, one mounted renderer, official attribution enabled,
  `region`/`group` semantics, OHLCV plus MA5/MA10/MA20, series `update` for the
  forming/latest candle, timestamp-anchored replacement restore, symbol/interval
  dataset fitting only after new points, zero-size resize guards, locale/theme
  in-place updates, horizontal touch drag, pinch zoom, touch kinetic scroll,
  and complete observer/chart/animation-frame cleanup.
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

For the non-modal PWA status island:

```vue
<!-- Wrong: a full-screen modal blocks the application for an auxiliary state. -->
<aside class="pwa-status" style="inset: 0; pointer-events: auto">
  <div class="backdrop" />
  <StatusCard />
</aside>

<!-- Correct: the root is transparent to input and only the island is active. -->
<Transition name="pwa-status-reveal">
  <aside class="pwa-status" aria-live="polite" aria-atomic="false">
    <section class="pwa-status__card" role="status">
      <div class="pwa-status__panel"><StatusContent /></div>
    </section>
  </aside>
</Transition>
```

```css
.pwa-status { pointer-events: none; }
.pwa-status__card { pointer-events: auto; }
@media (prefers-reduced-motion: reduce) {
  .pwa-status *, .pwa-status-reveal-enter-active { animation: none; transition: none; }
}
```

For a Turnstile SPA lifecycle:

```ts
// Wrong: component-local script injection and an unguarded post-await render.
script.async = true
await scriptLoaded
window.turnstile?.render(container, callbacks)

// Correct: one module loader plus generation- and container-owned rendering.
const lifecycle = createTurnstileLifecycle()
await lifecycle.render({
  resolveContainer: () => currentContainer,
  isContainerCurrent: (node) => mounted && currentContainer === node,
  options: { sitekey, language: locale === 'en' ? 'en' : 'zh-cn' },
  callbacks,
})
// Before route teardown, two-factor transition, or replacement:
lifecycle.remove()
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

## Margin Position Close Sheet Contract

- The Contract position-card `Close` action opens the body-Teleported bottom
  sheet mapped from Pencil light frame `ajSJF` and dark frame `DGiNR`. Opening
  the sheet is read-only and must not call the close endpoint.
- Keep the selected 390px geometry as the reference: a 500px bottom sheet,
  24px top corners, 20px horizontal content inset, 58px price and quantity
  fields, 24px percentage rail, 69px position summary, and 62px confirmation
  rail. At 340px and below the horizontal inset may contract to 16px; at every
  supported width the sheet remains full-width with zero page overflow.
- Render pair, direction, margin mode, leverage, mark price, position quantity,
  and estimated PnL from the selected authoritative position/risk snapshot.
  Missing fields render `--`; Pencil sample values must never become fallbacks.
- `margin-close-sheet__ratio` is the position-size selector, not decorative
  tick marks. It uses native range semantics over integer percentages
  `1..=100`, defaults to 100 on every fresh open, supports touch/pointer and
  keyboard input, and exposes localized current-value text. Moving it updates
  the displayed closable quantity, proportional estimated PnL, confirmation
  copy, and visual fill without issuing a request.
- The selected ratio is frozen together with position ID and one idempotency
  key when final confirmation starts. Busy state disables further ratio edits;
  a failed or uncertain request keeps the same ratio/key for an exact retry,
  while closing/reopening the sheet starts a new intent.
- The pink 62px action is a pointer/keyboard slider, not a click-to-submit
  button. A pointer gesture must start on the circular handle; tapping or
  pressing the track itself emits nothing. Pointer release below normalized
  progress `0.9` resets to zero and emits nothing; release at or above `0.9`
  emits exactly one confirmation. Use pointer capture, `touch-action: none`,
  request/emit guards, and reset on cancel, failure, disappearance of the
  target position, or sheet reopen.
- The pink confirmation rail separately exposes slider semantics (`role=slider`,
  0..100 progress, localized value text). Arrow keys adjust confirmation by ten
  percent, Home/End set boundaries, and Enter or Space confirms only after its
  progress already reached the threshold. It must never be reused as the
  position-size selector.
- The close button is the initial safe focus. Overlay, close button, and Escape
  dismiss only while idle; Tab stays inside, body scrolling stays locked, and
  focus returns to the exact position-card action. In-flight state blocks
  dismissal and duplicate drags.
- Preserve bottom safe area, reduced-motion fallback, both color themes, Lucide
  icons, and zero horizontal overflow at 320x720, 390x844, and 448x900.
- Because the sheet Teleports outside the scoped component ancestry, its dark
  override must compile to the full global descendant selector
  `html[data-theme='dark'] .margin-close-sheet`. A regression test must inspect
  `vue/compiler-sfc` output; matching only the source selector is insufficient,
  because partial `:global()` syntax can collapse the override onto bare `html`
  and let the sheet's local light variables win the cascade.

## Directional Margin Leverage Sheet Contract

- The authenticated leverage trigger opens the body-Teleported sheet mapped
  from the current Pencil dark frame `NTiiS` and light frame `CulR4`. It edits
  independent future-order defaults for long and short; opening, stepping,
  choosing a shortcut, or paging the shortcut window never mutates an existing
  position or sends a request.
- Keep the 390×920 reference geometry exact: the sheet starts at `y=80`, is
  390×840 with 24px top corners and `18px 20px 16px` padding; Header is 350×34,
  and the fixed bottom confirmation action is 350×52 with a 26px radius. The
  middle row alone scrolls on short viewports while Header/action remain visible.
- Each direction owns a 16px label, a 350×64 stepper, 42×42 visual plus/minus
  controls, a 52px numeric value with a 22px `x`, and a 350×46/r23 shortcut
  rail. Pseudo hit areas expand the 34px close, 42px step buttons, 38px pills,
  and 32px more icon to at least 44px without changing the Pencil face geometry.
- Normalize the backend product's `leverageLevels` to unique positive ascending
  values. Plus/minus moves only to adjacent real levels. Each direction shows a
  current-centered window of at most six real levels; the more control pages
  that window and never invents Pencil sample levels or changes the selection.
- Opening/reopening initializes both drafts from the current saved settings.
  Backdrop, close, and Escape discard drafts. Confirm emits both values in one
  guarded request; success updates both defaults and closes, while failure keeps
  the sheet and both exact drafts for retry. Busy state blocks dismissal and
  duplicate mutation; focus remains trapped and returns to the leverage trigger.
- Max-open preview may use current margin-wallet available, selected leverage,
  and positive live reference price. Required margin mirrors the current real
  form amount. A local liquidation preview is allowed only for isolated mode
  using the same entry/leverage/maintenance-rate equation as backend position
  risk; cross mode or missing inputs renders `--`. Pencil sample balances and
  prices are never production fallbacks.
- Light roles are page `#FFFFFF`, field/step `#F0F2F1`, line `#D8DEDA`, text
  `#111512`, submit `#087A16`; dark roles are page `#0B0F0D`, field `#181E1A`,
  step `#202723`, line `#364039`, text `#F5F7F6`, submit `#16A765`. Long is
  `#14C982`, short is `#FF3E73` in both themes.
- At 320, 390, and 448px the sheet has zero horizontal overflow, honors dynamic
  viewport and bottom safe-area insets, and keeps normal/reduced-motion behavior.
  Because it Teleports outside component ancestry, dark variables must compile
  to the full global descendant selector
  `html[data-theme='dark'] .contract-sheet--leverage`; source-text matching alone
  is not sufficient regression coverage.

## Runtime Performance, Functional Motion, and Dense Account UI Contract

```ts
type PerformanceTier = 'standard' | 'constrained'

resolvePerformanceTier(signals: {
  saveData?: unknown
  deviceMemory?: unknown
  hardwareConcurrency?: unknown
}): PerformanceTier

detectPerformanceTier(navigatorLike?: object | null): PerformanceTier
```

- Resolve the tier and write `data-performance-tier` on `<html>` before the Vue
  application mounts. Explicit `saveData`, at most 2 GiB memory, at most two
  logical cores, or the combination of at most 4 GiB and four cores selects the
  constrained tier. Missing, non-finite, or invalid browser signals select the
  standard tier rather than assuming that an older WebView is slow.
- Functional feedback and decorative motion are separate contracts. The shared
  `functional-spinner` animation uses only `transform: rotate(...)`; `.spin`,
  Turnstile loading, recharge loading, and other real busy indicators keep a
  low-frequency `steps(8)` rotation under reduced-motion and constrained mode.
  Decorative infinite animation, route veils, heavy backdrop filters, and
  ambient transitions may stop in those modes without hiding business state.
- `SignalField` runs at no more than 30 frames per second in standard mode,
  pauses while the document is hidden or the canvas is outside the viewport,
  and cancels every animation/resize observer on unmount. Reduced-motion and
  constrained mode set the component's semantic static state before measuring
  the canvas, render the static fallback, and start no continuous rAF loop.
- `LaunchIntro` must decide whether motion is allowed before importing GSAP.
  Load GSAP dynamically only when the once-per-session intro will really play;
  constrained and reduced-motion sessions skip the overlay and do not download
  that chunk. Async import failure, page hide, timeout, or unmount must remove
  scroll lock and release every timeline/context exactly once.
- `LightweightMarketChart` observes a replacement points-array boundary rather
  than deep-traversing every OHLC object. Producers must publish a new array for
  live append/update; the chart retains its incremental update and viewport
  restoration rules.
- Theme-specific hero artwork renders exactly one `<img>` for the active theme.
  Do not use two downloaded images whose visibility is controlled only by CSS.
- The Assets member total uses deterministic formatted-length tiers and never
  `text-overflow: ellipsis`. It may wrap and stack the Today Return row at the
  minimum tier, but stays at least 20px, preserves every digit, does not overlap
  the action row, and creates no horizontal overflow at 320, 390, or 448px.
- The KYC country field uses the configured backend country set as its only
  options and opens a body-Teleported searchable dialog. Search matches ISO
  code, backend name, and localized region name; selection preserves the
  backend's original `country` value. The dialog focuses search on open, traps
  Tab, closes with Escape/backdrop/close, restores the exact trigger, locks body
  scroll only while open, and shows a localized no-results state.
- The KYC document-type field uses a second independent searchable dialog and
  renders only the current country rule's raw `document_types` (or the existing
  fallback when the rule is empty). Search matches the localized display label
  and raw backend value with the country search normalizer. Only selecting a
  result changes `form.documentType`; submit preserves that raw value as
  `document_type`, including unknown configured types.
- Both KYC dialogs share the `.kyc-picker-*` visual surface and explicit
  Teleport focus styles, while retaining separate open/query/dialog/trigger
  state. Do not couple their queries or reuse one selected value for the other.
- A Teleported KYC picker does not inherit page-local `--surface-2`. Its field
  and close surfaces must derive from `--surface-elevated` and `--ink` so light
  and dark themes retain readable text/icon contrast without hard-coded theme
  duplication.
- Reduced-motion and constrained-device blur fallbacks for the Teleported KYC
  mask must compile to the actual mask selector. Wrap the complete descendant
  selector in `:global(...)` and inspect `vue/compiler-sfc` output; a partial
  global selector can collapse onto bare `html` and leave the overlay blur
  active on the devices that most need the fallback.

Required verification includes pure tier tests, spinner source/runtime tests,
SignalField lifecycle contracts, a production PWA build proving GSAP is split,
browser constrained/reduced-motion checks, KYC search in both themes, and Assets
long-number geometry at 320/390/448px.

## Release Lifecycle, Route Accessibility, and Artifact Budget Contract

### 1. Scope / Trigger

- Trigger: changing PWA registration/update/install UI, the root route shell,
  global motion, build modes, CSP, or release artifacts.

### 2. Signatures

```ts
runPwaUpdate(options: RunPwaUpdateOptions): Promise<boolean>
createPwaInstallEligibilitySession(input: PwaInstallEligibilityInput): PwaInstallEligibilitySession
createRouteAccessibilityCoordinator(input): RouteAccessibilityCoordinator

PWA_UPDATE_TIMEOUT_MS = 15_000
PWA_INSTALL_SESSION_DELAY_MS = 60_000
PWA_INSTALL_FREQUENCY_CAP_MS = 7 * 24 * 60 * 60 * 1000
```

### 3. Contracts

- An update races one 15-second deadline, always releases timers/listeners, and
  leaves an explicit retry/recovery state after timeout, worker rejection, or
  controller loss. It never reloads indefinitely.
- The install prompt is eligible only after the session delay, on a value route,
  outside standalone mode, with a real deferred install event, and outside the
  seven-day frequency cap. Dismissal never blocks update status.
- PWA and Tauri builds are isolated. PWA owns manifest/service-worker/precache;
  Tauri owns an explicit functional CSP and contains no PWA worker artifact.
  Large stage art is optimized and excluded from precache.
- The app shell owns one visible route `<main>`, one localized document title,
  a polite destination announcement, and a visible-on-focus skip link that
  focuses `#main-content`. Transition completion must match the current render
  key before focus/announcement commits.
- Coarse-phone controls expose at least a 44px hit area without changing the
  approved painted geometry. Functional busy indicators remain moving in
  constrained/reduced-motion mode; decorative loops may stop.
- Source-size, behavior-test, JS/CSS bundle, and generated-artifact budgets are
  hard release gates, not advisory reports.

### 4. Validation & Error Matrix

| Condition | Required behavior |
|-----------|-------------------|
| Update exceeds 15 seconds | Unlock UI, preserve retry, no automatic reload loop |
| No deferred install event or non-value route | Do not display install prompt |
| PWA worker appears in Tauri output | Fail artifact check |
| Stage image enters PWA precache | Fail artifact check |
| Old transition finishes after newer route | Ignore stale completion |
| Active modal exists during route change | Announce destination; retain modal focus |
| 320/390/448 viewport | Zero document horizontal overflow |
| Budget exceeds tracked limit | Fail release gate with measured file/size |

### 5. Good / Base / Bad Cases

- Good: an update times out, the user retries, and the second worker activation
  succeeds without duplicate listeners or a stuck loading state.
- Base: route title updates immediately; focus and announcement commit after the
  matching transition finishes.
- Bad: cache the multi-megabyte stage image, ship a service worker in Tauri, or
  remove functional spinner motion together with decorative animation.

### 6. Tests Required

- Unit-test update timeout/retry/controller recovery and install delay, route,
  standalone, event, dismissal, and frequency-cap decisions.
- Route tests assert all typed destinations, one main landmark, stale transition
  rejection, dialog focus preservation, skip-link focus, and localized titles.
- Artifact tests inspect generated PWA and Tauri directories; budget tests read
  actual compressed output; behavior quality tests require executable assertions.
- Ego Browser checks home/trade/seconds/orders/assets/KYC at 320, 390, and 448px.

### 7. Wrong vs Correct

```ts
// Wrong: an update can hang forever and every page shows install UI.
await updateServiceWorker(true)
showInstallPrompt.value = Boolean(deferredPrompt)

// Correct: bounded update plus centralized eligibility.
await runPwaUpdate({ updateServiceWorker, timeoutMs: PWA_UPDATE_TIMEOUT_MS })
const install = createPwaInstallEligibilitySession({ routeName, now, storage })
showInstallPrompt.value = install.evaluate({ deferredPrompt, standalone }).eligible
```
