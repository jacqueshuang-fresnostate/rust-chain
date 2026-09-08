# Mobile chart history and frameless toggle review

## Delivered behavior

- The shared chart listens for a user gesture that moves toward the oldest loaded
  edge, then requests one earlier page. The existing public endpoint is sufficient:
  exclusive cursor becomes inclusive `end=beforeMs-1`, no `start`, limit <=100.
- Latest requests also omit the synthetic lookback window, so real stored candles
  separated by gaps remain reachable. Smaller requested sparkline limits remain.
- Shared history state retains explicitly loaded rows across subsequent bounded
  live input. Same-slot current data wins; prepending preserves timestamp/zoom
  and a gesture after request completion overrides queued renderer work.
- Single-flight, dataset/load generations and disposal reject old replies. Errors
  keep the chart and show explicit retry. Empty/nonprogress pages show a localized
  no-earlier-data notice; no automatic page-draining or failure retry loop.
- Chart-toggle border and all inset/outer shadows are removed across regular,
  pressed, expanded and dark states. The 44px control, theme contrast, keyboard
  focus outline and reduced-motion-aware pressed feedback remain.

No backend changes, provider requests, historical writes or fabricated OHLCV.
Default 1m, hidden wicks, current-price line, volume/MA, attribution and initial
fit/resize behavior remain. TradeView and both view scripts were not changed.

## Verification

| Check | Result |
|---|---|
| API sparse window and request cap regressions | Both failed before the fix |
| API executable adapter/query tests | 6/6; 276 unique sparse rows across 4 requests |
| API + protocol + no-cache whitelist | 19/19 |
| History session + renderer intent + actual compiled wrapper | 19 new behavior tests |
| Combined focused chart cases incl prior viewport/price/theme/layout | 60/60 |
| Toggle compiled CSS regression | Test-first fail, final layout suite 12/12 |
| Full Mobile release gate | 755/755; both type checks, PWA/Tauri web builds, artifact/bundle/source/test-quality budgets pass |
| Static cross-layer review | No concrete defect; see history-api-audit.md |
| Real browser | Native multi-page drag, live/viewport retention, retry/partial/empty pages and stale interval reply pass; toggle 12 width/theme/mode combinations pass |
| Trellis context and diff whitespace | 4+4 and git diff --check pass |

The first full suite found two obsolete source-pattern checks requiring direct
`normalizeMarketChartPoints(props.points)` in the wrapper. They now check the
shared history-session wiring and its normalized output. Actual normalization,
merge and lifecycle behavior is covered by the executable tests; no gate, timeout
or budget was lowered. The final full gate passed after these supplemental-only
test updates. TradeView stays 6131/6131 lines, 179598/179654 bytes, unchanged from
this task's baseline.

## Limits and handoff

- Pagination reads **existing stored history**. It cannot recover candles that
  never reached the backend; earlier ingestion gaps remain a separate task.
- Pages are 100 maximum because that is the existing server cap, not the 160-row
  live retention default. A short normalized page is not alone an end signal.
- Before browsing, input retains its existing bounded live window. Once browsing
  starts, the active chart retains requested history and subsequent live rows
  until reload/dataset/unmount; no cross-route persistent history cache is added.
- No new real-backend route/DB execution was required (backend source unchanged).
  Native Android/iOS binaries and deployed API acceptance were not run.
- All prior uncommitted work is retained. Shared Mobile specs/components/tests
  overlap preceding tasks, so earlier commit plans need reviewed hunks or a revised
  combined manifest. No commit/push/deploy or task/journal auto-commit performed.

Real-browser results use the actual chart components and API adapter with local
deterministic transport, not the full route or production backend. Native drag
and wheel are exercised; physical touch/pinch is not. The toggle uses real
compiled scoped CSS and theme rules. A separate artificial advancing-bar stress
run did not reach the history edge and is not counted as a passing case. See
`research/history-browser-verification.md` for exact ranges and limitations.

Owned browser task space 4 completed, server stopped, port 13049 has no listener,
and temporary fixture/cache/screenshots were removed. Only its research report
is retained. Implementation and verification are ready for review/handoff; task
status remains in_progress until the user-directed commit/archive workflow.
