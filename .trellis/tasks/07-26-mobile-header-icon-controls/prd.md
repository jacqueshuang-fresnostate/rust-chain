# Replace Segmented Header Controls With Standalone Icons

## Goal

Remove the bulky segmented containers around the header and home utility icons,
replacing them with lightweight standalone circular controls that preserve the
shared interaction language without looking like large grey cards.

## What I Already Know

- The user rejected the current two-cell segmented control treatment.
- The screenshot shows the outer rounded rectangle, central divider and detached
  badge as the dominant visual problem.
- Theme, notification, scan and message actions are already functional and use
  Lucide icons.
- The previous revision established consistent 44px touch targets.

## Requirements

- Remove the outer background, border, radius, padding and internal divider from
  both action groups.
- Keep the two actions horizontally aligned with a restrained gap.
- Render each action as an independent 44px circular icon control.
- Use a subtle surface and border in light and dark themes.
- Anchor notification dots tightly to the affected circular button.
- Preserve all action behavior, accessible names and responsive search sizing.

## Acceptance Criteria

- [x] Neither action group has a shared rounded rectangle or central divider.
- [x] All four controls are independent 44x44 circles.
- [x] Notification dots remain attached to the top-right button corner.
- [x] Light and dark themes retain sufficient icon and boundary contrast.
- [x] 390x844 and 1440x900 layouts have no overflow or overlap.
- [x] Lint, production build, focused tests and console checks pass.
- [x] The exact validated source is deployed to the existing public Sites URL.

## Out Of Scope

- Changing the HIPPO logo.
- Changing action destinations or notification state.
- Modifying any secondary page or financial workflow.

## Technical Notes

- Primary files:
  - `mobile/sites-prototype/app/globals.css`
  - `mobile/sites-prototype/tests/rendered-html.test.mjs`
- Existing markup and accessibility labels remain unchanged.
