# Redesign the Real Mobile Client and Add PWA Support

## Goal

Migrate the approved HIPPO Sites prototype visual system and navigation model
into the real `mobile/` Vue client without replacing its backend integrations,
then make the Web build a production-grade installable PWA that coexists safely
with the existing Tauri iOS and Android applications.

## What I Already Know

- The user approved the public Sites prototype as the design reference.
- The real mobile client already contains the functional routes and API
  integrations; this task is a redesign and platform-capability upgrade, not a
  mock replacement.
- Earlier prototype decisions remain binding: Lucide icons only, no emoji, no
  Web3 wallet, and spot, seconds, and contract trading are separate columns.
- The task runs unattended, so non-blocking product decisions use the
  recommended approach documented here rather than pausing for preference
  questions.

## Assumptions

- Both light and dark themes are part of the approved prototype and should be
  available in the real client with persisted preference.
- The PWA should be installable on Chromium and iOS Safari, provide an offline
  application shell and update prompt, but must not claim that trading or
  account data works offline.
- Existing route names and API payloads are compatibility contracts.
- Push notifications are not included because there is no requested server-side
  Web Push subscription/VAPID implementation.

## Requirements

### 1. Shared visual system

- Replace the current generic light-only tokens with a HIPPO design system for
  light and dark themes: page, surface, elevated surface, ink, muted text,
  cool-neutral borders, green positive, red negative, blue focus, and orange
  brand signal accents.
- Use the official HIPPO wordmark and compact mark from the approved prototype.
- Keep typography compact and operational, with tabular financial values.
- Shared buttons, inputs, selects, dialogs, lists, empty/error/success states,
  and icon controls must work in both themes.
- Interactive controls must be at least 44x44 CSS pixels and keyboard focus
  must remain visible.

### 2. Shell, headers, and navigation

- Upgrade root navigation to seven visible destinations:
  Home, Markets, Spot, Seconds, Contract, Assets, and Profile.
- Spot and Contract reuse the real trade route with distinct persisted modes.
  Seconds uses the real seconds-contract route and remains separately reachable
  from Product Hub.
- Use a shaped navigation body with the center Seconds action raised, while
  avoiding page overflow at 320-448px.
- Root and secondary headers must be opaque sticky layers above route
  transitions and content.
- Preserve safe-area behavior and route-back fallbacks.

### 3. Main application surfaces

- Home: official brand header, theme and notification/news actions, search,
  real authenticated asset estimate where available, dense feature shortcuts,
  market signal band, real market rows, and announcements.
- Markets: prototype-style signal headline, search, category controls, real
  metrics and ticker rows, including trade-pair picker mode.
- Product Hub: operational two-featured/three-secondary matrix using real
  routes for Earn, Loan, New Coins, Prediction, and Seconds.
- Trade: preserve real spot and contract order flows while visually separating
  modes and upgrading instrument, order-book, form, balance, and order actions.
- Seconds: preserve real products/orders/API actions and present a dedicated
  short-cycle workspace with bright light-theme and dark-theme instrument
  panels.
- Loan: preserve real products, applications, orders, cancellation, and
  repayment while presenting comparable product choices and a consistent
  bottom form.
- Assets and Profile: preserve real account data and mutations while adopting
  the prototype hierarchy and action treatment.
- Message Center: connect the Home notification action to a real secondary
  route backed by platform announcements, with truthful local read-state
  tracking and no fabricated account or transaction notifications.
- Security receives targeted layout polish because of its high density and
  financial sensitivity.
- All remaining functional routes are included in the visual migration:
  market detail, orders, swap, earn, prediction, new-coin products/details/
  records, news list/detail, deposit and withdrawal selectors/details/history,
  wallet ledger, quick recharge, login/register/recovery/2FA, KYC, account
  bindings, referrals, and language settings.
- Secondary pages must retain their distinct information architecture while
  replacing hard-coded light surfaces, legacy focus borders, and generic
  dialogs with the shared theme-aware primitives.

### 4. PWA

- Add a standards-compliant Web App Manifest with HIPPO name, short name,
  standalone display, portrait orientation, theme/background colors, start URL,
  scope, and official branded icons including maskable coverage.
- Generate a production service worker that precaches only compiled application
  shell assets and supports navigation fallback.
- Never runtime-cache `/api/`, authorization, wallet/order/KYC responses, or
  WebSocket traffic.
- Add browser-only install availability, install action, online/offline state,
  and update-ready action with localized Chinese and English copy.
- Do not register or activate the PWA service worker inside Tauri.
- Add mobile-web and iOS metadata such as Apple touch icon and standalone
  capability.

### 5. Compatibility and localization

- Preserve all existing API contracts, route names, authentication redirects,
  stores, and Tauri packaging.
- New visible copy must exist in both `zh-CN` and English locale files.
- Use Lucide icons for all interface controls and no emoji.
- Do not add a new general-purpose UI framework.

## Acceptance Criteria

- [x] Seven root navigation destinations are visible and Spot, Seconds, and
      Contract open separate real operational routes/modes.
- [x] Home, Markets, Product Hub, Trade, Seconds, Loan, Assets, Profile, and
      Security visibly follow the approved prototype system.
- [x] Every route-backed secondary page renders coherently in both themes; no
      legacy hard-coded white dialog or light-only field remains.
- [x] Home notifications open the announcements-backed Message Center and no
      fake account, order, or funds notification is displayed.
- [x] Existing authenticated API actions remain wired and no financial workflow
      is replaced by mock data.
- [x] Light and dark themes render readable controls, fields, charts, dialogs,
      and status colors and persist across reloads.
- [x] No horizontal page overflow or incoherent overlap at 320x720, 390x844, or
      448x900.
- [x] Manifest, branded icons, standalone mode, service worker, offline shell,
      install prompt, connectivity state, and update prompt are present.
- [x] API and WebSocket requests are absent from service-worker runtime cache
      rules.
- [x] Tauri runtime does not register the service worker.
- [x] All new copy is localized in Chinese and English.
- [x] `npm run type-check`, `npm test`, `npm run build`, and `git diff --check`
      pass.
- [x] Built Web output is validated in a browser for installability, manifest,
      service-worker control, offline reload, theme switching, root navigation,
      and key page layouts.

## Out of Scope

- Backend API or database changes.
- Web Push notifications and server-side subscription management.
- Fabricated offline market/account/order data.
- Replacing Tauri with PWA-only distribution.
- Changing financial validation or permission rules except where a UI bug makes
  existing behavior unreachable.

## Technical Notes

- Architecture audit:
  `research/mobile-architecture.md`
- PWA research:
  `research/pwa-integration.md`
- Applicable project specs:
  `.trellis/spec/mobile/index.md`,
  `.trellis/spec/mobile/navigation-and-localization.md`,
  `.trellis/spec/guides/cross-layer-thinking-guide.md`, and
  `.trellis/spec/guides/code-reuse-thinking-guide.md`.

## Definition of Done

- Implementation and regression tests are complete.
- Responsive and PWA browser checks are recorded.
- Project specs and `docs/superpowers/PROGRESS.md` capture the new contracts.
- Work is committed, the Trellis task is archived, and the session is recorded.
