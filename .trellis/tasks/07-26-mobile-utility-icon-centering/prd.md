# Center Home Utility Icons

## Goal

Center the scanner and message Lucide icons inside their circular home utility
buttons without changing the control size, group placement, notification badge,
or action behavior.

## What I Already Know

- The screenshot shows both utility SVGs aligned toward the left side of their
  circular buttons.
- `.icon-button` already uses grid centering, while `.utility-icon` does not
  define an equivalent internal layout.
- Both utility buttons already have stable 44px circular dimensions.

## Requirements

- Give every action-cluster icon button explicit internal centering.
- Remove default button padding from these icon-only controls.
- Keep all four controls at 44x44 and preserve their existing group gap.
- Preserve Lucide icons, notification dots, accessible names and click actions.
- Add a focused source contract test for the centering rule.

## Acceptance Criteria

- [x] Scanner and message SVG centers match their circular button centers.
- [x] Theme and notification SVGs remain centered.
- [x] Button dimensions, group gap and badge placement remain unchanged.
- [x] The 390x844 layout has no overflow or console errors.
- [x] Lint, production build and focused tests pass.
- [x] The exact validated source is deployed to the existing public Sites URL.

## Out Of Scope

- Changing icon artwork or size.
- Changing the search field or row layout.
- Changing action destinations or notification state.

## Technical Notes

- Primary files:
  - `mobile/sites-prototype/app/globals.css`
  - `mobile/sites-prototype/tests/rendered-html.test.mjs`
- The implementation should use `display: grid`, `place-items: center` and
  `padding: 0` on the shared action-cluster control selector.
