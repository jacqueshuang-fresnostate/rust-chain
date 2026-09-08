# HIPPO-USDT range strategy configuration

## Request and scope

User requests configuring the existing online HIPPO-USDT market strategy, following the discussed range preset, global/node volatility0.02, mean0.9, noise1.5, wick0.1. Inspect the actual current configuration first. The sample starting price and one-hour preview are examples, not authority to overwrite unrelated prices/time schedules blindly.

## Plan

1. Read the current strategy, price, time range and version through the specified admin UI.
2. Audit configuration/activation effects and preserve a non-sensitive before/after record.
3. Prepare a bounded range draft and inspect no-side-effect OHLCV preview.
4. Verify only explicitly requested changes. Do not replay or overwrite historical data.
5. Record actual outcome and any required final user action in PROGRESS.

## Constraints

No account/order/wallet operations, source-code edits, Git operations, dependency changes or historical repair. Never store credentials or tokens in task files. Existing uncommitted work remains untouched. If activation initiates consequential financial actions, hand off that action to the user rather than execute it.

## Acceptance

- [x] Exact HIPPO-USDT strategy and existing values verified.
- [x] Draft parameters and nodes match requested oscillation shape.
- [x] OHLCV preview inspected and limitations explained.
- [x] Final applied state or precise handoff recorded honestly.

## Current outcome

User confirmed proceeding with the default one-hour option. Saved and independently read back the2026-09-07 03:30–04:30 range configuration; status remains paused. Complete60-candle preview checked. Final activation remains user-operated because the price feed can execute pending orders and influence settlement. See research/saved-configuration.md; online-preflight.md records the earlier baseline, not the final state.

## 2026-09-07 postpone request

User asks to postpone because it is already04:29. Current local clock04:30. Plan: shift the single one-hour window to05:00–06:00 today UTC+08, with nodes05:15/05:30/05:45. Preserve every other saved parameter and keep the existing paused state. Cancel the stale enable dialog; re-read authoritative details, preview the newly timed version, save once, and read back exact fields. Final activation remains user-operated. No historical repair or implicit recurring schedule.

- [x] New window and all three timestamps previewed and saved.
- [x] Non-time fields and paused state preserved in authoritative readback.
- [x] Current handoff reflects the new05:00–06:00 interval.

Latest saved state:2026-09-07 05:00–06:00 UTC+08,paused,all three node times shifted. See research/postponed-configuration.md. Manual enable confirmation is prepared but not submitted.
