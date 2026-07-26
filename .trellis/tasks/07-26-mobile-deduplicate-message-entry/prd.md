# Remove Duplicate Home Message Entry

## Goal

Keep the header bell as the single global message-center entry, remove the
duplicate home message button, and let the home search control use the released
horizontal space.

## What I Already Know

- The header bell and home message bubble both navigate to `message-center`.
- The user approved removing the duplicate home entry.
- The scanner remains a distinct home utility action.
- The message center route and page must remain available through the bell.

## Requirements

- Remove the home message button and its notification dot.
- Preserve the header bell and its `message-center` navigation.
- Keep the scanner as one centered 44x44 circular utility button.
- Collapse the home utility action grid to one column so search expands.
- Update focused source contracts for the single home utility action.

## Acceptance Criteria

- [x] Home renders no message button or duplicate message-center click target.
- [x] Header bell still opens `message-center`.
- [x] Home scanner remains 44x44 and centered.
- [x] Home search expands into the released width.
- [x] 390x844 layout has no overflow or console errors.
- [x] Lint, production build and tests pass.
- [x] Exact validated source is deployed to the public Sites URL.

## Out Of Scope

- Changing message-center content.
- Removing the header notification bell.
- Changing scanner behavior or other navigation.

## Technical Notes

- Primary files:
  - `mobile/sites-prototype/app/page.tsx`
  - `mobile/sites-prototype/app/globals.css`
  - `mobile/sites-prototype/tests/rendered-html.test.mjs`
- Add a `.home-utility-actions` one-column override after the shared two-column
  action-cluster rule.
