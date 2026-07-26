# Mobile seconds trading, shaped navigation, and sticky header

## Context

The public mobile prototype has four related presentation gaps:

- The light theme still uses `rgba(11, 24, 17, 0.12)`, equivalent to
  `#0b18111f`, for borders.
- The seconds-contract route exists but is hidden in the product hub and uses a
  generic financial ticket instead of a recognizable trading workspace.
- The bottom navigation is a conventional rectangular six-column bar.
- The root header shares a stacking level with content rendered after it, so
  scrolling content can paint above the sticky header.

## Goals

- Replace the banned light-theme border family with a cooler neutral palette.
- Make seconds trading a prominent, dedicated entry and operational prototype.
- Turn the bottom navigation into a responsive shaped control with a raised
  seconds-trading action.
- Keep root and secondary headers above scrolling page content.

## Requirements

1. Light mode must not contain `#0b18111f` or its
   `rgba(11, 24, 17, 0.12)` equivalent. Related hard-coded light borders must
   use the same new neutral family.
2. The bottom navigation must expose the six root destinations plus a centered
   seconds-contract action.
3. The center action must be visually raised from the navigation body and the
   navigation background must have a non-rectangular shoulder/profile.
4. Every navigation action must keep a minimum 44x44 CSS pixel target and must
   fit from 320px through 448px without horizontal overflow or label collision.
5. Clicking the center action must open the typed `seconds` route and preserve
   the existing login-return behavior for guest sessions.
6. The seconds page must include pair, live reference price, round/cycle
   context, direction, duration, amount, estimated payout, balance, local
   confirmation, and session record feedback.
7. Seconds trading remains a deterministic local prototype and must not imply
   a real order or external side effect.
8. Root `.topbar` and `.secondary-header` must remain sticky above all scrolling
   content with an opaque readable surface; normal content must not share or
   exceed their stacking level.
9. Use Lucide icons only and no emoji.

## Acceptance criteria

- Source and rendered CSS contain no banned light-border color.
- Seconds trading is reachable directly from the bottom navigation.
- The seconds page is visually distinct from the generic product ticket and
  exposes all required trading information.
- The shaped navigation and raised action are stable at 320x844, 390x844, and
  448x900.
- Root and secondary headers remain unobstructed while scrolling.
- Light and dark themes retain readable contrast.
- `npm run lint`, `npm test`, and `git diff --check` pass.

