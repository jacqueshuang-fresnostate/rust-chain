# Seconds stake input, cycle limits, and available balance

## User request

- Leave the stake input empty instead of automatically entering 500.
- Show minimum and maximum stake for each selected duration in `seconds-cycle-limit`.
- Show available funds on the Seconds page.

## Scope and decisions

Production Mobile Seconds only. The matched spot wallet (`stakeAssetId`) and existing exact decimal APIs remain authoritative. No backend, settlement, admin, PC, prototype, or deployment changes.

- Initial load, refresh, clearing the field, and duration selection must not insert a stake.
- Preserve a manually entered draft when changing duration and validate against the newly selected limits. Selecting another product clears the draft.
- Show both limits; an explicit nullable maximum means no product ceiling, not a missing/invalid bound. Invalid financial input stays unavailable and prevents review.
- Make available balance visible independently of the amount field. Distinguish exact zero, unknown/missing wallet, loading, and unauthenticated states. Never include locked funds or replace missing balances with zero.
- Replace the fixed form/operation height and truncated limit value with content-sized rows; preserve existing chart, buttons, keyboard interaction, and confirmation geometry.

## Implementation plan

1. Add failing financial-boundary and view-contract regressions.
2. Remove auto-fill, show balance and full limits, fix nullable maximum classification, localize Chinese/English copy.
3. Run targeted tests, Mobile release gate, and browser checks against isolated local API fixtures; document contracts and results.

## Acceptance

- [x] Blank amount on entry/reload/clear; no duration-based refill; explicit manual draft preserved.
- [x] Duration limits switch together and expose both ends without clipping, including no-maximum products.
- [x] Available funds use matched asset availableText; zero is visible and unknown is not fabricated.
- [x] Real Vue page checked at 390px/320px, light/dark, Chinese/English; review remains exact and no order is submitted during browser checks.
- [x] Mobile release gate, source budgets, progress and specification updates complete.
