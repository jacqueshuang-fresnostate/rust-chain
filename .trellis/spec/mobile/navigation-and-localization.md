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
goBackOr(router: Router, fallback?: RouteLocationRaw): Promise<void>
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

- Bottom tabs (`home`, `markets`, `trade`, `assets`, `profile`) use router `replace`.
- Drill-down pages and modals represented as routes use router `push`.
- Every detail route defines `meta.depth`, `meta.showBottomNav: false`, and `meta.backFallback`.
- `PageHeader` calls `goBackOr`; it must not call `router.back()` directly.
- `scrollBehavior` restores `savedPosition` and otherwise returns `{ top: 0, left: 0 }`.

### Trade picker

- Pair picker route: `/markets?purpose=trade&mode=spot|contract`.
- Picker selection replaces the route with `/trade/:BASE_QUOTE` and keeps `mode=contract` when applicable.
- Storage keys are `hippo_mobile_last_trade_symbol` and `hippo_mobile_last_trade_mode`.
- Switching spot/futures updates the current trade route with `replace`, so switching modes does not add a back-stack entry.

### Authentication redirects

- Only single-slash internal paths are accepted.
- Values such as `https://host`, `//host`, non-strings, and empty values fall back to `/` or the caller-provided internal fallback.
- Successful login and registration use `replace` to avoid returning to a completed auth step.
- Login only exposes username mode when `/auth/login/config` enables it.
- Registration requires or hides the email-code field and requires the invitation code according to `/auth/register/config`.

### Localization

- Fixed UI text must use `vue-i18n`; do not add Chinese or English literals to Vue templates or API fallback mapping.
- Supported app locales are `zh-CN` and `en`; the persisted key is `hippo_mobile_locale`.
- Language changes update the Vue locale, `<html lang>`, runtime `Intl` locale, and persisted locale in one operation.
- Locale-aware content APIs receive `currentApiLocale()` when the endpoint supports a locale parameter.
- Backend enum/status values may be mapped when known, but unknown values must remain visible rather than being replaced with an incorrect translation.

## 4. Validation & Error Matrix

| Condition | Required behavior |
|-----------|-------------------|
| Pair has fewer than two assets | Use `BTC_USDT` |
| Redirect does not start with exactly one `/` | Use internal fallback |
| Router history has no internal `state.back` | `router.replace(meta.backFallback)` |
| Main tab selected | Replace current history entry |
| Trade mode changes | Replace route and persist mode |
| Stored locale is unknown | Use system locale, then `zh-CN` |
| Locale persistence is unavailable | Keep the in-memory locale active |
| Content translation is unknown | Preserve backend source text |
| Public country endpoint is unavailable | Show the basic region list and keep server validation on submit |
| Authentication config endpoint is unavailable | Default to email-only login, required email code, and optional invitation code |

## 5. Good / Base / Bad Cases

- Good: Open futures for `DOGE_USDT`, visit Assets, then tap Trade; the app returns to `/trade/DOGE_USDT?mode=contract`.
- Base: Open `/profile/language` directly and tap Back; the app replaces the route with `/profile`.
- Bad: Tap Home, Markets, and Assets, then browser Back returns to Markets. This means a main tab used `push`.
- Bad: Switch to English and still see fixed Chinese labels on product pages.

## 6. Tests Required

- Unit: route symbol normalization, redirect sanitization, usable back-state detection, and transition direction.
- Unit: locale normalization and app-locale to API-locale mapping.
- Unit: dynamic prediction text preserves English and localizes supported Chinese patterns.
- Browser: pair picker returns to the selected trade pair and preserves futures mode.
- Browser: main tabs do not remain in history; direct-open detail back uses its fallback.
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
selectTradeMode('contract') // persists mode and replaces the route
const label = t('prediction.confirmOrder')
```
