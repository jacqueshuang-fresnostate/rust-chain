# Research: production-grade PWA integration for mobile

- Query: Research production-grade PWA integration for the existing `mobile` Vue 3 + Vite 5 + Tauri 2 application, covering plugin compatibility, manifest/icons, caching and security boundaries, install/update UX, offline shell, Tauri coexistence, iOS constraints, and verification.
- Scope: mixed
- Date: 2026-07-28

## Findings

### Executive recommendation

Adopt `vite-plugin-pwa@1.3.0` with its default `generateSW` strategy for the web/PWA build, manual service-worker registration, and prompt-based updates. Keep the first production release deliberately conservative:

1. Precache only the versioned application shell: `index.html`, hashed JavaScript/CSS, fonts, and PWA icons.
2. Treat every API request and all WebSocket traffic as network-only. Do not cache authenticated responses, public market data, news, account state, or mutations.
3. Do not enable Workbox Background Sync for orders, trades, transfers, withdrawals, KYC, authentication, or any other financial operation.
4. Build a truthful offline shell that clearly says trading data is unavailable/stale and disables all state-changing financial actions.
5. Disable the PWA plugin during Tauri builds and independently gate runtime registration with an explicit Tauri-runtime check.
6. Use a user-controlled update prompt. Never automatically reload while an order, withdrawal, KYC, 2FA, or other unsaved form is active.

`generateSW` is sufficient for this scope and has a smaller maintenance surface than a custom service worker. Move to `injectManifest` only if a later requirement needs custom push handling, special request transforms, or cache behavior that cannot be expressed safely with Workbox configuration.

### Repository baseline and files found

| File | Current role and relevant observation |
| --- | --- |
| `mobile/package.json` | Vue/Vite/Tauri scripts and dependencies. `build` is shared by web and Tauri (`mobile/package.json:8`); there is no PWA dependency or dedicated PWA/native build command. |
| `mobile/package-lock.json` | npm lockfile that will capture `vite-plugin-pwa`, Workbox, and their transitive dependencies. |
| `mobile/vite.config.ts` | Vite 5 configuration. Only the Vue plugin is registered (`mobile/vite.config.ts:10`); Tauri environment prefixes and the H5 API proxy are configured here (`mobile/vite.config.ts:16`). |
| `mobile/index.html` | Current favicon is a wide source logo (`mobile/index.html:5`), the viewport already enables `viewport-fit=cover` (`mobile/index.html:6`), and `theme-color` is white (`mobile/index.html:7`). |
| `mobile/src/main.ts` | Creates and mounts Vue, Pinia, router, and i18n (`mobile/src/main.ts:1`); there is no service-worker registration. |
| `mobile/src/core/platform.ts` | Detects `__TAURI_INTERNALS__` (`mobile/src/core/platform.ts:4`), but only returns native platform values when the user agent also matches iOS or Android (`mobile/src/core/platform.ts:8`). Desktop Tauri therefore falls through to `desktop_web`. |
| `mobile/src/config/app.ts` | H5 development uses same-origin proxying, but the production fallback backend is `http://127.0.0.1:8080` (`mobile/src/config/app.ts:5`), which is invalid for a deployed HTTPS PWA and would be blocked as mixed content. |
| `mobile/src/api/client.ts` | Stores access and refresh tokens in `localStorage` (`mobile/src/api/client.ts:5`), adds bearer authorization (`mobile/src/api/client.ts:47`), performs one refresh/retry (`mobile/src/api/client.ts:79`), and clears auth on terminal failure (`mobile/src/api/client.ts:91`). |
| `mobile/src/stores/session.ts` | Restores authentication state from the locally stored access token (`mobile/src/stores/session.ts:5`). |
| `mobile/src/router/index.ts` | Uses `createWebHashHistory` (`mobile/src/router/index.ts:1`) and lazy route components (`mobile/src/router/index.ts:39`). Hash routing simplifies the offline navigation fallback because the server-visible path remains the app shell URL. |
| `mobile/src/api/marketSocket.ts` | Connects to `/ws/public`, converts HTTPS to WSS (`mobile/src/api/marketSocket.ts:24`), and implements heartbeat/reconnect (`mobile/src/api/marketSocket.ts:65`); it has no explicit online/visibility recovery path. |
| `mobile/src/stores/market.ts` | Maintains a market-data timestamp, but writes `updatedAt` in `finally` even after a failed REST refresh (`mobile/src/stores/market.ts:27`), so it cannot currently support a truthful stale-data indicator. |
| `mobile/src/App.vue` | Handles auth expiry and navigation (`mobile/src/App.vue:13`); there is no global offline, install, or update UI. |
| `mobile/src/styles/base.css` | Already applies top and bottom safe-area insets (`mobile/src/styles/base.css:47`), which should be retained for standalone iOS display. |
| `mobile/src/env.d.ts` | Contains Vite and route type declarations only (`mobile/src/env.d.ts:1`); the PWA virtual-module types are absent. |
| `mobile/tsconfig.json` | Lists only `vite/client` under compiler types (`mobile/tsconfig.json:19`). |
| `mobile/src-tauri/tauri.conf.json` | Tauri uses `npm run build` and serves `../dist` (`mobile/src-tauri/tauri.conf.json:6`); native icons are configured separately (`mobile/src-tauri/tauri.conf.json:28`). Native CSP is currently `null` (`mobile/src-tauri/tauri.conf.json:24`). |
| `mobile/src/assets/logo.png` | Existing 1000×250 wide logo; not suitable as a square install icon. |
| `mobile/src-tauri/icons/icon.png` | Existing 512×512 transparent native icon; useful as source artwork or an `any` icon, but it should not be declared maskable without a separate full-bleed, solid-background composition that passes the maskable safe-zone check. |
| `mobile/README.md` | Documents shared H5/iOS/Android source and current build verification (`mobile/README.md:1`); PWA deployment and native/PWA build separation are not documented. |

