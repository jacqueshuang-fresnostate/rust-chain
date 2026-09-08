# Mobile chart renderer/runtime audit

Date: 2026-09-07 (Asia/Hong_Kong)
Scope: read-only production/test inspection; this research document is the only
file written by this auditor. Existing Admin/Rust/Mobile changes are preserved.
No browser, production request, build, full gate, dependency or code edit was
performed. Root independently owns upstream stream/loading/error analysis.

## Executive decision

1. **Confirmed P2: replacements that advance the live tail stop following it.**
   Repair the viewport decision for rolling history, batched append and
   last-candle-revision-plus-append, while preserving timestamp anchors for users
   browsing old candles and for pure history prepend/correction.
2. **Confirmed component boundary, lower priority: initial history hydration is
   inferred from candle count and fails after several early live bars.** The
   renderer has no authoritative history-ready signal. Include only together
   with the root's upstream loading review; do not replace it with another
   arbitrary candle-count heuristic.
3. Initial all-history fitting, resize time-span behavior, right padding and
   candle/volume overlap are **current presentation policies**, not proven data
   defects. They are eligible explicit UX improvements, not silent bug fixes.
4. No evidence justifies speculative caching, a new chart engine, OHLC rewriting,
   dropping indicators, resubscribing streams, or removing official attribution.

## Inspected contracts and source

- `/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/.trellis/spec/mobile/backend-integration.md:225-263`:
  normalized real candles, indicator authority, incremental update, initial /
  dataset fitting, timestamp-anchored history replacement, shared display format.
- `/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/.trellis/spec/mobile/pwa-and-shell.md:225-254`:
  local single renderer, theme/locale preservation, hidden wicks with untouched
  OHLC, body/MA/volume/current-price/scale invariants.
- `/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/mobile/src/components/LightweightMarketChart.vue`:
  imperative rendering, options and lifecycle; references below use current lines.
- `/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/mobile/src/components/MobileMarketChart.vue:10-44`:
  parent-owned points/loading, normalized points and computed real MAs; child
  receives points/MA/key/locale but no history-readiness or initial-fit signal.
- `/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/mobile/src/core/marketChartRuntime.ts`:
  update classification and timestamp viewport helpers.
- `/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/mobile/src/core/marketChart.ts`:
  finite/positive OHLCV normalization, timestamp dedupe/sort and display precision.
- `/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/mobile/src/core/marketChartTheme.ts`:
  computed token reading and theme-observer cleanup.
- `/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/mobile/src/core/marketIndicators.ts`:
  real-close rolling SMA5/10/20.
- Installed primary implementation inspected locally:
  `/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/mobile/node_modules/lightweight-charts/dist/lightweight-charts.development.mjs`.
  Version is pinned to 5.2.0; no external documentation assumption was needed.

## Finding A: advancing replacements lose live-tail following

### Source evidence

`marketChartRuntime.ts:16-44` only calls a change `append` if precisely one row
is added and every previous row is identical. These legitimate updates instead
become `replace`:

- bounded history shifts left by one and receives the next candle;
- two or more new candles arrive in the same committed frame;
- the previous forming candle receives its final close while the next candle is
  appended in that same frame.

`LightweightMarketChart.vue:303-313` captures the old viewport for these cases;
`renderAllData:125-148` sets all series and then restores that old timestamp
anchor; `marketChartRuntime.ts:75-93` resolves only the old timestamp and width.
There is no distinction between browsing history and watching the live tail.

Ordinary single append uses `updateLatestData:151-160` without restoring the old
anchor. The installed library has `shiftVisibleRangeOnNewBar: true` by default
(line 12359) and checks whether the prior last bar is visible before following
new right-hand data (lines 7088-7113). Thus single append and economically
identical batched/retained-history updates have different user behavior.

### Executed reproduction (real production pure functions)

Construct 300 valid candles with consecutive minute timestamps, all with original
OHLC `10/12/9/11`, volume `1`, and an old visible range `[239.5, 299.5]`.
Capture the production viewport and resolve it against each replacement:

| Update | Classification | Restored range | New last index |
| --- | --- | --- | --- |
| Drop first + append one | replace | `[238.5, 298.5]` | 299 |
| Append two | replace | `[239.5, 299.5]` | 301 |
| Revise previous last close + append | replace | `[239.5, 299.5]` | 300 |

All three assertions reproduced: the new last candle's center lies to the right
of the restored range. With continued replacements the user remains anchored to
an old timestamp instead of tracking the market. This is a pure viewport issue;
all data are present and no backend repair is appropriate.

Reproduction entry points, executable via Node strip-types:

```ts
const viewport = captureMarketChartLogicalViewport(previous, {
  from: 239.5, to: 299.5,
})!
assert.equal(classifyMarketChartDataUpdate(previous, next), 'replace')
const actual = resolveMarketChartLogicalRange(next, viewport)!
assert.ok(actual.to < next.length - 1)
```

### Narrow repair

Capture both the timestamp anchor and whether the prior last candle is currently
visible, including its right-edge offset. On same-dataset replacement:

- If the user was at the live tail **and the newest timestamp advances**, preserve
  logical width/right padding relative to the new tail.
- If the user was browsing earlier data, preserve the existing timestamp anchor.
- For pure prepend, same-tail historical correction, theme or locale work, retain
  existing timestamp-anchor semantics; do not newly scroll to the latest candle.
- Keep symbol/interval fitting isolated, including key-only changes that still
  await a replacement points array. Preserve pending RAF cancellation/cleanup.
- Do not optimize rolling-window updates by leaving trimmed old rows indefinitely
  in the renderer; retaining bounded memory is independent of tail-follow choice.

