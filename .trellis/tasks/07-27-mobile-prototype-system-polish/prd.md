# Mobile prototype system polish

## Goal

Polish the existing public HIPPO mobile prototype after a complete visual and
interaction audit. Preserve the current business flows while improving the
weakest surfaces and resolving system-level inconsistencies across root and
secondary pages.

## What is already known

- Root Home, Markets, Spot, Contract, Assets, and Profile pages already have
  complete layouts and working local interactions.
- Product Hub is still a plain action list with excessive empty space and does
  not communicate product differences.
- Message Center wraps five categories into an incomplete three-column grid at
  common mobile widths.
- The light-theme seconds market board remains a large dark panel despite the
  rest of the page using a bright surface.
- Bottom navigation focus leaves a large blue rectangular outline after
  interaction in the in-app browser.
- Loan product comparison collapses to one column at 390px, making comparison
  slower and the page unnecessarily long.
- Existing routes, local-only mutation safeguards, confirmation flows, Lucide
  icon policy, and typed navigation must remain intact.

## Requirements

1. Replace Product Hub's generic action list with a compact operational product
   matrix that distinguishes featured and secondary products, shows meaningful
   product metadata, and keeps every existing destination reachable.
2. Keep Message Center's five category controls on one balanced row from 320px
   through 448px without overflow or clipped labels.
3. Strengthen unread-message differentiation without relying only on text
   color.
4. Give light-theme seconds trading a bright, high-contrast market board while
   retaining the same information hierarchy and a dark-theme counterpart.
5. Move bottom-navigation keyboard focus treatment to the icon target instead
   of outlining the full navigation cell. Preserve visible keyboard focus.
6. Set an explicit bottom-navigation layer below route transitions and sticky
   headers but above normal content.
7. Keep the two loan products side by side where the content width can support
   it, while preserving a single-column layout on narrow 320px devices.
8. Improve shared inbox and borrowing overview surfaces with consistent
   operational accents in light and dark themes.
9. Preserve all existing local-only behavior, authentication return paths,
   confirmation dialogs, typed routes, responsive constraints, and Lucide-only
   icon usage.

## Acceptance criteria

- Product Hub no longer renders through the generic `ActionList`.
- Product Hub exposes Earn, Loan, New Coins, Prediction, and Seconds routes.
- Message categories render as five equal tracks at 320px, 390px, and 448px.
- Light seconds market board uses a light surface and readable dark text.
- Bottom-navigation focus does not draw a large rectangular outline.
- Bottom navigation has an explicit layer below sticky headers.
- Loan products render in two columns at 390px and one column at 320px.
- No page has horizontal overflow at 320x844, 390x844, or 448x900.
- Light and dark theme screenshots retain readable contrast.
- `npm run lint`, `npm run build`, `npm test`, and `git diff --check` pass.

## Out of scope

- Real backend integration or data persistence.
- Changes to trading, lending, security, or wallet business behavior.
- New root routes or removal of existing destinations.
- Redesigning already strong root pages from scratch.

## Research reference

- `research/mobile-ui-audit.md`