Repository search found no current `serviceWorker`, `navigator.serviceWorker`, `manifest.webmanifest`, `beforeinstallprompt`, Workbox, or `vite-plugin-pwa` implementation.

The installed dependency tree at research time is:

- `vite@5.4.21`
- `@vitejs/plugin-vue@5.2.4`
- `vue@3.5.39`
- `@tauri-apps/api@2.11.1`
- `@tauri-apps/cli@2.11.4`

### Plugin and Vite 5 compatibility

As of 2026-07-28, npm publishes `vite-plugin-pwa@1.3.0` as latest. Its package metadata declares support for Vite `^3.1.0 || ^4 || ^5 || ^6 || ^7 || ^8`, so it is directly compatible with the repository's Vite 5.4.21. The plugin documentation notes that releases from 0.17 onward require Vite 5.

Recommended dependency policy:

- Pin `vite-plugin-pwa` to a reviewed version rather than relying on an unbounded major range.
- Commit the npm lockfile update.
- Run `npm audit` and inspect the resolved Workbox/transitive dependency tree before merge.
- Do not upgrade Vite as part of this PWA integration unless separately required; the current Vite 5 line is supported.

Recommended Vite PWA configuration shape:

- `strategies: 'generateSW'`
- `registerType: 'prompt'`
- `injectRegister: null` so application code controls whether registration occurs
- `devOptions.enabled: false` so normal Vite and Tauri development cannot leave a development worker behind
- `workbox.navigateFallback: '/index.html'`
- a navigation denylist for `/api/`, `/api/v1/`, `/ws/`, and any health/download endpoints
- no general-purpose runtime cache in the initial release
- explicit manifest fields and explicit icon paths under `mobile/public/`

The Vue integration exposes `virtual:pwa-register/vue`, including `needRefresh`, `offlineReady`, and `updateServiceWorker`. Add the matching `vite-plugin-pwa/vue` type entry rather than suppressing TypeScript errors. Manual registration is important here because the same Vue entry point is also compiled for Tauri.

Official references:

