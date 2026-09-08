# Shared chart history runtime

## Ownership and flow

`MobileMarketChart.vue` owns an isolated `createMarketChartHistorySession` for its
mounted symbol/interval. Views keep their existing bounded current/live session;
`LightweightMarketChart.vue` remains render-only and emits demand. No view script,
backend, API adapter or existing market-detail layout test was changed by this
worker. Existing hidden wicks, raw OHLCV, attribution, price formats, MA/volume,
initial hydration and timestamp viewport work were preserved.

Flow:

1. A real pointer drag, touch move or wheel gesture arms the current logical range.
2. The renderer's subscribed logical range moves toward older rows (`from` decreases)
   and reaches `from <= 20`; emit `load-history` once for that armed gesture frame.
3. Shared session sends `fetchOlderKlines(symbol, interval, earliestTime * 1000)`.
   Cursor is exclusive Unix milliseconds. API owns inclusive-end translation and
   its 100-row limit. No artificial start window is supplied by this session.
4. Normalize and dedupe the earlier page, exclude timestamps at/after the captured
   cursor, and merge current rows last so late REST never overwrites a live row.
5. New points enter the existing renderer watcher. It captures the viewport at
   settlement, not demand time; timestamp anchoring preserves the newest gesture
   and a later gesture cancels queued viewport RAF work.

The session keeps only the current bounded input before the first history demand.
Once browsing starts, it retains loaded history and subsequent live rows until a
symbol/interval/reload/unmount boundary. A short valid page remains pageable;
empty, invalid-only or overlapping-only/nonprogress pages become exhausted.
There is one request in flight per active session. No automatic settlement fetch
occurs. Errors retain the canvas and require the explicit retry button, so a
continuing drag does not hammer a failing endpoint. Initial `loading=true`
suppresses pagination. Its false-to-true transition resets pagination ownership
for an explicit same-dataset reload. Key-only replacements clear old rows and wait
for replacement source points; generation checks reject A→B→A and unmount replies.

## Public hooks for verification

- `LightweightMarketChart` event: **`load-history`**, no payload.
- Renderer input props remain `points`, `movingAverages`, `symbol`, `interval`,
  `historyLoading`, `locale`, `label`. No API transport is inside the renderer.
- `MobileMarketChart` input props remain `points`, `symbol`, `loading`, `interval`.
- `.mobile-market-chart[data-history-state]`: `idle | loading | error | exhausted`.
- `.mobile-market-chart__history`: localized `role="status"`, `aria-live="polite"`,
  and `aria-busy=true` only during older-page loading. This is separate from
  parent/renderer initial history loading and never rearms hydration fitting.
- Error button: `type=button`, localized `marketDetail.retryOlderChart` aria-label,
  localized `common.retry` text, 44×44 minimum target, visible keyboard focus.
- Notice is pointer-transparent except for retry. Top-right placement reserves
  the left 64 pixels for Market Detail's expand toggle; attribution remains free.
- Exhausted notice: `marketDetail.noOlderChart` (“暂无更早的 K 线” /
  “No earlier candles available”), no retry button. This describes available
  stored history, not complete exchange history.
- Pure session exports `sync`, `requestOlder`, `retry`, `snapshot`, `dispose`.
  Transport is injected as `loadOlder(symbol, interval, before)` and optional
  `onChange(snapshot)` drives reactive consumers.

## Installed Lightweight Charts 5.2.0 ordering evidence

Inspected local `mobile/node_modules/lightweight-charts/dist/typings.d.ts` and
`lightweight-charts.development.mjs`, without internet/package changes:

- Typings lines 2990–3021 document `LogicalRange | null` notifications and the
  matching subscribe/unsubscribe callback contract.
- Runtime `getVisibleLogicalRange` around line 12890 calls
  `_internal_visibleLogicalRange` (6063), which calls `_private__updateVisibleRange`
  (6510). The getter synchronously recalculates from current right offset/bar
  spacing and fires the changed logical range; it does not require painted canvas.
- Native `_internal_scrollTo` (6329) synchronously updates the right offset and
  invalidates visible range. Wheel handler (11026) applies time zoom/scroll in the
  event handler. A capture-phase application callback precedes native handling.
- The gesture RAF therefore reads the actual post-handler model range even if
  the library has not delivered its next paint-time notification. A subscribed
  callback can emit sooner; either path consumes the one armed gesture.
- Fit, theme, resize, data writes, reload and key changes clear armed demand.
  Unmount clears the RAF, DOM capture listeners and logical-range subscription.
  Touch inertia without a new physical gesture does not create additional demand.

## Executable verification

Red runs:

- New `market-chart-history.test.ts` initially failed with missing session module.
- New `market-chart-history-renderer.test.ts` initially failed 6 cases: absent range
  subscription and absent gesture-to-`load-history` event bridge.

Green runs:

- 60/60 focused cases across `market-chart-history*.test.ts`,
  `mobile-market-chart-history.test.ts`, existing `market-chart-renderer.test.ts`,
  price-format/theme tests and `market-detail-reference-layout.test.ts`.
- Production `npm run type-check` and test `npm run type-check:tests` pass.
- Session tests execute delayed success/failure, exclusive sparse pages, repeated
  demand, live/REST conflicts, bounded pre-browsing retention, stale owners,
  source key-only switching, reload, failure retry and terminal empty pages.
- Renderer harness executes the actual SFC setup with Vue scheduling and real
  runtime helpers; only chart/DOM I/O is fake. It covers real event handlers,
  subscription cleanup, gesture baseline, notification-before/after range read,
  prepend anchors, settlement-time newer gestures and cancellation of queued work.
- Wrapper harness compiles the actual SFC script **and template** with the installed
  Vue compiler and renders through a tiny host renderer. Real Vue and vue-i18n
  execute the public renderer event→transport bridge, lifecycle, localized status,
  retry handlers and current/history prop wiring. Only HTTP/DOM/icons/child canvas
  I/O is fake; no pagination decisions are duplicated in the harness.

Full release gate, actual browser gestures/layout and final PROGRESS/spec updates
belong to the root task and browser worker. These deterministic unit results do
not claim online API or native Android/iOS verification.
