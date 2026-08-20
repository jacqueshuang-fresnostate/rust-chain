# Mobile Navigation and Localization Contract

## 1. Scope / Trigger

Apply this contract when changing a route, bottom navigation item, back button, trade pair/mode picker, authentication redirect, language selector, user-facing copy, or locale-sensitive API call in `mobile/`.

The contract prevents three observed failures: main tabs polluting browser history, direct-open detail pages exiting the app on back, and language switches translating navigation while leaving business pages or formatted values in the old locale.

## 2. Signatures

Core navigation signatures live in `mobile/src/core/navigation.ts`:

```ts
normalizeRouteSymbol(value: unknown): string
sanitizeInternalRedirect(value: unknown, fallback?: string): string
hasUsableRouterBack(state: unknown): boolean
goBackOr(
  router: Router,
  fallback?: RouteLocationRaw,
  options?: { preferFallback?: boolean },
): Promise<void>
createBottomNavSecondsTarget(): RouteLocationRaw
createBottomNavSecondsFallbackTarget(): RouteLocationRaw
isBottomNavigationSecondsEntry(state: unknown): boolean
createLoginRedirectTarget(redirect: unknown): RouteLocationRaw
replaceAuthStep(router: Router, target: RouteLocationRaw): ReturnType<Router['replace']>
updateRouteTransition(toDepth: unknown, fromDepth: unknown): void
```

Locale signatures live in `mobile/src/i18n/index.ts`:

```ts
normalizeMobileLocale(value: unknown): 'zh-CN' | 'en' | null
setAppLocale(locale: 'zh-CN' | 'en'): void
currentApiLocale(): 'zh-CN' | 'en-US'
currentIntlLocale(): string
```

Authentication configuration signatures live in `mobile/src/api/auth.ts`:

```ts
fetchLoginConfig(): Promise<{ usernameLoginEnabled: boolean }>
fetchRegisterConfig(): Promise<{ emailCodeRequired: boolean; inviteCodeRequired: boolean }>
```

The navigation store persists both parts of the latest trade context:

```ts
rememberTradeSymbol(symbol: unknown): void
rememberTradeMode(mode: unknown): void
lastTradePath: ComputedRef<string>
```

## 3. Contracts

### Router history

- Root navigation has exactly five ordered visual destinations and uses router
  `replace`: `home`, `markets`, persisted `trade`, `assets`, and `profile`.
- The central Trade destination restores both the persisted symbol and the
  persisted spot/contract mode. Spot, contract, and seconds remain independent
  operational routes and stay reachable through product/page actions; visual
  dock consolidation must not merge their business behavior.
- Spot resolves to `/trade/:symbol`; contract resolves to
  `/trade/:symbol?mode=contract`; seconds resolves to `/seconds`. Do not merge
  these three operational surfaces into one root destination.
- The selected Home shortcut grid links its seventh product cell to the named
  `seconds` route and labels it with the localized `home.secondsShortcut` copy.
  Prediction remains reachable from Product Hub, but must not replace Seconds
  in the selected Home grid.
- Drill-down pages and modals represented as routes use router `push`.
- Every detail route defines `meta.depth`, `meta.showBottomNav: false`, and `meta.backFallback`.
- Seconds order history is the named `seconds-history` detail route at
  `/seconds/history`. It has depth 2, hides the Dock, falls back to `/seconds`
  when opened directly, and is reached from the `/seconds` Header with
  `router.push({ name: 'seconds-history' })`. The trading workspace keeps active
  order cards but does not duplicate historical rows below the order form.
- The Seconds settlement card's History action clears every queued result and
  pushes `{ name: 'seconds-history' }`. Close and Continue remove only the
  current FIFO item and leave the user on the trading page.
- Message Center is one of those detail routes. The Home Bell pushes the named
  `message-center` route; `/messages` declares depth 1, hides the Dock, and
  falls back to named Home. Its custom selected-frame ArrowLeft calls
  `goBackOr` exactly like `PageHeader` rather than calling `router.back()`.