- [vite-plugin-pwa npm package](https://www.npmjs.com/package/vite-plugin-pwa)
- [vite-plugin-pwa 1.3.0 package metadata and Vite peer range](https://raw.githubusercontent.com/vite-pwa/vite-plugin-pwa/v1.3.0/package.json)
- [vite-plugin-pwa getting started](https://vite-pwa-org.netlify.app/guide/)
- [Service-worker registration modes](https://vite-pwa-org.netlify.app/guide/register-service-worker)
- [Vue 3 integration and update state](https://vite-pwa-org.netlify.app/frameworks/vue.html)
- [Service-worker strategies and behaviors](https://vite-pwa-org.netlify.app/guide/service-worker-strategies-and-behaviors)
- [Development service-worker configuration](https://vite-pwa-org.netlify.app/guide/development.html)
- [Type declarations and build-size failure behavior](https://vite-pwa-org.netlify.app/guide/faq)

### Web manifest and icon requirements

Use a stable application identity and align all URL fields with the actual deployment base:

- Dedicated-origin deployment: `id: '/'`, `scope: '/'`, `start_url: '/'`.
- Subpath deployment: use the same permanent prefix for all three, for example `id: '/mobile/'`, `scope: '/mobile/'`, `start_url: '/mobile/'`, and set Vite `base` accordingly.
- Do not change `id` after launch merely because the display name, route, or tracking parameters change; browsers use it as installation identity.
- Suggested metadata: `name: 'Hippo Mobile'`, `short_name: 'Hippo'`, `lang: 'zh-CN'`, `dir: 'ltr'`, `display: 'standalone'`, and `categories: ['finance']`.
- Keep `theme_color`, `background_color`, `mobile/index.html`, and the application loading surface visually consistent.
- Avoid shortcuts to authenticated or destructive financial routes. If shortcuts are added later, restrict them to harmless public or navigation-only entry points.
- Screenshots are optional. If supplied, use public/logged-out sample data with no account identifiers, balances, orders, KYC material, QR codes, or access tokens.

Required icon set:

| Asset | Purpose |
| --- | --- |
| 192×192 PNG, `purpose: "any"` | Baseline Chromium install icon. |
| 512×512 PNG, `purpose: "any"` | Large install/splash source and baseline install criterion. |
| 512×512 PNG, `purpose: "maskable"` | Separately composed full-bleed icon with important content inside the maskable safe zone. |
| 180×180 PNG linked as `apple-touch-icon` | iOS Home Screen icon; WebKit gives this link precedence when present. |
| SVG/PNG favicon as needed | Browser-tab identity, independent of install icons. |

The existing transparent Tauri icon may be reused as source art for an `any` icon after visual verification. It is not automatically a valid maskable icon: maskable artwork must tolerate circular and platform-specific cropping, with essential content inside the specification's safe region. The wide `mobile/src/assets/logo.png` is unsuitable for these square roles.

Official references:

- [W3C Web App Manifest](https://www.w3.org/TR/appmanifest/)
- [PWA minimal requirements](https://vite-pwa-org.netlify.app/guide/pwa-minimal-requirements.html)
- [vite-plugin-pwa assets generator](https://vite-pwa-org.netlify.app/assets-generator/)
- [Web application install criteria](https://web.dev/articles/install-criteria)
- [Lighthouse installable manifest audit](https://developer.chrome.com/docs/lighthouse/pwa/installable-manifest)
- [WebKit manifest icon support](https://webkit.org/blog/12445/new-webkit-features-in-safari-15-4/)
- [Apple web-app icon guidance](https://developer.apple.com/library/archive/documentation/AppleApplications/Reference/SafariWebContent/ConfiguringWebApplications/ConfiguringWebApplications.html)

### Service-worker caching strategy

#### Precache allowlist

Precache only build-time, content-addressed application resources:

- `index.html`
- hashed Vue application JavaScript and CSS
- lazy route chunks generated from `mobile/src/router/index.ts`
- bundled fonts needed to render the shell
- manifest and install icons
- small static public assets required before authentication

Hash routing means an offline reload of `/#/markets` or `/#/trade/...` can load the same cached `index.html`; the fragment never reaches the server. The shell may render route structure, but it must not imply that account or market data is current.

Review the production build manifest instead of blindly increasing Workbox's default `maximumFileSizeToCacheInBytes`. `vite-plugin-pwa` fails builds when this threshold warning occurs. If a generated chunk is too large, split or exclude it; raising the ceiling should be a measured exception backed by asset-size and first-install impact.

#### Runtime network boundary

The safe initial policy is “no runtime application-data cache”:

| Request class | Policy | Reason |
| --- | --- | --- |
| `/api/v1/auth/**`, refresh, 2FA, password | Network only; server `Cache-Control: no-store` | Tokens and authentication state must never enter Cache Storage. |
| Orders, positions, margin, balances, wallets, transfers, withdrawals, KYC | Network only; no background replay | Responses are private and mutations are time/order sensitive. |
| Public market prices, candles, order books, tickers | Network only | Stale data can cause financial harm and is hard to label correctly across every screen. |
| News and announcements | Network only initially | Avoid accidental stale or personalized content; consider a later, explicit stale-tolerant policy only after product review. |
| `/ws/public` and any future private WebSocket | Direct WSS connection | Service workers do not intercept WebSocket frames; reconnect and snapshot reconciliation belong in application logic. |
| Hashed application assets | Precache / cache first by revision | Immutable content-addressed resources are safe and enable the offline shell. |
| Public brand/help images, if later approved | Explicit allowlist with Cache First plus expiration | Only non-sensitive, immutable resources should be considered. |

Do not add a broad `CacheFirst`, `StaleWhileRevalidate`, or hostname-based rule that could match API responses. Use path/method/origin allowlists, and keep navigation fallback denied for API and WebSocket paths so an API failure cannot accidentally return `index.html`.

Workbox `NetworkOnly` still uses the browser network stack, so origin response headers remain important. Authenticated/private responses should set `Cache-Control: private, no-store`; sensitive legacy responses should also be checked for intermediary/CDN caching. Service-worker route exclusion is the primary programmable-cache boundary, while HTTP cache headers are defense in depth.

#### Background Sync

Workbox Background Sync persists failed requests in IndexedDB and replays them later. This is unacceptable for financial mutations because user intent, price, balance, risk checks, session state, and idempotency windows may have changed. Do not queue or replay:

- create/cancel order
- margin or leverage actions
- deposits, withdrawals, internal transfers
- KYC uploads or submissions
- authentication, password, refresh, 2FA, or security changes
- any `POST`, `PUT`, `PATCH`, or `DELETE` unless a future endpoint is explicitly designed and audited for offline idempotency

Offline actions must fail closed with a clear message and require the user to resubmit after connectivity and fresh state are restored.

Official references:

- [Workbox caching strategy overview](https://developer.chrome.com/docs/workbox/caching-strategies-overview)
- [Workbox runtime caching](https://developer.chrome.com/docs/workbox/caching-resources-during-runtime/)
- [Workbox strategies](https://developer.chrome.com/docs/workbox/modules/workbox-strategies)
- [Workbox routing and navigation fallback](https://developer.chrome.com/docs/workbox/modules/workbox-routing)
- [Workbox Background Sync request replay](https://developer.chrome.com/docs/workbox/retrying-requests-when-back-online)
- [W3C Service Workers](https://www.w3.org/TR/service-workers/)
- [W3C note on WebSockets not using the Fetch algorithm](https://www.w3.org/TR/upgrade-insecure-requests/)
- [OWASP browser-cache weakness testing](https://owasp.org/www-project-web-security-testing-guide/latest/4-Web_Application_Security_Testing/04-Authentication_Testing/06-Testing_for_Browser_Cache_Weaknesses)
- [HTTP caching and private/no-store directives](https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/Caching)

### API, WebSocket, authentication, and cache security boundaries

#### Transport and origin

A production PWA must be served from HTTPS, with all API calls on HTTPS and sockets on WSS. The current production fallback `http://127.0.0.1:8080` (`mobile/src/config/app.ts:5`) cannot be used by a public PWA: it points at the end user's device and creates mixed content when the shell is HTTPS.

Prefer a same-origin edge/reverse-proxy layout:

- `https://mobile.example.com/` → PWA assets
- `https://mobile.example.com/api/v1/**` → backend HTTPS
- `wss://mobile.example.com/ws/**` → backend WebSocket

This reduces CORS complexity and makes the service-worker scope boundary easier to audit. The current Vite development proxy covers only the configured API prefix (`mobile/vite.config.ts:5`) and does not proxy `/ws/public`; web deployment and local test configuration must account for both.

Deployment headers should include:

- HTTPS redirect and HSTS after domain validation
- correct `application/manifest+json` or supported JSON manifest MIME type
- `Cache-Control: no-cache` or short revalidation for `index.html`, `sw.js`, and the manifest
- long-lived `immutable` caching only for hashed assets
- strict CSP for the web application and service-worker script
- `Cache-Control: private, no-store` on auth/account/financial API responses
- no service-worker script redirects to another origin

#### Token storage

The current web contract stores bearer access and refresh tokens in `localStorage` (`mobile/src/api/client.ts:5`). A PWA does not create this problem, but an installed, persistent shell raises the impact of an XSS or compromised service worker. Service workers are powerful, origin-scoped, and persistent; a hostile worker can observe or alter future same-origin traffic.

Long-term production web architecture should move browser sessions toward Secure, HttpOnly, SameSite cookies or a same-origin backend-for-frontend so JavaScript cannot read the refresh credential. That change affects backend/session contracts and is outside this research-only task. If the current local-storage contract remains for the first PWA release:

- do not load third-party scripts into the authenticated origin
- enforce a strict nonce/hash-based CSP and Trusted Types where browser support permits
- never place tokens in Cache Storage, manifest URLs, analytics, error reports, or install shortcuts
- redact authorization headers and WebSocket URLs from logs
- keep the existing one-shot refresh/retry contract; do not let a worker retry authentication
- clear tokens and private client stores on logout, but do not unregister a static-only service worker merely because a user logs out

The backend spec documents a future/private socket token in a query string. The mobile client currently uses only `/ws/public`. If a private socket is added, prefer an audited short-lived, one-use socket ticket or cookie-based handshake; query tokens can leak through logs and diagnostics even over TLS.

If private offline data is ever introduced, define deletion/versioning separately and consider server-side `Clear-Site-Data` on security-sensitive logout/account-compromise paths. The initial recommendation avoids private offline caches entirely.

Official references:

- [W3C Service Workers security model](https://www.w3.org/TR/service-workers/)
- [W3C Clear Site Data](https://www.w3.org/TR/clear-site-data/)
- [OWASP Session Management Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html)
- [vite-plugin-pwa deployment guidance](https://vite-pwa-org.netlify.app/deployment/)

### Install prompt UX

Chromium:

- Capture `beforeinstallprompt`, suppress the browser's automatic timing, and expose a user-initiated install action only after meaningful engagement.
- Do not show install UI during login, order entry, order confirmation, withdrawal, KYC, password, 2FA, or error recovery.
- Hide the action when already running in standalone mode, after `appinstalled`, and in every Tauri runtime.
- Persist only a non-sensitive dismissal/cooldown flag. Do not repeatedly prompt after rejection.
- Treat the prompt event as optional; browsers may not expose it even when installation is possible.

iOS/iPadOS:

- Safari does not expose the Chromium `beforeinstallprompt` flow.
- Provide a small, dismissible instructional surface: Share → Add to Home Screen.
- Show it only on supported iOS browser contexts, never in standalone mode or Tauri.
- Detect standalone with the standards-based display-mode media query and the iOS compatibility property where needed; do not rely on user agent alone.

Official references:

- [Custom install experience and `beforeinstallprompt`](https://web.dev/articles/customize-install)
- [Chrome install criteria](https://web.dev/articles/install-criteria)
- [WebKit bug: no `beforeinstallprompt` implementation](https://bugs.webkit.org/show_bug.cgi?id=193959)
- [Web Push and Home Screen web apps on iOS/iPadOS 16.4](https://webkit.org/blog/13878/web-push-for-web-apps-on-ios-and-ipados/)

### Update UX and version compatibility

Use prompt-based updates, not `autoUpdate`. The plugin's official auto-update guidance warns that automatic reloads can lose form data; that risk is unacceptable in a trading application.

Recommended behavior:

1. A waiting worker sets a global “new version available” state.
2. If the application is at a safe point, show “更新可用 / 立即更新 / 稍后”.
3. If an order, withdrawal, KYC, 2FA, transfer, or other dirty form is active, defer the prompt and never call `skipWaiting` automatically.
4. “立即更新” completes safe local persistence, asks the waiting worker to activate through `updateServiceWorker(true)`, and reloads once.
5. “稍后” keeps the current page under the old worker until the next safe prompt.
6. Check for worker updates when the online app regains focus/visibility and periodically (for example hourly), using a no-cache/no-store update request.

Backend and frontend deployments must tolerate at least the active and immediately previous web client version. A waiting service worker means old tabs and new shell assets can coexist during rollout; API removals cannot be coupled atomically to worker activation.

Official references:

- [Prompt for update](https://vite-pwa-org.netlify.app/guide/prompt-for-update.html)
- [Automatic update behavior and form-data warning](https://vite-pwa-org.netlify.app/guide/auto-update)
- [Periodic service-worker update checks](https://vite-pwa-org.netlify.app/guide/periodic-sw-updates)

### Offline shell behavior

“Offline ready” must mean only that the shell can open. It must not imply that trading, balances, orders, or market data work offline.

The offline shell should:

- render navigation, theme, localization, non-sensitive static help, and a clear connectivity banner
- mark API and WebSocket state separately; a browser may be online while the trading stream is disconnected
- show the timestamp of the last successful server snapshot
- mark prices/account state stale immediately when the socket is disconnected or data exceeds a defined freshness threshold
- disable order, cancel, leverage, transfer, deposit/withdrawal, KYC, password, 2FA, and similar mutation controls
- require a successful snapshot refresh after reconnect before re-enabling financial actions
- avoid rendering a cached authenticated screen as if the session were still valid

Two existing behaviors require implementation attention:

- `mobile/src/stores/market.ts:27` updates `updatedAt` even when REST refresh fails; success and attempt timestamps must be separated.
- `mobile/src/api/marketSocket.ts:65` reconnects with backoff but does not react explicitly to `online`, `offline`, or `visibilitychange`; foreground/reconnect should fetch an authoritative REST snapshot before data is considered fresh.

### Tauri coexistence and native registration guard

The web and native application currently share the same `npm run build` command (`mobile/package.json:8` and `mobile/src-tauri/tauri.conf.json:8`). A production integration should separate intent:

- `build:pwa`: PWA plugin enabled, manifest generated, worker emitted, registration permitted in browser runtime.
- `build:tauri`: PWA plugin `disabled: true`; manifest/head links, worker generation, and automatic registration disabled.
- Change Tauri `beforeBuildCommand` to the dedicated native build command.

Tauri sets hook environment variables such as `TAURI_ENV_PLATFORM`. Read that environment in `vite.config.ts` and use it as the primary build-time switch. A dedicated Vite mode can be used as a second, explicit signal, but should not replace the Tauri hook variable.

Registration must also be runtime-gated:

- create/reuse an explicit `isTauriRuntime()` based on `window.__TAURI_INTERNALS__`
- call the PWA registration composable only when it is a web build, not Tauri, and `serviceWorker` exists
- keep `injectRegister: null`; otherwise an injected `<script>` could register before Vue runtime checks execute
- keep development workers disabled

The existing `detectPlatform()` is not a complete registration guard because desktop Tauri can be classified as `desktop_web` (`mobile/src/core/platform.ts:4`). Use the raw Tauri-runtime predicate, not the user-facing platform enum.

Native builds should continue to use Tauri's updater and native bundle icons. Service-worker updates, manifest installation, and `beforeinstallprompt` are web-only concepts. Do not register a worker under Tauri's local production origins (`tauri://localhost` or `https://tauri.localhost`) even if a particular WebView happens to expose the API; WebView capabilities vary by OS/provider version.

Verification must assert `await navigator.serviceWorker.getRegistrations()` returns an empty list in desktop, iOS, and Android Tauri builds.

Official references:

- [Tauri Vite integration and build hooks](https://v2.tauri.app/start/frontend/vite/)
- [Tauri environment variables](https://v2.tauri.app/reference/environment-variables/)
- [Tauri Webview local URLs](https://v2.tauri.app/reference/javascript/api/namespacewebview/)
- [Tauri WebView versions and OS-provider behavior](https://v2.tauri.app/reference/webview-versions/)
- [vite-plugin-pwa manual registration](https://vite-pwa-org.netlify.app/guide/register-service-worker)

### iOS PWA constraints

- iOS/iPadOS 16.4 added standards-based Home Screen web-app behavior and Web Push for installed Home Screen apps. Push is not needed for the P0 PWA integration and should be a separate security/product task.
- Safari 26 allows any site to be added as a web app, but older supported iOS versions still depend on conventional manifest/installability behavior. Keep a complete standards-based manifest.
- `apple-touch-icon` takes precedence when supplied; provide the dedicated 180×180 asset.
- There is no site-triggered install prompt. Use manual Add to Home Screen instructions.
- The existing `viewport-fit=cover` and safe-area CSS are useful and should be tested in portrait/landscape standalone mode.
- Backgrounded JavaScript and WebSocket continuity cannot be assumed. On foreground, treat the stream as stale until reconnect and authoritative snapshot refresh complete.
- Website storage is best-effort and may be evicted. Never make Cache Storage, IndexedDB, or `localStorage` the sole record of a financial transaction or user intent.
- Test storage clearing/eviction and session expiry. An icon remaining on the Home Screen does not guarantee cached assets or valid credentials remain.
- If multiple installed instances or account-labeled installations are introduced later, preserve stable manifest identity semantics and ensure no credentials enter the manifest/start URL.

Official references:

- [Web Push for Home Screen web apps on iOS/iPadOS 16.4](https://webkit.org/blog/13878/web-push-for-web-apps-on-ios-and-ipados/)
- [Safari 26 web-app changes](https://webkit.org/blog/17333/webkit-features-in-safari-26-0/)
- [WebKit storage policy updates](https://webkit.org/blog/14403/updates-to-storage-policy/)
- [WebKit manifest icon support](https://webkit.org/blog/12445/new-webkit-features-in-safari-15-4/)
- [Apple standalone web-app configuration](https://developer.apple.com/library/archive/documentation/AppleApplications/Reference/SafariWebContent/ConfiguringWebApplications/ConfiguringWebApplications.html)

### Repository mapping: likely implementation changes

No application files were changed by this research. A later implementation agent will likely touch:

| Likely file | Intended change |
| --- | --- |
| `mobile/package.json` | Add pinned PWA dev dependency and separate PWA/Tauri build scripts. |
| `mobile/package-lock.json` | Lock reviewed plugin/Workbox dependency versions. |
| `mobile/vite.config.ts` | Add `VitePWA`, manifest, Workbox precache/navigation rules, `disabled` native-build switch, and web deployment base/origin handling. |
| `mobile/src/main.ts` | Initialize web-only PWA state after the explicit Tauri runtime guard. |
| `mobile/src/core/platform.ts` | Expose a reusable, explicit Tauri-runtime predicate that covers desktop Tauri. |
| `mobile/src/env.d.ts` and/or `mobile/tsconfig.json` | Add `vite-plugin-pwa/vue` virtual-module types and any typed build constants. |
| New `mobile/src/pwa/` modules | Encapsulate registration/update state, install prompt lifecycle, standalone detection, and online/offline state. |
| `mobile/src/App.vue` or global layout components | Render install, update, offline, stale-stream, and registration-error UX. |
| `mobile/index.html` | Align theme metadata and add the iOS touch icon if not injected by the plugin/assets integration. |
| New assets under `mobile/public/` | Add verified 192/512 `any`, 512 maskable, and 180 Apple icons. |
| `mobile/src/api/marketSocket.ts` | Add online/visibility recovery and explicit stale/snapshot reconciliation. |
| `mobile/src/stores/market.ts` | Separate successful freshness timestamp from failed refresh attempt time. |
| `mobile/src-tauri/tauri.conf.json` | Point `beforeBuildCommand` to the native/PWA-disabled build. |
| `mobile/.env.example` and `mobile/README.md` | Document HTTPS/WSS production origin, same-origin proxy option, build modes, deployment headers, and verification. |
| Deployment configuration (not found in repository) | Configure HTTPS, MIME types, caching, CSP, API no-store, routing, and WSS proxying. |

The native `csp: null` at `mobile/src-tauri/tauri.conf.json:24` deserves a separate Tauri security review. It is not necessary to widen this PWA task into that review, but the PWA implementation must not present a web CSP as protection for the native WebView.

### Test and verification plan

#### Static and dependency checks

1. Run `npm install` and inspect the lockfile.
2. Run `npm ls vite vite-plugin-pwa workbox-build workbox-window serialize-javascript`.
3. Run `npm audit`; resolve or explicitly assess all PWA/Workbox build-chain findings.
4. Run the repository's existing mobile checks:
   - `npm run type-check`
   - `npm test`
   - `npm run build`
5. Add and run the dedicated production commands:
   - `npm run build:pwa`
   - `npm run build:tauri`
6. Per the mobile spec, also build the affected native targets:
   - `npm run tauri:build:android`
   - `npm run tauri:build:ios`

#### PWA artifact inspection

- Confirm the web output includes a valid manifest, `sw.js`, the expected Workbox assets, and all declared icons.
- Confirm the manifest `id`, `scope`, `start_url`, and Vite `base` agree.
- Inspect the generated precache manifest: it must contain shell assets and must not contain API responses, auth data, source maps unless deliberately approved, or sensitive sample files.
- Confirm the native output contains no manifest link or worker registration. Prefer no PWA artifacts at all when `disabled: true`.
- Verify `sw.js`, `index.html`, and the manifest use revalidation/no-cache headers; hashed assets use immutable caching.

#### Browser automation against a production build

Use Playwright or an equivalent real-browser test against the served production build:

1. First online load installs the worker without registration errors.
2. A controlled page is obtained after reload.
3. Cold offline reload works for every important hash route and renders the shell.
4. Offline API reads fail visibly; no stale private response appears.
5. Financial mutation controls are disabled offline and remain disabled until authoritative refresh succeeds.
6. Cache Storage contains only approved shell assets.
7. IndexedDB contains no Background Sync queue for API mutations.
8. API, auth, and WebSocket URLs never receive the navigation fallback document.
9. Simulate version 1 → version 2 deployment and confirm the update prompt appears without automatic reload.
10. Verify “later”, safe-point “update now”, dirty-form deferral, and exactly one reload.
11. Test the custom Chromium install CTA, dismissal cooldown, standalone hiding, and `appinstalled`.

The plugin itself uses build tests plus browser/Playwright coverage; this application should follow the same production-build testing model rather than testing service workers only in Vite development.

#### DevTools/manual web checks

In Chrome/Edge DevTools Application:

- validate manifest fields and installability
- inspect maskable safe-area rendering
- inspect worker lifecycle, update, skip waiting, unregister, and clear-site-data behavior
- inspect Cache Storage and IndexedDB
- emulate offline and reload deep hash routes
- confirm no API response is stored
- confirm no old worker remains after local development

Official reference: [Chrome DevTools PWA inspection](https://developer.chrome.com/docs/devtools/progressive-web-apps/)

#### Tauri checks

Run desktop, Android, and iOS native builds:

- assert the explicit runtime Tauri predicate is true
- assert `navigator.serviceWorker.getRegistrations()` is `[]`
- assert no install/update/offline-ready PWA UI appears
- assert native bundle/update behavior is unchanged
- uninstall any worker left by an earlier experimental build before judging the guard

#### Real iOS device checks

On at least the oldest supported iOS version and a current version:

- Add to Home Screen from Safari and, where supported, another browser
- verify icon, display name, standalone launch, splash/background color, safe areas, portrait, and landscape
- verify the manual install instructions and standalone detection
- cold-launch offline and verify shell-only messaging
- background/foreground the app and confirm WebSocket plus REST snapshot reconciliation
- expire the session while suspended and confirm safe reauthentication
- clear/evict website data and confirm the application fails safely without implying transactions were retained

#### Network and deployment checks

Use `curl`, browser network inspection, and a WSS client to verify:

- HTTP redirects to HTTPS
- API and socket endpoints resolve to the intended production backend, never loopback
- manifest MIME type and service-worker scope
- cache headers for shell, worker, manifest, hashed assets, and sensitive APIs
- CSP on both application documents and `sw.js`
- authenticated endpoints are not cached by CDN/proxy/browser
- WSS reconnect and snapshot behavior under disconnect, packet loss, and server restart

Official reference: [vite-plugin-pwa service-worker testing](https://vite-pwa-org.netlify.app/guide/testing-service-worker)

### Related specs

- `.trellis/spec/mobile/index.md` — mobile package startup, type-check, test, build, and native build quality gates.
- `.trellis/spec/mobile/navigation-and-localization.md` — hash-route contracts, shared H5/iOS/Android source, and route/navigation validation.
- `.trellis/spec/backend/auth-sessions.md` — bearer token persistence, refresh/retry behavior, logout handling, and private WebSocket token contract.
- `.trellis/spec/backend/user-authentication.md` — authentication and security-sensitive user flows that must remain network-only.
- `.trellis/spec/backend/realtime-websockets.md` — `/ws/public`, future private socket, heartbeat, and realtime topic contracts.
- `.trellis/spec/guides/cross-layer-thinking-guide.md` — frontend/backend/deployment contract review.
- `.trellis/spec/guides/code-reuse-thinking-guide.md` — centralize PWA/runtime detection and avoid per-screen registration logic.

### External reference index and versions

| Area | Reference |
| --- | --- |
| Plugin version | [vite-plugin-pwa on npm](https://www.npmjs.com/package/vite-plugin-pwa) — latest observed 1.3.0 on 2026-07-28. |
| Plugin peer compatibility | [v1.3.0 package.json](https://raw.githubusercontent.com/vite-pwa/vite-plugin-pwa/v1.3.0/package.json) — Vite 3.1 through 8 peer range, including Vite 5. |
| Workbox version used by plugin | [v1.3.0 package.json](https://raw.githubusercontent.com/vite-pwa/vite-plugin-pwa/v1.3.0/package.json) — Workbox build/window 7.4.1 range. |
| Manifest standard | [W3C Web App Manifest](https://www.w3.org/TR/appmanifest/) — current draft consulted in 2026. |
| Service-worker standard | [W3C Service Workers](https://www.w3.org/TR/service-workers/) — secure-context, scope, lifecycle, and security model. |
| Tauri 2 | [Tauri Vite guide](https://v2.tauri.app/start/frontend/vite/) and [environment variables](https://v2.tauri.app/reference/environment-variables/). |
| iOS/WebKit | [iOS/iPadOS 16.4 Home Screen web apps](https://webkit.org/blog/13878/web-push-for-web-apps-on-ios-and-ipados/), [Safari 26 changes](https://webkit.org/blog/17333/webkit-features-in-safari-26-0/), and [storage policy](https://webkit.org/blog/14403/updates-to-storage-policy/). |

## Caveats / Not Found

- `python3 ./.trellis/scripts/task.py current --source` reported no active task/source even though `.trellis/tasks/07-28-mobile-prototype-to-pwa-redesign/task.json` exists with planning status. This report uses the exact task path supplied by the user; the Trellis current-task pointer remains unresolved.
- No production web/CDN/reverse-proxy configuration was found in the inspected repository. The exact public origin, Vite `base`, manifest scope, HTTPS certificate, cache headers, CSP, and `/ws` proxy cannot be finalized until deployment ownership is identified.
- No PWA package was installed and no application/native file was edited. Production builds, browser tests, dependency audit, icon generation, and native registration checks were therefore not executed in this research task.
- The official vite-plugin-pwa documentation site displayed version 1.2.0 while npm/package metadata showed 1.3.0. Package metadata and release artifacts should be checked again at implementation time.
- An open plugin issue reports an npm audit finding involving `serialize-javascript` through Workbox/Rollup tooling: [vite-plugin-pwa issue #921](https://github.com/vite-pwa/vite-plugin-pwa/issues/921). The exact resolved tree in this repository is not known until installation. Do not apply an unreviewed override; inspect the lockfile, run `npm audit`, and use a compatible patched transitive version or upstream fix only after build/test verification. Current package information: [serialize-javascript on npm](https://www.npmjs.com/package/serialize-javascript).
- The current production bundle and Workbox precache size were not generated because builds write outside the researcher agent's allowed task research directory. Implementation must inspect generated chunks and should not blindly raise `maximumFileSizeToCacheInBytes`.
- The recommended cookie/BFF authentication hardening changes the existing backend/session contract and is not part of a PWA-only edit. It needs a separate cross-layer design and spec update.
- iOS behavior varies materially by OS version and installation context. Simulator-only verification is insufficient; real-device Home Screen testing is required.
- No exact minimum iOS support version was found in repository specifications, so the real-device matrix cannot yet be narrowed.
