# Mobile chart runtime implementation

Date: 2026-09-07 (Asia/Hong_Kong)

## Scope and coordinated ownership

Implemented only the two chart components, the existing runtime helper, a new
renderer regression file and its source-extracted SFC harness, and two narrow
renderer wiring assertions in the existing reference-layout test. The existing
`wickVisible: false` and its previously added regression were retained. No Admin,
Rust, API, stream, view, dependency, spec or PROGRESS changes were made by this
worker. Root owns upstream loader changes, release gates and shared records.

Root confirmed the cross-component loading contract before implementation:

1. `chartLoading` remains true until the current initial/interval REST K-line
   request settles, independently of depth/trades. Live callbacks do not clear it.
2. `MobileMarketChart.loading` passes through as renderer `historyLoading`.
   Existing visible candles remain visible while history loads.
3. Initial history settlement is consumed once per normalized symbol/interval;
   an ordinary same-dataset loading cycle does not rearm history fitting.

## Runtime decisions

- Capture the existing timestamp/right-edge anchor and optional visible-tail
  timestamp/right padding. Tail visibility uses the installed chart's strict
  logical-range rounding, including partial edge candles.
- `resolveMarketChartLogicalRange` remains timestamp-only by default. Data
  replacements explicitly enable advancing-tail following; it applies only when
  the captured last candle was visible and the next newest timestamp is later.
  Preserve width and signed right padding. Browsing history, prepend, same-tail
  corrections and older-tail replacements retain timestamp anchoring.
- Retain the intended pending viewport between coalesced replacements rather
  than recapturing transient raw chart indexes before the RAF restoration. A
  pending theme restoration is rebased when an incremental append arrives.
- Replace the `previous.length <= 1` hydration heuristic with explicit
  initial-history settlement. First visible live points still use the existing
  `fitContent` policy; initial history may fit once on settlement. A real key-only
  change waits for replacement points before fitting/rendering stale data.
- Capture passive wheel, pressed pointer move, touch move and double-click
  intent. User intent cancels pending restoration and prevents hydration refit;
  pointer hover does not count. New symbol/interval datasets reset this intent.
  Listener cleanup captures the original element because Vue clears template
  refs before `onUnmounted`.
- Keep raw OHLC, hidden wicks, candle price-line defaults, computed real MAs,
  volume, shared precision, attribution, gestures, theme/locale updates,
  initial all-history density and current resize policy unchanged.

## Executable red → green evidence

New tests execute the actual `LightweightMarketChart.vue` setup script, extracted
with the TypeScript AST and transpiled after removing imports. Vue reactivity,
watcher batching and production price-format/viewport/MA helpers are real. Only
DOM/chart/observer/RAF I/O is faked; renderer decisions are not duplicated.

First run against the original renderer: **11 passed / 9 failed** out of 20.
Failures covered rolling retention, batch append, prior-candle revision plus
append, live viewport edge, 1→2→3 live-candle hydration, delayed loading startup,
new-key hydration, consecutive pre-RAF replacements and gesture cancellation.
Hydration was then tightened to chronological growing live bars, not prepended
bars. Additional regressions brought the new suite to **23/23**:

- the theme-restoration-plus-append regression first failed and then passed;
- the partial-edge candle/right-padding case first failed and then passed;
- historical consecutive replacements and direct helper opt-in were added.

Final executed focused command from repository root:

```sh
node --test --experimental-strip-types \
  mobile/tests/market-chart-renderer.test.ts \
  mobile/tests/market-detail-reference-layout.test.ts \
  mobile/tests/market-chart-price-format.test.ts \
  mobile/tests/market-chart-theme.test.ts \
  mobile/tests/ui-prototype-alignment-trading.test.ts
```

Result: **49/49 passed**, including the existing hidden-wick and original OHLC
tests. A path-scoped `git diff --check` also passed. The existing reference-layout
test required only the new loading passthrough assertion and updating the RAF
restore-call assertion to include its explicit follow-mode argument.

## Limitations / root handoff

- No browser pixel geometry, native gesture propagation, native chart-internal
  RAF ordering, network, build, package typecheck or full gate was executed by
  this worker. The harness verifies application control flow and chart calls;
  its minimal native-append simulation does not claim to reproduce rendering.
- Intent detection conservatively treats touch movement/double-click within the
  chart as an explicit view choice; it does not intercept or prevent those
  events. Hover and plain pointer-down/taps do not consume history fitting.
- Loading settlement means request success or failure; a failed request with
  live rows fits those available rows only if the user has not interacted. Empty
  settlement retains the initial nonempty-fit opportunity. Same-dataset reloads
  that explicitly clear all data retain the prior empty-to-nonempty behavior.
- This helper does not recover timestamps already trimmed out of bounded data;
  the existing nearest-timestamp fallback remains intentional.
- Root must validate its authoritative history-loading lifetime and the complete
  Mobile `release:gate` before declaring end-to-end completion.

## Read-only upstream integration review

After root's loader implementation, inspected `marketDetailSnapshot.ts`, both
view loader/interval callbacks and `market-snapshot-loading.test.ts` without
editing root-owned files. The helper starts channels together and commits each
settlement separately. K-lines require both symbol/load ownership and the live
session/request token; depth/trades deliberately retain symbol/load ownership
across interval replacement. A persistent per-load live-depth flag protects a
previous interval's authoritative book, including a valid empty book.

An in-memory execution reused the source-extracted root loader harness (no new
test file) and passed **6/6 additional scenarios** across the two views:

- overlapping BTC → ETH → BTC full loads reject both older load generations;
- an empty live book remains authoritative across interval replacement and late
  nonempty REST, while a current interval K-line failure clears history loading;
- 1m → 5m → 1m requests accept only the newest interval generation but retain the
  pending initial symbol-owned depth/trade snapshots.

Also reproduced the Trade template's older depth/trade loading coupling: REST
depth can finish while trades remain pending, making `depthLoading` false with
no trades and prematurely selecting `noRecentTrades`. Root accepted this finding
and owns independent depth/trade loading flags and regressions in both views;
the runtime/historyLoading contract itself remains correct.

After root added independent loading flags, jointly executed the updated loader
and renderer suites with `node --test --experimental-strip-types
mobile/tests/market-snapshot-loading.test.ts mobile/tests/market-chart-renderer.test.ts`:
**42/42 passed** (19 upstream-loader cases plus 23 renderer cases).
