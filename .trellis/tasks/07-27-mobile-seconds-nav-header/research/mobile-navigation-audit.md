# Mobile navigation and seconds-contract audit

## Findings

- The final light theme declares `--line: rgba(11, 24, 17, 0.12)`, which is the
  eight-digit color `#0b18111f`.
- Additional light-only controls reuse the same RGB family with several alpha
  values, so replacing only the variable would leave the rejected hue in
  buttons and fields.
- `seconds` is already a typed protected route and has a product-hub entry, but
  the page only renders pair, direction, cycle, amount, and a generic ticket.
- The root navigation contains six root routes and is hidden on all secondary
  routes. A raised center action can therefore open `seconds` without changing
  root-route ordering or fallback behavior.
- `.topbar` and `.view-stack` both resolve to `z-index: 2`; because the view
  stack is rendered later, transformed or positioned descendants can cover the
  sticky header.
- `.secondary-header` has a local z-index but sits inside the view stacking
  context, so it also needs an explicit final header layer contract.

## Direction

- Replace the light border family with cool graphite-blue neutrals.
- Keep the six root destinations and insert a dedicated raised seconds action
  between the third and fourth root items.
- Build the irregular navigation body with CSS pseudo-elements and
  `clip-path`; keep semantics in normal buttons.
- Upgrade the existing typed seconds route instead of adding a duplicate route.
- Set final header stacking rules after all legacy layers so they cannot be
  overridden by earlier duplicate declarations.

