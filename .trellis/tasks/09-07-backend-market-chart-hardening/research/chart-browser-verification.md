# Mobile chart real-browser verification

Date: 2026-09-07 (Asia/Hong_Kong)

## Fixture and isolation

- Real Ego Chromium task space **2**, created for this verification only.
- Temporary Vite server on **127.0.0.1:13048**, with an isolated root/cache under
  `/private/tmp/codex-chart-browser-20260907`.
- Mounted the actual shared `MobileMarketChart.vue` and
  `LightweightMarketChart.vue`, including real Vue, vue-i18n and installed
  `lightweight-charts@5.2.0`. No component implementation was copied.
- Deterministic two-decimal OHLCV, 300 minute candles, explicit controls for
  1→2→3 live rows, history settlement, bounded rolling replacement, batch append
  and same-candle revision. Every candle retains high/low beyond its body.
- A temporary Vite pre-transform appended a probe exposing the real chart API's
  visible range, series data/options and renderer hydration-intent state. The
  probe also set initial ranges for repeatable viewport scenarios. It did not
  alter production rendering decisions or persist any production-file changes.
- Used only local fixture controls/data; no production API, authentication,
  backend, database, account or trade action. No package installation.

The inherited browser profile automatically injected three unrelated HTTPS
script resource entries during the first navigation. All HTTPS URLs were then
blocked with `Network.setBlockedURLs` in this task tab before the final runs;
final entries had zero transfer/body sizes. The fixture itself referenced only
loopback resources. The built-in attribution link remained present and was never
followed. This qualification avoids claiming the inherited browser profile had
no independent network activity during initial setup.

## Actual results

| Scenario | Native chart observations | Result |
| --- | --- | --- |
| Initial live rows, 390px | One row `[−1, 0]`; three rows `[1, 2]`; history remained pending and viewport intent false | Live data visible during REST wait |
| Initial history, 390px | After settlement: 300 rows, logical range `[0, 299]`, history pending false | Full initial history fitted |
| Visible live-tail rolling retention | Before `[239.5, 309.5]`, 300 rows; after dropping first/adding newest: same range, 300 rows, newest timestamp +60s | Width 70 and right padding 10.5 retained |
| Batched append at live tail | After adding two rows: `[241.5, 311.5]`, 302 rows | Follows +2 while retaining width/padding |
| Same-candle edit | Remained `[241.5, 311.5]` | No refit/pan |
| Historical rolling retention | Before `[50.5, 110.5]`; after dropping first/adding newest: `[49.5, 109.5]` | Timestamp-anchored window, not forced to tail |
| Real wheel in historical view | `[49.5, 109.5]` → `[46.2218592222, 112.999637]` | Actual native zoom changed |
| Real CDP mouse drag | `[46.2218592222, 112.999637]` → `[28.5021787574, 95.2799565352]` | Actual native pan changed |
| Real wheel before initial history | Three live rows → `[0.8925197778, 2.114742]`, viewport intent true | Gesture recognized while history still pending |
| History after that real wheel | `[297.8925197778, 299.114742]`, 300 rows; width delta ≈ `6.2e-15`, right timestamp-anchor index delta exactly +297 | Deliberate initial zoom preserved; no hydration fit |
| Initial history, 320px | Settled `[0, 299]`, 300 rows; repeated stable read matched | Pass |
| Initial history, 448px | Settled `[0, 299]`, 300 rows | Pass |
| Rolling live tail at 320/448px | Both retained `[239.5, 309.5]` and 300 rows | Pass |
| Horizontal overflow | `scrollWidth − innerWidth = 0` at 320, 390 and 448px | Pass |

Native `setVisibleLogicalRange`/`fitContent` application is asynchronous. Early
reads during rapid fixture actions sometimes returned the pre-fit range; final
observations above waited for settled native rendering. The 320px hydration was
repeated with spaced actions and two stable reads (`[0, 299]` both times). No
stable hydration or viewport defect remained in this matrix.

## Visual/data invariants

Inspected real screenshots at 390px for full-history density and a 35-logical-bar
zoomed window. Candle bodies had no upper/lower stems; all three colored MA curves,
volume bars, current-price dotted line, price axis and attribution remained.

Probe readback confirmed:

- `wickVisible === false`, including after history/rolling/revision;
- `priceLineVisible === true`;
- first history OHLC remained `100 / 102.7 / 98 / 100.7`;
- a later rolling candle remained
  `103.76 / 106.36 / 101.76 / 104.36`, rather than clamped high/low;
- 300 candles and volume entries, and 296 MA5 entries for the 300-row dataset;
- the chart used the same native canvas/series implementation at all widths.

## Limitations

- This is a deterministic component fixture, not full MarketDetail/Trade route
  E2E, and does not validate real REST/WebSocket transport, backend authority,
  authentication, or order/account operations.
- Desktop Chromium at emulated mobile viewport widths; no physical device,
  native Tauri/Android/iOS shell, touch pinch, or kinetic-touch-scroll matrix.
- Light-theme fixture tokens only; no full light/dark visual comparison.
- Repeatable tail/history initial ranges used the real chart API probe. Real
  wheel and multi-step CDP mouse-drag events separately verified actual native
  interaction; the delayed-history preservation scenario used a real wheel.
- Screenshots were visually inspected, not a pixel-diff or performance benchmark.
  Existing fit-all density and resize policy were not redesigned.

## Cleanup

- `completeTaskSpace(2, { keep: false })` returned **`{ done: true }`**.
- Stopped the owned Vite process; `lsof -nP -iTCP:13048 -sTCP:LISTEN` returned no
  listener.
- Removed the entire owned temporary fixture/cache directory and generated
  screenshot; confirmed the fixture directory no longer exists.
- The first cleanup command's automatic approval review timed out before
  execution. One bounded retry deleting the same owned files completed.
- No tracked production, test, spec or PROGRESS files were edited in this browser
  verification slice; this report is the only retained artifact.
