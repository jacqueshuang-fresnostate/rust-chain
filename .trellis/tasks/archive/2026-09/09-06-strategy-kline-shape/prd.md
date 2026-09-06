# Strategy K-line shape and low-price diagnostics

## Goal

Investigate and fix the unusual chart reported after configuring a market strategy. Trace configuration units through deterministic OHLC generation, persisted/live candles, and the Mobile chart before choosing the smallest correction.

## Evidence

- User screenshot: selected 1m, bodies near 0.10, long wicks extending toward 0.60, volume overlay, latest-price marker around 0.20. The screenshot alone does not identify the pair or exact settings.
- Working tree starts clean at `902de5e`; previous CI timeout fix is independent.
- Backend generation uses relative volatility directly, while node tolerance uses percent divided by 100. Admin currently labels volatility without its unit.
- Historical strategy versions must remain deterministically replayable. Do not silently change old snapshot semantics or rewrite generated history to make a chart look different.

## Configuration confirmation

The user supplied the production admin address and authorized local Mobile testing against that backend. Normal authenticated read-only detail confirms HIPPO-USDT / version 1 / volatility 6 / wick 0.75 / no nodes / flat 0.1 start and target. Thus the current shape follows a 600% ratio and up to 450% wick factor. Asked whether the intended value was 6%; local source work does not change the running strategy or old history.

## Plan

1. Inspect strategy input/preview/snapshot generation and Mobile chart formatting; reproduce the suspected issue with exact low-price fixtures.
2. Correct confirmed configuration/renderer defects at their owning boundary, keeping decimal precision, legacy replay, and REST/WS behavior intact.
3. Run focused regressions plus affected package gates; update the progress record and relevant contracts.

## Confirmed implementation boundary

- Keep API/draft volatility values as their existing decimal ratios. Label both global and node fields explicitly, display their percent interpretation, and surface combined volatility/wick warnings without silently changing valid historical values.
- Replace the close-only preview with an OHLC candlestick preview using the full high/low range; retain exact raw values, pure preview semantics, and a clear sampling notice.
- Correct Mobile candle/MA display precision from observed OHLC values, including scientific notation and small prices, without adding a data request, changing OHLC, or resetting pan/zoom on live updates.
- Add low-price deterministic generator regressions; production generation/snapshots/storage stay unchanged. Existing abnormal history is not a chart-formatting problem and remains untouched.

## Research reference

See `research/findings.md` for authenticated settings, public read-only samples and the unit/preview/renderer contracts. Any operational correction is separate from the confirmed local UI fixes above.

## Acceptance

- [x] Explain the observed shape with authenticated configuration and public OHLC evidence.
- [x] Tests cover units/precision/shape and low-price inputs; final Web 514 and Mobile 685 pass.
- [x] Pure generator remains byte-unchanged, its replay/low-price tests pass; original history and financial behavior stay unchanged. Final production GET remains active / V1 / ratio 6.
- [x] Validate real local Mobile intervals, stable viewport and eight display variants; Admin renders actual production preview responses for ratio 6 and unsaved 0.06. See findings for intermittent network errors and exact verification scope.

## Delivery status

Local implementation and affected validation are complete. The user confirmed
commit and push on 2026-09-06. Delivery follows the approved work/archive/journal
commit chain to origin/main; remote SHA is verified after pushing. No deployment,
production configuration adjustment or historical data write is included.
Await the user's intended volatility before any separate operational plan.

## Out of scope

No live strategy edits, funds/orders/settlement changes, automatic historical cleanup/recovery, or unrelated interface redesign. No dependency or algorithm replacement merely to cosmetically hide real OHLC values.
