# Mobile candle-wick visibility (2026-09-07)

## Confirmed scope and implementation

User explicitly selected candle wicks (影线), not MA or current-price lines.
The production change is exactly `wickVisible: false` on the shared mobile
CandlestickSeries. Market Detail and Trade chart consumers inherit the option.
Raw OHLCV, candle bodies, MA5/10/20, volume, current-price line/labels, autoscale,
theme/locale updates, dataset identity, incremental updates and gestures remain
unchanged. No new dependency, backend data modification or migration.

## Verification

- Added a scoped CandlestickSeries option regression before production edit;
  it failed because the original options did not hide wicks.
- Price-format/theme/reference-layout focused run: 18/18 passed after the edit.
- Full `npm run release:gate`: production/test type checks, 686/686 tests, PWA
  and Tauri web builds, artifact checks, bundle budgets, source-size budgets
  and critical-test quality checks all passed.
- Independent read-only review found no concrete issue; extra new-test-only
  run 1/1 passed. No full gates repeated by the reviewer.
- `git diff --check` passed. No live browser screenshot/online deployment or
  native Android/iOS binary build was performed; native startup/dependencies
  are unchanged. Existing Admin/Rust verification remains the prior slice's
  evidence, not a claim of rerunning those suites for this one-line UI change.

## Delivery

Updated the mobile shell spec to prevent future code from clamping raw high/low
or re-enabling wicks during theme/dataset refreshes. Work remains uncommitted;
no production API/session/financial/configuration changes were made.
