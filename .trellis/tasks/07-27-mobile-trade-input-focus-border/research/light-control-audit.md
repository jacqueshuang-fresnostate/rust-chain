# Light control audit

## Observed behavior

- The global `input:focus-visible` rule can draw a two-pixel cyan outline around
  the inner trade input, while the parent field keeps its original border.
- Trade `input` elements are readonly but focusable, so keyboard navigation
  still reaches them and exposes the global focus ring.
- In the deployed light theme, selected trade controls mostly use transparent
  backgrounds with green text. Percentage and side selectors therefore look
  similar to inactive controls.
- Secondary form fields use white surfaces but mix several border and focus
  treatments, weakening the visual relationship between trade, loan, and
  security workflows.

## Implementation direction

- Use `:focus-within` on the field container and suppress only the nested trade
  input outline.
- Keep color semantics: cyan for focus, green for buy/complete, coral for
  sell/error.
- Scope new surface and button treatments under `.theme-light` so the dark
  visual system stays stable.
- Prefer border, inset line, and restrained shadow changes that do not affect
  geometry.

