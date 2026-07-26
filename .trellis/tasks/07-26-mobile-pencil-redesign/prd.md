# Immersive Mobile Prototype With Sites

## Goal

Create and publish an interactive, high-fidelity mobile prototype that translates
the existing PC trading workflows and backend capabilities into a distinctive
mobile experience. The result should feel like a premium digital product rather
than a static concept page, while keeping high-frequency trading actions clear,
reachable, and credible.

## What I Already Know

- The current mobile router covers home, markets, market detail, spot and
  contract trading, convert, product center, earn, loan, new coin, prediction,
  seconds contracts, orders, profile, security, assets, deposit, withdrawal,
  ledger, quick recharge, and authentication.
- Backend APIs already expose market data, spot and margin orders, convert,
  earn, loan, seconds contracts, prediction, new coin, wallet, profile, KYC,
  security, referral, and authentication capabilities.
- The existing standalone prototype under `mobile/design/` covers only a subset
  of the product and remains independent from production mobile screens.
- The prototype must preserve the five main tabs: Home, Markets, Trade, Assets,
  and Profile.
- User requested Sites as the design and publishing surface.

## Requirements

- Build a standalone Sites-compatible prototype under `mobile/sites-prototype/`.
- Redesign version 2 using current OKX mobile information hierarchy and visual
  restraint as a reference without copying OKX branding or exact screens.
- Make account valuation, daily performance, buy crypto, and deposit the first
  home-screen workflow.
- Replace the oversized experimental home hero with a compact exchange workspace:
  search/utilities, balance performance, primary funding actions, feature grid,
  promotion/insight banner, and ranked market tabs.
- Use black, white, and neutral gray as the primary visual system. Reserve HIPPO
  green plus market red for status, live data, and buy/sell semantics.
- Prefer thin dividers, compact spacing, near-square controls, 4-8 px radii, and
  bold numeric typography over decorative card composition.
- Use Lucide icons consistently for all interface icons.
- Do not use emoji in interface copy, decorative content, source-authored UI, or
  status indicators.
- Optimize the primary experience for a 390 px mobile viewport while remaining
  responsive on wider browser widths.
- Provide five interactive main views: Home, Markets, Trade, Assets, and Profile.
- Include realistic entry points for Convert, Earn, Loan, New Coin, Prediction,
  Seconds Contracts, Deposit, Withdraw, Quick Recharge, Security, KYC, Invite,
  and Orders.
- Use realistic trading data and product-specific Chinese copy.
- Make the browser feel like an interactive visual canvas through kinetic
  typography, fluid transitions, tactile controls, layered depth, responsive
  chart rendering, and pointer/touch-reactive motion.
- Keep critical financial workflows legible and restrained: balances, prices,
  order controls, primary actions, risk labels, and navigation must not be
  obscured by decorative motion.
- Support meaningful interactions rather than static screenshots: tab switching,
  market filtering, favorite toggling, spot/contract mode switching, buy/sell
  switching, amount controls, product overlays, theme toggling, and simulated
  order confirmation.
- Use a non-monochromatic palette with dark neutral surfaces, luminous green,
  warm coral, electric cyan, and restrained off-white.
- Respect reduced-motion preferences.
- Publish the validated prototype through Sites and return the production URL.

## Acceptance Criteria

- [x] The first viewport clearly presents a mobile trading product, not a
      marketing landing page.
- [x] All five main tabs switch to complete, distinct views without reloading.
- [x] Home exposes the major backend/PC product areas through reachable
      interactions.
- [x] Markets supports category filtering and favorite state.
- [x] Trade supports spot/contract and buy/sell modes, an interactive amount
      control, and a simulated order result.
- [x] Assets exposes deposit, withdraw, transfer, and quick recharge actions.
- [x] Profile exposes KYC, security, invite, language, and theme controls.
- [x] Every interface icon comes from Lucide; no emoji appears in the rendered UI.
- [x] At 390 px width there is no horizontal overflow, bottom navigation does
      not cover content, and all primary controls remain reachable.
- [x] Animations remain smooth and reduced-motion mode disables nonessential
      motion.
- [x] The production build succeeds and the published Sites deployment reaches a
      terminal successful state.

## Definition Of Done

- The complete interaction flow is implemented in the standalone prototype.
- The deployment build passes.
- Mobile browser behavior is checked at 390 px and a wide desktop viewport.
- The exact validated source is saved and deployed through Sites.
- Project progress is recorded in `docs/superpowers/PROGRESS.md`.

## Out Of Scope

- Wiring the prototype to live backend endpoints or authenticated production
  user data.
- Replacing existing Vue screens under `mobile/src/`.
- Implementing real order placement, wallet transfers, deposits, withdrawals, or
  identity verification.
- Reworking PC or backend business logic.

## Technical Notes

- Follow `.trellis/spec/mobile/navigation-and-localization.md` for the five-tab
  navigation model and mobile preview constraints.
- Reference study: `research/okx-mobile-reference.md`.
- Use a dedicated Sites project so deployment metadata does not alter the
  existing Vue/Tauri mobile application.
- Keep simulated state local to the prototype.
- Prefer CSS, Canvas, and Lucide components for interface visuals. Use one
  generated raster asset only when it adds a coherent brand moment rather than
  replacing functional UI.

## Open Questions

- None. The repository and the user's explicit visual requirements provide
  enough context to proceed with a first complete version.
