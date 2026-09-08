# History pagination and frameless-toggle browser verification

Date: 2026-09-07 (Asia/Hong_Kong)

## Isolation and implementation exercised

- Real Ego Chromium task space **4** and an owned Vite server at
  **http://127.0.0.1:13049**, rooted/cached under
  `/private/tmp/codex-chart-history-browser-20260907`.
- Mounted the actual `MobileMarketChart.vue` and `LightweightMarketChart.vue`,
  with installed Vue, vue-i18n, Lucide, and `lightweight-charts@5.2.0`.
- The component imported the real `fetchOlderKlines` and shared history session.
  Only the actual Axios client's adapter was replaced: it captured requests and
  returned a deterministic 400-row archive, with a one-day gap every 100 rows.
  Adapter promises were held until an explicit fixture settlement/failure.
  This exercised request creation, millisecond mapping, deduplication, session
  merging, native chart updates, and notices without production transport.
- Every captured request targeted the local `/api/v1/markets/FIXTUREUSDT/klines`
  path and had no Authorization header. The fixture rejected unexpected paths.
  No production API, auth endpoint, account, trade, database, or backend data was
  requested or changed. No dependency installation.
- A temporary Vite pre-transform appended a readback probe exposing the native
  chart range, timestamp-anchor capture, actual series data/options, and a range
  setter. It did not alter the production implementation or its decisions.
- The toggle used the real view's compiled scoped CSS, actual production
  light/dark market-detail token rules, and actual legacy dark shadow rule.
  The legacy rule was deliberately appended after the compiled scoped CSS to
  verify specificity rather than relying on load order. Fixture markup supplied
  matching scoped attributes, real icons, and inline/expanded button semantics.

The default sandbox first rejected Ego bootstrap access and loopback binding;
the same bounded commands succeeded through reviewed escalation. HTTPS was
blocked in the task tab while it was still `about:blank`, before fixture
navigation. The inherited profile nevertheless reported two cached remote
script bodies on the first navigation (zero transfer sizes). Cache disabling,
the same HTTPS block, and a local fixture CSP then produced three unrelated
HTTPS entries with **zero transfer and zero body sizes** on the final runs.
These inherited-profile entries were not fixture dependencies, and the built-in
TradingView attribution link was retained without being followed.

## Native history results

All page-demand checks below used real multi-step CDP mouse drags. The fixture
never called `requestOlder`, emitted `load-history`, or invoked a renderer input
handler directly. Native chart API setters established repeatable starting
windows; a real wheel event separately changed zoom before page two.

| Scenario | Observation | Result |
| --- | --- | --- |
| Baseline drag, 390px | Initial 100 rows; drag requested `end=1700277239999`, `limit=100`, no `start`; prepend moved logical range by +100 without changing timestamp or width 40 | Pass |
| Final page one with live updates | 120ms same-tail revisions and one advancing candle during actual drag; cursor `1700277299999`; range `[12.430379746835442,52.43037974683544]` -> `[112.43037974683546,152.43037974683546]`; 100 -> 200 rows | Pass |
| Page-one viewport | Anchor timestamp `1700280420`, width 40, fractional anchor offset approximately `0.43037974683545` before/after | Preserved |
| Real wheel zoom | Width 40 -> `44.05494505494506`; range `[25.29242594505495,69.34737100000001]` before next drag | Native zoom verified |
| Page two with 120ms same-tail revisions | Native drag requested cursor `1700184899999`; range `[8.182953139379599,52.23789819432466]` -> `[108.1829531393796,152.23789819432466]`; 200 -> 300 rows | Pass |
| Page-two viewport | Anchor timestamp `1700188020`, anchor offset `0.2378981943246572`, width `44.05494505494506` -> `44.05494505494505` | Preserved within floating-point precision |
| Subsequent live append | 300 -> 301 rows; oldest `1700092500` retained; visible historical range and timestamp anchor unchanged | History retained |
| Page-three failure, 320px | Cursor `1700092499999`; failure leaves 301 rows and `[10.5,50.5]`; status becomes `error` with actual Retry button | Pass |
| Explicit retry | Actual button click repeats exactly the same cursor and `limit=100`, no `start`; success yields 401 rows and `[110.5,150.5]`, same timestamp `1700095500` and width 40 | Pass |
| Partial final page | Cursor `1700000099999` returns 1 row; 401 -> 402 rows; status remains `idle`; logical range shifts only +1 | Does not infer exhaustion from short page |
| Empty final page | Cursor `1700000039999` returns 0 rows; remains 402 rows, status `exhausted`, visible text `No earlier candles available` | Pass |
| Further drag after exhaustion | Native range changes, but request count remains six and no promise is pending | No request loop |
| Deduplication | Native series readback: 402 timestamps, 402 unique (400 archive rows plus two live additions) | Pass |
| Interval reset | Switching to 5m resets retained history to 100 new rows, status `idle`, native fit `[0,99]` | Pass |
| Pending response ownership | Native 5m drag creates pending cursor `1700349239999`; key-only change to 15m clears chart to zero rows; settling stale 5m response leaves zero rows | Stale response ignored |
| New dataset arrives | 30 fresh 15m rows render and settle at native fit `[0,29]` | Pass |