- News is also a Product Hub detail route. Product Hub pushes the named `news`
  route; `/news` declares depth 1, hides the Dock, and falls back to
  `/products`. Its selected Pencil `PageHeader` must set `back` to `true` so
  the shared 44px localized ArrowLeft is rendered. With usable internal
  history it returns to the actual source; when `/news` is opened or refreshed
  directly, `goBackOr` replaces it with `/products` instead of leaving the app.
- Help & Support is a separate detail route. The Profile help row pushes named
  route `help-support`; `/profile/help` declares depth 1, hides the Dock, and
  falls back to `/profile`. Do not reuse Message Center for this row: Home Bell
  remains the root announcement entry.
- The Help online-service row pushes named route `support-chat` at
  `/profile/help/chat`. It declares depth 2, hides the Dock, falls back to
  `/profile/help`, and uses the authenticated first-party support API. It must
  not open an environment URL or a new browser tab.
- `PageHeader` calls `goBackOr`; it must not call `router.back()` directly.
- `PageHeader` exposes an explicit `preferFallback` input. It bypasses a usable
  `history.state.back` only when the owning workflow requires a deterministic
  replacement target.
- The retained legacy bottom-navigation Seconds target adds the
  `bottom-navigation-seconds` history-state source while replacing `/seconds`.
  Only that explicit source makes the Seconds header prefer `/`; the selected
  Home shortcut pushes Seconds without the marker and therefore returns
  through history to Home. The deterministic Home replacement must explicitly
  clear the custom source because Vue Router web history merges user state
  during `replace`; later replacement routes must never inherit an active
  Seconds source.
- `scrollBehavior` restores `savedPosition` and otherwise returns `{ top: 0, left: 0 }`.

### Trade picker

- Pair picker route: `/markets?purpose=trade&mode=spot|contract`.
- Picker selection replaces the route with `/trade/:BASE_QUOTE` and keeps `mode=contract` when applicable.
- Storage keys are `hippo_mobile_last_trade_symbol` and `hippo_mobile_last_trade_mode`.
- Switching spot/futures updates the current trade route with `replace`, so switching modes does not add a back-stack entry.

### Authentication redirects

- Only single-slash internal paths are accepted.
- Values such as `https://host`, `//host`, `/\host`, paths containing
  backslashes or ASCII control characters, non-strings, and empty values fall
  back to `/` or a validated caller-provided internal fallback. An unsafe
  fallback itself resolves to `/`.
- Successful login and registration use `replace` to avoid returning to a completed auth step.
- Login replaces itself with registration, forgot-password, and two-factor
  steps while carrying only sanitized internal `redirect` context. Completion
  cannot expose the replaced login entry through browser Back.
- Registration and password reset explicitly prefer their redirect-carrying
  login fallback over stale root history. Two-factor header back, reset, and
  invalid-challenge branches use the same sanitized login target. Successful
  verification and setup replace to the sanitized business redirect.
- Login and registration use `push` only for the language page, carrying a
  sanitized `back` value so a refreshed or directly opened page still returns
  to the correct authentication step.
- Authentication form values such as account, email, password, verification
  code, confirmation, and invitation code stay in component/request state and
  must never be copied into route params or query strings.
- Login only exposes username mode when `/auth/login/config` enables it.
- Registration requires or hides the email-code field and requires the invitation code according to `/auth/register/config`.

### Localization

- Fixed UI text must use `vue-i18n`; do not add Chinese or English literals to Vue templates or API fallback mapping.
- A history-page title must name the business records it contains (for example,
  Seconds order history), rather than using an ambiguous generic History label.
- Seconds history uses symmetric `seconds.profitAmount`,
  `seconds.lossAmount`, and `seconds.profitLossAmount` keys. The label follows
  the authoritative result: win, loss, or unavailable/unknown respectively;
  neither the template nor the amount formatter assembles fixed Chinese or
  English copy.
