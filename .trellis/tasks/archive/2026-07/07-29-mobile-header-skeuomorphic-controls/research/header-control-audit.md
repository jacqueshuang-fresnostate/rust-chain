# Header Control Audit

## Current Surfaces

| Surface | Controls | Current geometry |
| --- | --- | --- |
| `RootHeader.vue` | theme, message | 44px circles in a two-column action cluster |
| `PageHeader.vue` | back, slotted action | 44px square controls in a 76px sticky header |
| `LoginView.vue` | back/close, language | 44px controls in an independent sticky header |
| `RegisterView.vue` | back, language | 44px controls in an independent sticky header |
| `MarketDetailView.vue` | back, share | 44px controls in an independent sticky header |

## Browser Baseline

Measured at 390x844 on `/seconds` in the light theme:

- Header: 390x76px, sticky, z-index 70, opaque `rgb(251, 252, 250)`.
- Back: 44x44px, `border-radius: 0`, transparent face, no box shadow.
- Refresh: 44x44px, `border-radius: 0`, transparent face, no box shadow.
- Both SVG centers match their button centers.
- Document horizontal overflow: 0px.

The RootHeader controls are circular but use a flat surface, single border, and
no tactile depth. The notification dot sits outside the upper-right rim.

## Structural Finding

`PageHeader.vue` renders the action slot inside a 44x44px
`.secondary-header-action` span, while each consuming view renders another
44x44px `.icon-button`. Both elements currently receive control framing from
the prototype CSS. The production treatment must make the wrapper a
transparent alignment track and render depth only on the interactive child.

## Design Decision

Use one CSS-owned header control system rather than changing every page:

- circular precision-instrument silhouette;
- cool-neutral bezel and convex face;
- light source from upper-left, restrained lower-right shadow;
- 1px physical press travel with compressed shadow;
- cyan focus ring and coral unread signal;
- theme-specific material values and no warm beige cast;
- no pseudo-SVG, emoji, or new runtime dependency.

## Relevant Contracts

- `.trellis/spec/mobile/index.md`
- `.trellis/spec/mobile/pwa-and-shell.md`
- `.trellis/spec/mobile/navigation-and-localization.md`

