# Elevate Mobile Prototype To An Award-Level Experience

## Goal

Elevate the existing public HIPPO mobile Sites prototype from a complete
exchange demo into a cohesive, award-level interactive art direction without
removing or weakening any of its 39 typed secondary pages, financial form
behavior, or exchange navigation.

## What I Already Know

- The user explicitly wants an Awwwards/FWA/CSSDA-level visual and interaction
  standard, experimental typography, fluid physical motion, code-driven
  rendering, imagery, and an immersive unified experience.
- The existing prototype is already public, has six root columns, separates
  Spot and Contract, removes the Web3 wallet entry, and exposes 39 typed
  secondary routes.
- The current source already contains canvas-rendered signal fields and trade
  charts, a desktop art stage, responsive phone canvas, light/dark themes,
  reduced-motion handling, and Lucide icons.
- The current production deployment is Sites version 5 at
  `https://hippo-mobile-signal-2026.ikuboy.chatgpt.site`.

## Assumptions

- Preserve the current OKX-referenced exchange information architecture and all
  complete workflows.
- Treat the award-level requirement as an art-direction and motion-system
  upgrade, not a marketing landing-page rewrite.
- Use the existing bitmap artwork as a real visual substrate and create one
  updated social-preview image after the final art direction is stable.
- Keep all transactional controls readable, predictable, and touch-safe even
  when surrounding editorial surfaces are experimental.

## Design Direction

### Signal Theatre

- A high-contrast black, white, signal-green, safety-coral and electric-blue
  palette with paper-white operational surfaces.
- Oversized kinetic typography and numeric typography used only for editorial
  moments, account totals, market indices, and transitions.
- An interactive signal mesh driven by pointer/touch position, market state,
  and theme, with deterministic nonblank fallback frames.
- A desktop exhibition stage with layered bitmap artwork, live market tape,
  vertical metadata and optical depth; mobile remains the actual product.
- Root-page transitions use a short signal-shutter sequence and direction-aware
  motion. Secondary pages use quieter push/pop transitions.
- Repeated list rows use restrained stagger, scan-line and focus effects;
  financial forms remain stable and do not physically shift on interaction.

## Requirements

### Visual System

- Rework shared color, type, spacing, border, shadow, texture and hierarchy
  tokens into one coherent art direction across light and dark themes.
- Give all six root columns a recognizable first viewport while preserving
  scanning density.
- Upgrade secondary headers, metrics, data tables, records and financial tickets
  so they belong to the same visual system.
- Avoid decorative cards, gradient orbs, excessive rounding, purple-dominant
  palettes, negative letter spacing, and marketing-style explanatory copy.

### Motion And Rendering

- Add a pointer/touch-reactive canvas field with particles, ribbons, scan lines
  and market-state color channels.
- Add a route transition veil and direction-aware page entry/exit language.
- Add deterministic stagger and micro-interaction tokens for rows, controls,
  tabs and CTA feedback.
- Respect `prefers-reduced-motion`: no perpetual movement, no large spatial
  transition, and no loss of information.
- Pause or reduce expensive rendering while the document is hidden.

### Functional Preservation

- Preserve all typed routes, route fallbacks, authentication return stack,
  parameter persistence and local-only financial records.
- Preserve independent Spot and Contract root columns and market selection.
- Preserve Lucide-only icon usage and the no-emoji contract.
- No real backend calls, real account mutation, payment handoff, KYC upload or
  financial transmission.

### Responsive And Accessibility

- No horizontal overflow or clipped sticky controls at 390x844 and 1440x900.
- Touch targets remain at least 44px where primary interaction requires it.
- Text remains legible over imagery and moving canvas layers.
- Focus-visible, selected, disabled, error and success states remain distinct.
- Canvas layers are decorative or have equivalent accessible text.

## Acceptance Criteria

- [x] The complete 39-route product remains reachable and functional.
- [x] All six root columns have upgraded art direction without changing their
      product responsibilities.
- [x] A pointer/touch-reactive canvas scene is visible, nonblank and performant.
- [x] Root and secondary route transitions feel related but appropriately
      different.
- [x] Financial forms do not shift, overlap, or lose validation clarity.
- [x] `prefers-reduced-motion` disables continuous and large spatial motion.
- [x] Lucide-only and no-emoji contracts pass.
- [x] 390x844 and 1440x900 browser checks show no overflow, clipping or overlap.
- [x] Lint, production build, tests, static route contracts and browser console
      checks pass.
- [x] The exact validated commit is deployed as a new public Sites version.

## Definition Of Done

- Implementation and focused regression tests are complete.
- Visual QA covers Home, Markets, Spot, Contract, Assets, Profile and
  representative secondary financial/auth pages.
- Canvas pixel checks confirm the rendered scene is nonblank.
- Public deployment is verified on a fresh production load.
- Progress and task records are updated.

## Out Of Scope

- Production mobile Vue changes under `mobile/src`.
- Backend or PC changes.
- Real wallet, order, payment, KYC or account integration.
- Pixel-copying any award site or third-party exchange.
- Adding a generic marketing landing page before the usable product.

## Technical Notes

- Primary files:
  - `mobile/sites-prototype/app/page.tsx`
  - `mobile/sites-prototype/app/secondary-pages.tsx`
  - `mobile/sites-prototype/app/globals.css`
  - `mobile/sites-prototype/tests/rendered-html.test.mjs`
  - `mobile/sites-prototype/app/layout.tsx`
- Existing image substrate: `mobile/sites-prototype/public/og.png`.
- Existing Canvas entry points: `SignalField` and `TradeChart`.
- The Sites project ID must be reused from `.openai/hosting.json`.

## Research References

- `research/award-interaction-patterns.md`
- `research/financial-ux-guardrails.md`