- Seconds settlement notices use symmetric locale keys for the settled kicker,
  profit/loss title, authoritative-source note, detail label, remaining count,
  Continue trading, and View order history. Pair, direction, duration, amount,
  and asset remain bound order values rather than fixed template copy.
- Supported app locales are `zh-CN` and `en`; the persisted key is `hippo_mobile_locale`.
- Language changes update the Vue locale, `<html lang>`, runtime `Intl` locale, and persisted locale in one operation.
- Locale-aware content APIs receive `currentApiLocale()` when the endpoint supports a locale parameter.
- Backend enum/status values may be mapped when known, but unknown values must remain visible rather than being replaced with an incorrect translation.
- Wallet ledger category labels and every known emitted `change_type` use
  symmetric `ledger.*` keys in `zh-CN` and `en`. An unknown `change_type`
  renders localized `ledger.typeOther` as its primary label and keeps the raw
  enum visible as secondary technical information.
- Wallet ledger dates group by the runtime's local calendar day. Group headings
  use localized Today/Yesterday labels or `Intl.DateTimeFormat`, and record
  counts use vue-i18n pluralization rather than assembled fixed copy.

## 4. Validation & Error Matrix

| Condition | Required behavior |
|-----------|-------------------|
| Pair has fewer than two assets | Use `BTC_USDT` |
| Redirect does not start with exactly one `/` | Use internal fallback |
| Redirect/back contains a backslash or ASCII control character | Use internal fallback |
| Router history has no internal `state.back` | `router.replace(meta.backFallback)` |
| Seconds history source is `bottom-navigation-seconds` | Ignore stale `state.back` and replace Home |
| Seconds was pushed from the selected Home shortcut | Use history Back and return Home |
| Main tab selected | Replace current history entry |
| Spot root selected | Open persisted symbol without `mode=contract` |
| Contract root selected | Open persisted symbol with `mode=contract` |
| Seconds root selected | Open the independent named route `seconds` |
| Home seventh product shortcut selected | Push the named `seconds` route; do not open Prediction |
| Seconds Header history action selected | Push `/seconds/history`, hide the Dock, and preserve `/seconds` in history |
| Seconds history is opened directly | Back replaces with `/seconds` |
| Seconds history result is win/loss/unknown | Render the matching localized profit/loss/generic label without translating an unknown source result incorrectly |
| Settlement card selects View order history | Clear the entire result queue and push the named `seconds-history` route |
| Settlement card selects Close or Continue | Advance exactly one FIFO result and remain on `/seconds` |
| Home Bell opens Message Center | Push `/messages`, hide the Dock, and preserve Home in history |
| Message Center Back has usable Home history | Use router Back and return Home |
| Message Center Back has no usable history | Replace with named Home fallback |
| Product Hub opens News | Push `/news`, hide the Dock, and preserve Product Hub in history |
| News Back has usable Product Hub history | Use router Back and return `/products` |
| News is opened directly | Replace with `/products` through the shared `PageHeader` fallback |
| Profile Help row is selected | Push `/profile/help`; do not open Message Center |
| Help is opened directly | Back replaces with `/profile` |
| Help opens online service | Push `/profile/help/chat`; keep Help in history and use no external URL |
| Support chat is opened directly | Back replaces with `/profile/help` |
| Trade mode changes | Replace route and persist mode |
| Stored locale is unknown | Use system locale, then `zh-CN` |
| Locale persistence is unavailable | Keep the in-memory locale active |
| Content translation is unknown | Preserve backend source text |
| Public country endpoint is unavailable | Show the basic region list and keep server validation on submit |
| Authentication config endpoint is unavailable | Default to email-only login, required email code, and optional invitation code |
| Login opens register, forgot-password, or 2FA | Replace Login while preserving sanitized `redirect` |
| Register, forgot-password, or invalid/reset 2FA returns to Login | Replace the explicit Login target with sanitized `redirect` |

## 5. Good / Base / Bad Cases

