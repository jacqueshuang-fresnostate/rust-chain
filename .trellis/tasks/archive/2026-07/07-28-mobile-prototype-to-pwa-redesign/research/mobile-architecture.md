# Mobile Client Architecture Audit

## Existing application

- `mobile/` is a Vue 3 + Vite 5 + Pinia + Vue Router client packaged by
  Tauri 2 for iOS and Android.
- The real client already exposes 39 named routes spanning home, markets,
  market detail, spot and contract trading, swap, products, earn, loans, new
  coins, prediction, seconds contracts, orders, profile, KYC, security, account
  bindings, referrals, assets, deposits, withdrawals, wallet ledger, quick
  recharge, authentication, registration, and password recovery.
- Backend integrations already exist under `mobile/src/api/`; redesign work
  must preserve those request contracts and authenticated workflows.
- Root navigation currently has five entries. Spot and contract share one
  destination and seconds contracts are only reachable through Product Hub.
- The app currently has a light-only token set in `src/styles/base.css`.
  Route transitions, safe-area handling, icon controls, shared buttons, inputs,
  headers, and a fixed bottom navigation already exist and should be evolved
  rather than replaced with an unrelated component framework.
- Most feature views keep their own scoped CSS. A global token and primitive
  upgrade can improve every route while targeted templates/styles migrate the
  main surfaces from the Sites prototype.

## Prototype contracts to carry into the real client

- Official HIPPO metallic wordmark in the header.
- Cool-neutral light theme plus a high-contrast dark theme; no retired
  green-black translucent light borders.
- Lucide icons only and no emoji.
- Separate spot, seconds, and contract destinations.
- Seven-slot shaped navigation with seconds raised at the center.
- Opaque sticky headers above scrolling content and route transitions.
- Compact operational layouts: real metrics, dense market rows, visible
  selection states, framed fields, bottom confirmation sheets, and no nested
  decorative cards.
- Product Hub hierarchy: two featured products plus three secondary products.
- Light seconds-contract workspace remains bright; dark theme keeps the dark
  instrument panel.
- Minimum 44px interactive targets and no horizontal overflow from 320px
  through 448px.

## Implementation boundaries

- Preserve existing API modules, stores, route names, authentication redirects,
  financial validations, and Tauri behavior.
- Add aliases or metadata only when required for root navigation; preserve old
  deep links such as `/products/seconds`.
- Do not cache API, authentication, wallet, order, KYC, or WebSocket traffic in
  the service worker.
- Offline behavior is an application shell with explicit connectivity state,
  never fabricated market or account data.
- PWA install and update controls render only in browser/PWA contexts. Native
  Tauri builds must not register a service worker.

## Primary affected areas

- Shell and design system:
  `src/styles/base.css`, `src/App.vue`, `src/components/AppBottomNav.vue`,
  `src/components/PageHeader.vue`, route metadata, and a theme store.
- Main surfaces:
  `HomeView.vue`, `MarketsView.vue`, `ProductHubView.vue`, `TradeView.vue`,
  `SecondsView.vue`, `LoanView.vue`, `AssetsView.vue`, `ProfileView.vue`, and
  `SecurityView.vue`.
- PWA:
  `package.json`, lockfile, `vite.config.ts`, `index.html`, `src/main.ts`,
  `src/env.d.ts`, browser-only PWA state/component files, and `public/` icons.
- Quality:
  existing unit tests plus source/config contract tests for PWA and navigation.
