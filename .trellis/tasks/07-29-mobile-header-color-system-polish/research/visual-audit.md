# Production Mobile Header And Color Audit

## Baseline

- Production uses a tracked visual snapshot in `prototype-base.css` and a runtime correction layer in `prototype-parity.css`.
- Root and secondary headers already satisfy the required 64px/76px geometry, sticky positioning, `z-index: 70`, and 44px icon targets.
- Functional handlers and Lucide icon usage are already correct.

## Observed Problems

- Light `--page`, `--surface`, and `--surface-2` are visually too close. Loan, security, and trade surfaces lose hierarchy.
- Dark colors remain green-biased rather than neutral. The logo has low contrast against the root header.
- The disabled home market brief can render a bright surface while its inherited internal copy stays light, producing unreadable text.
- Header control shadows and bevels are stronger than the surrounding UI and need a tighter neutral-metal treatment.
- Secondary header title and metadata hierarchy is too quiet relative to the large return/action controls.
- Fields, secondary actions, status cards, and selected controls do not share one production surface hierarchy.

## Implementation Direction

- Keep semantic state colors but move structural colors to a cool-neutral graphite/white system.
- Define header surface, border, shadow, and control material tokens once per theme.
- Add explicit dark and light market-brief colors instead of relying on inverted `--text`/`--page` roles.
- Strengthen field and card borders through shared production selectors while preserving their dimensions.
- Keep bright signal green for fills and charts; use contrast-safe role colors for small status text.

## Validation Targets

- RootHeader, PageHeader, Home, Trade, Profile, Message Center, Loan, and Security Center.
- Light and dark modes at 390x844.
- Horizontal overflow at 320x720, 390x844, and 448x900.
- Header dimensions, z-index, icon geometry, focus ring, and reduced-motion behavior.
