# Mobile light form and button polish

## Context

The mobile prototype currently exposes a bright inner `input:focus-visible`
outline in the trade ticket when keyboard focus lands on the readonly price or
quantity input. In light mode, form controls and option buttons also have weak
state hierarchy: inputs read as unfinished white boxes, and selected buttons
often change only their text color.

## Goals

- Move trade input focus feedback from the inner input to the complete field
  container.
- Keep a visible, accessible keyboard focus state without a double border.
- Give light-mode fields a coherent neutral surface, stronger labels, and clear
  focus/complete/error states.
- Improve light-mode primary, secondary, segmented, quick amount, and trade
  option button hierarchy without changing dark-mode behavior.
- Preserve existing routes, interactions, and mobile layout dimensions.

## Requirements

1. `.input-stack input:focus-visible` must not render its own outline.
2. `.input-stack label:focus-within` must visibly identify the full field using
   the design-system accent and must not move or resize the layout.
3. Light-mode `.field`, `.secondary-search`, and `.input-stack` surfaces must
   use a consistent bright neutral surface and readable border treatment.
4. Light-mode selected controls must have a filled or inset state, not rely on
   text color alone.
5. Light-mode primary actions must retain strong contrast; secondary and quick
   actions must remain visibly actionable and distinct from disabled controls.
6. All affected buttons must retain at least a 44px touch target.
7. Existing dark-mode colors, form validation behavior, and trade semantics
   must remain unchanged except for the corrected field-level focus treatment.

## Acceptance criteria

- Focusing the trade price or quantity field no longer produces an inner cyan
  rectangle; the complete field receives one stable focus treatment.
- Light-mode trade segmented controls, percentage buttons, and submit action
  expose clear active, inactive, hover/focus, and disabled hierarchy.
- Light-mode secondary forms use the same field surface and focus language as
  the trade ticket.
- CSS regression tests cover the focus override and light-mode state contracts.
- `npm test`, `npm run type-check`, and `npm run build` pass.
- Production browser verification confirms both light and dark trade states at
  a 390x844 viewport.