There was one exploratory stress run before the final realistic-live matrix:
the fixture generated a **new minute bar every 250ms** during a slow roughly
nine-second mouse drag. Native data grew 200 -> 236, the final left logical
position moved from about 27.29 to 42.18, and the drag did not reach the history
edge or request page two. This artificial advancing-bar stress is **not a
passing case**, nor proof of a request-suppression defect: the native viewport
did not reach the trigger threshold. It was reported to the parent/runtime
worker. The final acceptance runs above instead used frequent revisions of the
forming candle, with a single new candle during a drag and a further live
append after pagination. No production change was made for the stress run.

Native chart `setVisibleLogicalRange` and fitting settle asynchronously. An
early 15m read briefly returned the preceding width with `[-39,1]`; a later
settled read confirmed `[0,29]` with all 30 rows. The final results above report
settled ranges rather than treating early renderer state as a completed fit.

## Toggle, notice, and visual results

Tested **12 combinations**: viewport width 320/390/448 × light/dark ×
inline/expanded. At every combination:

- Computed `border-width: 0px` and `box-shadow: none`, including focused state.
- Button rect exactly **44×44**; real 18px icon center delta **[0, 0]**.
- Inline x=16; expanded x=10. Expanded top=56 in this zero-safe-inset fixture
  (48px chart toolbar + original 8px offset).
- Light icon `rgb(17, 23, 20)`; dark icon `rgb(242, 247, 244)`.
- Backdrop remains `blur(14px) saturate(1.45)`.
- After keyboard modality and focus: actual `:focus-visible` matched, outline
  was **2px solid** with **3px offset**, and no shadow frame reappeared.
- `aria-pressed` followed the actual expand/collapse button clicks.
- Horizontal overflow, measured as `max(0, scrollWidth - innerWidth)`, was zero.

At 320px in the error state, toggle rect was `(16,200,44,44)`, notice rect
approximately `(84.91,192,216.09,44)`, and Retry rect `(251,192,44,44)`.
Toggle/notice intersection area was **0**. Actual Retry remained clickable and
triggered the held adapter request rather than expanding the chart.

Under emulated reduced motion, an actual held mouse press retained its active
theme fill (`color(srgb 0 0 0 / 0.94)` in dark mode), while border stayed zero,
shadow stayed none, and transform/transition were both none.

Inspected real screenshots of initial/zoomed candles, the 320px failure notice,
and dark expanded focus state. Candle bodies remained without upper/lower
stems; volume, three moving-average series, current-price line, axes, and
attribution remained. Native options confirmed `wickVisible=false` and
`priceLineVisible=true`; data retained genuine high/low beyond candle bodies.

## Limitations

- Deterministic component/CSS fixture, not complete MarketDetail/Trade route
  E2E or verification of real REST/WebSocket/backend storage.
- MarketDetail script handlers, full-page scroll lock/focus trap, account and
  route integration were not copied or exercised; this slice verified the
  actual relevant CSS and the actual shared chart/data-history implementation.
- Desktop Chromium at emulated viewport widths, not physical touch devices,
  touch pinch/inertia, native Android/iOS/Tauri, safe-area hardware, or pixel-diff
  and performance benchmarking.
- Focus was measured after a real Tab established keyboard modality followed
  by programmatic focus of the actual button; it was not a full-page Tab-order
  audit.
- No dedicated post-dispose transport check or symbol-change browser matrix;
  interval ownership and stale reply checks were exercised as above. Those
  broader lifecycle cases remain covered by the task's executable tests.

## Cleanup

- `completeTaskSpace(4, { keep: false })` returned **`{ done: true }`**.
- Stopped the owned server through its execution session (exit 130).
- Owned fixture/cache/screenshots were deleted, and port 13049 was checked for
  no remaining listener.
- No tracked production/test/spec/PROGRESS or Git operations in this browser
  slice; this research report is its only retained file.
