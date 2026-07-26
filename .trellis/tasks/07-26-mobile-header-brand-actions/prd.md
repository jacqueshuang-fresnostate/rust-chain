# Refine Mobile Header Brand And Action Controls

## Goal

Refine the public HIPPO mobile prototype header so the official metallic logo
feels integrated with the surface instead of appearing as a black sticker, and
unify the header and home utility controls into one precise visual language.

## What I Already Know

- The user supplied screenshots showing the current light-theme header.
- The official compact logo is rendered on a forced black rectangle with a
  coral offset shadow, which creates an unintended pasted-on appearance.
- Theme and notification actions are visually separate bare icons.
- Scan and message actions repeat the same pattern in the home utility row, but
  spacing, grouping and badge placement do not feel related to the header.
- All four functional controls already use Lucide icons and valid actions.

## Assumptions

- Preserve the official logo artwork and improve only its presentation.
- The transparent compact lockup remains the correct mobile header asset.
- The two header controls and two home utility controls should share one
  segmented control treatment.
- Keep every action at least 44x44 and preserve existing click behavior.

## Requirements

- Remove the forced black plate, border and coral block shadow from the mobile
  header logo.
- Render the transparent official lockup at a tighter optical size with
  theme-specific contrast treatment.
- Add a restrained brand accent that does not alter the logo artwork.
- Group theme and notification controls in one compact two-cell control rail.
- Group scan and message controls in the same component treatment.
- Normalize Lucide icon size, control dimensions, spacing and notification-dot
  placement across both rails.
- Keep the search field flexible and truncation-safe from 320px upward.
- Preserve all navigation and business behavior.

## Acceptance Criteria

- [x] The light-theme logo has no black rectangular backing or block shadow.
- [x] The official lockup remains legible in light and dark themes.
- [x] Header and home utility actions use the same two-cell control treatment.
- [x] Theme, notification, scan and message controls remain at least 44x44.
- [x] Notification dots align to the same corner position.
- [x] The 390x844 and 1440x900 layouts have no overflow or overlap.
- [x] Existing actions remain reachable and functional.
- [x] Lint, production build, focused tests and browser console checks pass.
- [x] The exact validated source is deployed to the existing public Sites URL.

## Definition Of Done

- Header branding and both action groups are visually verified.
- Regression tests cover the shared control structure and logo treatment.
- Public deployment and project progress records are updated.

## Out Of Scope

- Redrawing the official HIPPO logo.
- Changing navigation destinations or notification behavior.
- Modifying the production Vue mobile application.
- Changing secondary-page headers.

## Technical Notes

- Primary files:
  - `mobile/sites-prototype/app/page.tsx`
  - `mobile/sites-prototype/app/globals.css`
  - `mobile/sites-prototype/tests/rendered-html.test.mjs`
- Existing brand asset: `mobile/sites-prototype/public/hippo-logo-compact.png`.
- Existing public Sites project must be reused.