- Good: Open futures for `DOGE_USDT`, visit Assets, then tap Trade; the app returns to `/trade/DOGE_USDT?mode=contract`.
- Base: Open `/profile/language` directly and tap Back; the app replaces the route with `/profile`.
- Base: Open `/news` directly and tap Back; the app replaces the route with `/products`.
- Bad: Tap Home, Markets, and Assets, then browser Back returns to Markets. This means a main tab used `push`.
- Bad: Render `/news` with `PageHeader :back="false"`; users have no visible route back to Product Hub.
- Bad: Switch to English and still see fixed Chinese labels on product pages.

## 6. Tests Required

- Unit: route symbol normalization, redirect sanitization, usable back-state detection, and transition direction.
- Router/history: use Vue Router memory history to prove the legacy marked
  Seconds target forces Home, an unmarked Home Seconds push returns Home,
  auth-step replacement leaves no
  Login entry, and 2FA reset/invalid returns preserve sanitized redirects. Also
  exercise Vue Router web history replacement semantics to prove the custom
  Seconds source is cleared rather than merged into later routes.
- Unit: locale normalization and app-locale to API-locale mapping.
- Unit: dynamic prediction text preserves English and localizes supported Chinese patterns.
- Browser: pair picker returns to the selected trade pair and preserves futures mode.
- Router/behavior: prove Seconds -> Seconds history -> Back returns to Seconds,
  direct-open history replaces with `/seconds`, and the Header action uses the
  named route rather than scrolling the trading page.
- Localization/source: prove all three Seconds history profit/loss labels exist
  symmetrically in `zh-CN` and `en`, and the Vue template contains no fixed
  Chinese copy.
- Localization/source: prove all settlement-card keys exist symmetrically, all
  three actions use localized copy, and History clears the queue before named
  route navigation.
- Browser: main tabs do not remain in history; direct-open detail back uses its fallback.
- Browser: Home Bell opens Message Center without Root Header or Dock; its
  ArrowLeft returns Home, while a direct-open message route uses the same Home
  fallback.
- Source/router: `/news` renders the shared Pencil `PageHeader` with
  `:back="true"`, declares `backFallback: '/products'`, preserves Product Hub
  history when pushed, and uses `replace('/products')` when opened directly.
- Browser: Profile Help opens `/profile/help` without the Dock, while the Home
  Bell still opens `/messages`; direct-open Help falls back to Profile.
- Source/router/browser: the online-service row opens the named internal
  `support-chat` route, direct-open Back falls back to Help, guests render the
  login-required state, and no `VITE_SUPPORT_CHAT_URL` reference remains.
- Browser: all five dock destinations remain visible with at least 44px icon
  targets inside the 84px navigation, and no horizontal page overflow at
  320px, 390px, and 448px. Independently verify spot, contract, and seconds
  route reachability.
- Source/browser: the selected Home product cell renders the 19px Lucide Zap,
  localized Seconds label, and reaches `#/seconds`; no Prediction target is
  present in that shortcut grid.
- Browser: switching language survives reload and both 390px mobile and wide H5 layouts remain usable.
- Build: H5, Android Debug APK, and iOS simulator bundle after dependency or startup changes.

## 7. Wrong vs Correct

### Wrong

```ts
router.push('/assets')
router.back()
mode.value = 'contract'
const label = '确认订单'
```

### Correct

```ts
router.replace('/assets')
await goBackOr(router, route.meta.backFallback || '/')
// /news route meta.backFallback is '/products'; PageHeader owns this call.
selectTradeMode('contract') // persists mode and replaces the route
const label = t('prediction.confirmOrder')
```

## 8. Prototype-to-Client Handoff

- The approved visual reference lives in `mobile/sites-prototype/`; the
  production implementation lives in `mobile/src/`.
- Reuse visual hierarchy, tokens, and route intent from the prototype, but keep
  all real API calls, validation, stores, authentication redirects, and route
  names from the Vue client. Never copy deterministic prototype data into a
  financial workflow.
- The real client owns light/dark theme controls, localization, safe areas, and
  PWA behavior. Prototype-only controls must not become a second runtime shell.
