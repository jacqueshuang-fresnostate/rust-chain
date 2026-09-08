# Frameless chart toggle implementation

## Scope

- Changed only the chart-toggle CSS in `mobile/src/views/MarketDetailView.vue`
  and its existing regression in `mobile/tests/market-detail-reference-layout.test.ts`.
- Preserved existing dirty work and left view script/chart wiring to the other
  implementation slices. No shared styles, backend, specs, Git state, or progress
  log were changed by this slice.

## Changes

- Replaced the normal 1px border with `border: 0`.
- Replaced normal, active, and focus-visible inset/outer shadows with
  `box-shadow: none`; removed the unused shadow transition.
- Kept the high-specificity scoped selector that outranks the legacy dark-theme
  inset-shadow rule in `prototype-parity.css`. Expanded mode inherits the same
  frameless treatment and only changes its existing position.
- Preserved the 44px square hit target, 12px radius, centered 18px icon,
  theme-derived icon color, backdrop blur, inline/expanded placement, button
  semantics, pressed state, and keyboard focus outline.
- Active state retains 1px press travel and changes the theme-derived surface
  fill, so reduced-motion mode can suppress movement without losing visible
  press feedback. This fill adds no border or shadow.

## Verification

- Test-first regression failed against the old visible border (expected `0`,
  received `1px solid color-mix(...)`). A second test-first assertion failed
  when active background feedback was absent, then passed after implementation.
- Ran from `mobile/`:
  - `node --test --experimental-strip-types --test-name-pattern='图表切换按钮' tests/market-detail-reference-layout.test.ts`
    — passed 1/1 after border/shadow removal.
  - `node --test --experimental-strip-types tests/market-detail-reference-layout.test.ts`
    — final run passed 12/12 after active-fill feedback was added.
- The focused regression executes Vue scoped CSS compilation, checks that all
  three compiled toggle paint states declare no box shadow and outrank the
  legacy dark shadow rule, and checks expanded placement, inherited frame
  removal, icon color, target size, semantics, focus outline, and reduced motion.
- Reviewed scoped `diff -u` against pre-edit temporary copies: only the toggle
  CSS and its existing test section changed. Full Mobile gate and browser
  computed-style/geometry verification remain with the parent task.
