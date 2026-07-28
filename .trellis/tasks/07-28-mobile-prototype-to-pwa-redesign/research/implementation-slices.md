# Implementation Slices

## Shared shell

- `src/styles/base.css`
- `src/App.vue`
- `src/components/AppBottomNav.vue`
- `src/components/PageHeader.vue`
- `src/stores/theme.ts`
- `src/router/index.ts`

Contracts: seven root destinations, separate spot/seconds/contract entry points,
opaque sticky headers, safe areas, persisted light/dark theme, and no route-name
or route-meta regression.

## PWA runtime

- package and Vite configuration
- web manifest and official square icon set
- browser-only registration/install/update/offline state
- Tauri build-time and runtime exclusion
- localized PWA and message-center copy

Contracts: shell-only precache, no API/WebSocket/runtime data cache, no
background replay, prompt-based updates, and no service worker in Tauri.

## Root and product workspaces

- Home, Markets, Product Hub
- Trade, Seconds, Loan
- Assets, Profile, Security, Message Center

Contracts: preserve stores, requests, mutations, route parameters, authentication
guards, and error semantics while replacing presentation.

## Secondary-page waves

1. Market detail, news list/detail, shared chart/order-book/asset states.
2. Orders, swap, earn, prediction, new-coin list/detail/records.
3. Deposit/withdraw selectors and details, histories, ledger, quick recharge.
4. Login, registration, recovery, 2FA, KYC, bindings, referrals, language.

Contracts: no mock financial data, no hard-coded light-only surfaces, theme-aware
inputs/dialogs, 44px controls, and existing i18n/API behavior retained.

## Integration gates

1. Static scans: no emoji, no forbidden legacy light color, no unreviewed
   hard-coded `white` dialog surfaces, no broad service-worker runtime cache.
2. `npm run type-check`
3. `npm test`
4. `npm run build:pwa`
5. `npm run build:tauri`
6. `git diff --check`
7. Production-preview browser checks at 320x720, 390x844, and 448x900.
8. Manifest, service-worker control, approved cache contents, offline shell,
   update/install UI, and Tauri registration guard checks.
