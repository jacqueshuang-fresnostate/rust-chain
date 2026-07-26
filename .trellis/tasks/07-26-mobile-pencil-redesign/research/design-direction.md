# Mobile Prototype Design Direction

## Product Constraint

The prototype is a financial trading product. Experimental presentation should
create identity and momentum, but prices, balances, order controls, risk labels,
and navigation must remain instantly scannable.

## Direction

Use a "signal field" visual system:

- Dense, oversized typography acts as spatial structure on Home and product
  discovery surfaces.
- A live canvas field and restrained parallax create depth behind functional UI.
- Luminous green communicates positive price movement and primary buy actions.
- Warm coral communicates negative movement and sell actions.
- Electric cyan is reserved for information, focus, and secondary highlights.
- Off-white type and graphite surfaces keep the palette grounded.

## Interaction Conventions

- Five persistent bottom tabs reflect the production navigation contract.
- View transitions combine opacity, vertical momentum, and directional blur.
- Market rows and product actions use stable dimensions to avoid layout shift.
- Trade controls are denser and calmer than discovery views.
- Touch targets remain at least 44 px.
- Reduced-motion users receive direct state changes without ambient animation.

## Implementation Notes

- React plus CSS handles application state, view transitions, and controls.
- Lucide React provides the complete icon set.
- Canvas renders a lightweight responsive price/signal field.
- Local state drives favorites, filters, order mode, percentage selection, theme,
  and simulated confirmations.
- The Sites build stays separate from `mobile/src/` and from the existing static
  prototype in `mobile/design/`.
