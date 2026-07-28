# Mobile PWA, Theme, and Application Shell Contract

## 1. Scope / Trigger

Apply this contract when changing `mobile/` build modes, Vite public base,
manifest metadata, service-worker behavior, install/update prompts, theme
tokens, the root application shell, bottom navigation, or the announcement
message center.

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
- Shared text uses stable pixel sizes and `letter-spacing: 0`; do not scale
  font size with viewport width.
- The visual bottom navigation order is Home, Markets, Spot, Seconds,
  Contract, Assets, Profile. Seconds is the raised center action and remains a
  secondary motion route; all icon targets remain at least 44x44px from 320px
  through 448px.
- The message center calls `fetchNews(40)` and may persist only local read IDs.
  It must not invent account, order, wallet, security, or transaction events.

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

## 5. Good / Base / Bad Cases

- Good: `build:pwa` produces manifest, service worker, brand icons, and an
  offline-reloadable shell while cache inspection contains no API or WebSocket
  endpoint.
- Good: selecting Contract opens the persisted pair with
  `?mode=contract`; selecting Seconds opens `/seconds`.
- Base: Safari on iOS receives localized Add to Home Screen instructions
  because `beforeinstallprompt` is unavailable.
- Bad: a Tauri bundle contains `sw.js` or `manifest.webmanifest`.
- Bad: a message row claims a deposit, login alert, or order event without a
  real backend source.

## 6. Tests Required

- Unit/source contract: build modes, Tauri double guard, manifest fields,
  `runtimeCaching: []`, denied fallback routes, single `PwaStatus`, safe prompt
  routes, theme normalization, and complete `zh-CN`/English keys.
- Build: `npm run build:pwa` and inspect generated manifest/service worker;
  `npm run build:tauri` and assert no PWA artifacts remain.
- Browser: theme switch survives reload; all seven routes are reachable;
  320x720, 390x844, and 448x900 have no horizontal overflow; focused inputs
  retain a complete visible outline.
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
- Root-to-root navigation uses the prototype's six-item `NAV_ITEMS` order, a
  continuously rendered re-keyed `.route-veil-root`, and
  `route-forward|back` plus `transition-root`. The visual bottom navigation
  still has seven items: its raised Seconds action resolves to no root key and
  uses `transition-secondary`, whose veil is hidden.
- Keep the incoming route component as the `.view-stack`. Do not put
  `.view-stack` on the persistent route host, because that traps sticky headers
  in the wrong stacking context.
- Sticky root/secondary headers remain at z-index 70, the root navigation at
  z-index 40, and all ambient/veil layers remain `pointer-events: none`.

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
  sticky-header z-index; clean console.
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