Tests should execute this decision with real helper inputs and mocked chart calls,
not only source regexes. Cover all three reproductions, panned-back views, positive
right padding, pure prepend, same-tail correction, same-candle update, key-only
change followed by points, and unmount/pending-frame cleanup. Existing source
contracts may remain as supplemental wiring checks.

## Finding B: first REST/history arrival can inherit a two-candle viewport

### Source and executed component-boundary evidence

`LightweightMarketChart.vue:139-145` consumes initial fitting on the first nonempty
points. `:299-302` re-arms fitting only when the prior data length is at most one.
No other history-ready boundary is passed by `MobileMarketChart.vue:37-43`.

A local, in-memory Node harness extracted the actual SFC script, removed imports
using the TypeScript AST, transpiled it, and executed it with real production
runtime/price-format/MA functions and fake chart/Vue lifecycle/RAF objects. It
replaced points with one live candle, then two, then three, then the full 300-row
history. Actual script decisions produced only two `fitContent` calls and kept a
logical width of two after history hydration. All 300 actual rows, original high
and low, and `wickVisible:false` remained intact. The fake chart verifies control
flow, not browser pixel geometry; exact indices after native append are not
claimed by this harness.

This is an asynchronous boundary hole, not proof that every initial load is
broken. A normal immediate 300-row response is fitted correctly by current policy.
Root must assess upstream reachability/readiness; live frames can precede REST,
so a renderer-only arbitrary length threshold is not a coherent repair.

### Narrow repair boundary

If selected, establish an explicit first-history-settlement/initial-view intent
from the existing loader and let the renderer consume it once per dataset. Respect
user pan/zoom performed before that settlement; never refit every refresh or
same-candle update. Test delayed history after one/two/three live bars, initial
empty/failure, dataset replacement, and a user interaction before settlement.
Avoid overloading generic loading state without distinguishing initial loading
from background refresh. This requires coordination with root's upstream audit.

## Deliberate policies and optional UX changes

### Initial 300-row fit

`LightweightMarketChart.vue:143-145` calls `fitContent`; installed library
`:6404-6415` explicitly fits first through last. With a 350 CSS-pixel container,
300 bars occupy **at most** about 1.17 pixels each before subtracting the price
axis. Library default `barSpacing:6` is replaced by fitting and does not rescue
readability (`:12343-12350`, `:6390-6400`). This explains a crowded initial chart
without requiring broken data or hidden indicators.

The current Mobile spec explicitly allows this initial fit policy. Showing a
bounded latest window (for example a chosen count/density with trailing space)
would be a reasonable requested chart optimization, but its exact count and
right-padding values are presentation choices. Keep the full dataset available
for panning, make it initial/new-dataset-only, and update the spec/tests if root
selects it. Never truncate authoritative high/low or remove points just to zoom.

### Resize / CSS fullscreen / orientation

`resize:201-206` correctly skips zero width/height and resizes the existing chart.
Installed library defaults `lockVisibleTimeRangeOnResize:false` (`:12352`): width
changes keep bar spacing and therefore change the visible time span. Setting it
true rescales spacing to preserve the time span (`:6097-6128`). The current specs
require preserving theme/locale/history viewport state, but do not establish
which resize policy wins. This is an explicit UX choice; no proven lifecycle
leak or forced refit was found. Dimension-only work must not remount or refetch.

### Autoscale / hidden wicks / volume overlap

Raw high/low still flow through `candleRow:94-101`; retaining their autoscale effect
is explicitly required. A small candle body below large real high/low is not
permission to clamp those values. Volume occupies the bottom 24% of its independent
price scale (`:249-254`, `:276`) while candles use the default scale margins; this
can visually overlap bodies, but it is an overlay-layout choice, not wrong OHLCV.
No different scale policy should be smuggled into the tail-follow repair.

### Performance / theme / indicators

Normalization clones, deduplicates and sorts bounded history; classification,
price-format inference and rolling MAs scan it. No measured hot-path bottleneck
was established at 160/300 rows, so memoization, complex diffs or caches are not
justified here. The existing three fixed SMA windows are bounded and use real
closes. Theme observation correctly follows both stage and root transitions and
has tested disconnect behavior. A theme switch intentionally refreshes volume
row colors; no evidence supports removing that work or observer coverage.

## Current testing gaps and actual validation

Executed the following focused baseline, **26/26 passed**:

```sh
node --test --experimental-strip-types \
  mobile/tests/market-chart-price-format.test.ts \
  mobile/tests/market-chart-theme.test.ts \
  mobile/tests/market-detail-reference-layout.test.ts \
  mobile/tests/ui-prototype-alignment-trading.test.ts
```

Also executed the three real-core tail-loss reproductions and the extracted-SFC
hydration control-flow reproduction above. These were inline scripts and created
no files. They demonstrate missing scenarios despite the baseline being green.

Current runtime regression in
`/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/mobile/tests/market-detail-reference-layout.test.ts:275-321`
checks update classification plus a single pure prepend anchor. Most renderer
wiring/lifecycle checks in `:195-250` are source assertions. No current test
executes tail-follow across rolling/batched replacement or delayed initial
history beyond two live candles. Retain wick, original OHLC, precision, single
renderer, gestures, theme and cleanup assertions when filling this gap.

## Handoff

Recommended first Mobile slice: Finding A with pure decision + renderer-call
regressions. Root may add a separately stated initial readable-window choice and
Finding B only after resolving the upstream first-history intent. Root owns task
PRD, shared PROGRESS updates, implementation, full package gates and any eventual
browser verification; this audit does not claim those steps completed.
