# Mobile UI audit

## Audited surfaces

- Home
- Markets
- Spot
- Contract
- Assets
- Profile
- Product Hub
- Seconds Trading
- Message Center
- Loan
- Security Center

Viewport used for primary visual review: 390x844. Responsive contracts also
need verification at 320x844 and 448x900.

## Findings

### Product Hub

The surface is a five-row generic list followed by a large empty area. It does
not explain product differences, surface priority, limits, or availability.
This is the weakest secondary page and should become a compact product matrix.

### Message Center

Five categories use a three-column grid below 421px. The second row contains
only two controls, producing an unfinished layout. Five short Chinese labels
fit in one row at mobile widths.

Unread rows currently use a faint background gradient and a small dot. A
structural accent should supplement the dot.

### Seconds Trading

The dark market board is coherent in dark mode but visually conflicts with the
bright light theme. A theme-specific bright board can preserve the information
hierarchy without reintroducing the retired green-black border family.

### Bottom navigation

The shaped navigation is distinct and usable, but full-cell focus outlines are
visually intrusive. Keyboard focus should remain visible around the icon target.
The navigation should also declare its intended place in the established
content / navigation / route-transition / header stacking order.

### Loan

At 390px the two product cards stack because of a broad max-width rule. The
cards are short enough to compare side by side at this width, reducing page
length and improving decision ergonomics. They should still stack at 320px.

### Shared overview surfaces

Inbox and borrowing summaries use similar data structures but inconsistent
visual emphasis. They should share an operational band treatment with a
top-edge semantic accent and subtle theme-aware surface color.

## Preserved strengths

- Root views have clear domain-specific composition.
- Spot and Contract are correctly separated.
- Assets and Profile have strong first-viewport hierarchy.
- Message, Loan, Security, and Seconds workflows are functionally complete.
- Existing headers, fields, confirmation sheets, and local-only safeguards
  should be preserved.

