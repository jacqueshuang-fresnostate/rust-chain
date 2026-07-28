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
- The root navigation order is Home, Markets, Spot, Seconds, Contract, Assets,
  Profile. Seconds is the raised center action; all icon targets remain at
  least 44x44px from 320px through 448px.
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
