# Historical K-line API audit

## Conclusion and exact request

The existing API supports backward keyset pagination over **available stored
history** without backend changes. For earliest loaded `KlinePoint.time = T`
(Unix milliseconds), send:

```text
GET /markets/{normalizedSymbol}/klines?interval=1m&end={T - 1}&limit=100
```

Use the selected interval, omit `start` entirely, and compute the next cursor
from the minimum accepted returned `open_time`, not from the previous cursor
minus an interval/window. The initial snapshot should likewise omit `start`
and request `end=Date.now()` with an effective page size at most 100.

For any requested N in 1..100 the query returns the newest N rows strictly
before T, sorted ascending. More than 100 rows requires repeated requests;
the backend silently clamps 160 or 500 to 100. Do not mistake 100 returned
rows for exhaustion because the old Mobile default limit is 160.

Omitting `start` allows the Mongo query to cross arbitrarily large gaps. A
calculated window `start = end - intervalDuration * limit` can return no rows
even while older history exists. Subtract **one millisecond**, not one candle
interval: alignment is the writer's responsibility and the domain key does
not guarantee aligned open times.

## Source evidence

| Concern | Contract and source |
| --- | --- |
| Route | `src/modules/market/routes.rs:124-140`: Axum query DTO; response is a bare array, not an envelope with `has_more` or cursor. |
| Query parsing | `src/modules/market/presentation.rs:71-79`: required `interval`, optional `start/end` via `option_unix_millis`, optional unsigned `limit`. |
| Timestamp units | `src/time.rs:49-60`: timestamp deserialization uses `i64` and `DateTime::from_timestamp_millis`. `src/modules/market/presentation.rs:81-92`: response `open_time` serializes with `unix_millis`. |
| Interval/limit | `src/modules/market/domain.rs:95-108,138-156`: exact interval whitelist `1m/5m/15m/1h/4h/1d`; default and maximum limit 100; clamp 0 to 1; preserve optional times. No start-before-end validation and no alignment guarantee. |
| Read orchestration | `src/modules/market/application.rs:145-172`: validate symbol/listing, query Mongo, optionally merge synthetic higher-interval forming cache. Mongo is required; query/storage errors are not empty pages. |
| Exact bounds | `src/modules/market/infrastructure/persistence.rs:304-315`: `$gte` start and `$lte` end, BSON millisecond conversion. Both bounds inclusive; absent bounds omitted; inverted interval naturally matches nothing. |
| Limit/order | `src/modules/market/infrastructure/persistence.rs:278-301`: filter by interval and optional time; `open_time: -1`, then database limit, then reverse to ascending. No generated candles or time-continuity condition. |
| Current cache | `src/modules/market/service.rs:44-73`: only current aligned unexpired 5m/15m/1h/4h/1d cache may merge; bounds enforced, stored same-slot row wins, newest N retained. End=T-1 excludes the current/loaded boundary slot. No historical gap filling. |
| Market isolation | `src/modules/market/infrastructure/persistence.rs:28-32`: one collection per validated normalized symbol. `src/modules/market/application.rs:175-191`: listed-market check precedes history access. |

## Duplicate and provider semantics

- `src/infra/mongo.rs:35-55` defines the unique `(interval, open_time)` index
  per symbol collection. `src/modules/market/infrastructure/adapters/ingestion.rs:413-453`
  ensures it before writes and handles concurrent same-slot insert/update
  races. The API reads rows as stored; it does not perform its own duplicate
  grouping. Unexpected manually corrupted/indexless data is not a supported
  alternate pagination ordering.
- Provider is **not** part of the key or the read filter. The persisted
  `source` is provenance of the accepted same-slot write;
  `src/modules/market/infrastructure/adapters/ingestion.rs:803-822,838-857,888-904`
  shows provider stored as source and observation time as updated_at, while
  the key stays interval/open_time. No provider precedence is selected by
  the history endpoint.
- `src/modules/market/repository.rs:20-31` and
  `src/modules/market/presentation.rs:207-220` read/map only interval,
  open_time and OHLCV plus the requested symbol; source/updated_at are not in
  the public candle DTO. Mobile should deduplicate by open time inside the
  already-owned symbol/interval session, not by provider, and preserve actual
  live-slot authority separately.
- Actual stored gaps remain gaps. This GET does not invoke provider fetches,
  restore missing external history, create strategy roots, or persist a
  forming aggregate. Pagination accesses already existing historical rows,
  not history that never reached Mongo.

## Mobile integration observations

The pre-implementation Mobile `fetchKlines` in `mobile/src/api/market.ts:93-107`
constructed a fixed-width start/end window. Remove that artificial lower
bound for both latest and older pages. `mobile/src/api/marketSocketProtocol.ts:151-177`
normalizes, deduplicates and retains only the newest `limit` entries, so old
history will be immediately discarded if a prepended session is merged using
the old fixed 160 retention limit. Page size and loaded-session retention must
be distinct concepts.

`mobile/src/api/marketSocketProtocol.ts:483-493` yields millisecond point times
for contemporary backend timestamps. Do not multiply `KlinePoint.time` by
1000 again; chart-library seconds are only a rendering concern.

The endpoint provides no total or `has_more`. An empty correctly-owned page
establishes no older stored rows at query time; a short valid raw page also
does so only against the **effective server page size**. Normalization can
drop malformed rows, so a short mapped page alone should not be described as
proof of backend exhaustion. Always require strictly older progress and
deduplicate to prevent repeated/overlapping response loops. Historical data
can later be repaired, so exhaustion is session state rather than permanent
global metadata.

## Existing tests and verification scope

Read, not executed in this audit:

- `tests/market_routes.rs:129-136`,
  `kline_query_validates_interval_and_clamps_limit`: invalid interval and cap.
- `tests/market_routes.rs:904-1092`,
  `strategy_worker_pushes_book_trades_all_intervals_and_rest_returns_latest_roots`:
  seeds 160 roots, requests limit 160, verifies newest 100 in strictly
  ascending order, includes all intervals and current forming-only cache.
  Requires MySQL, Redis and Mongo; missing environment skips execution.
- `tests/unit_src/src_modules_market_mod_tests.rs:280-332`,
  `forming_read_model_merges_only_current_in_range_slot_and_keeps_latest_limit`:
  current-cache merge, duplicate preservation, stale/future exclusion and
  lower-bound rejection.
- `tests/market_ingestion.rs:258-283`: one same-slot stored row, actual unique
  index, and persisted provider source. `tests/market_ingestion.rs:627-695`
  verifies stale synthetic update leaves stored close and broadcast unchanged.
- `tests/unit_src/src_time_tests.rs:17-60`: integer millisecond serialization
  and optional timestamp roundtrip.

No existing dedicated route test for two historical pages with end-only
exclusive cursors or sparse intervals was found in the reviewed market suites.
Recommended Mobile assertions: end=T-1 and omitted start; actual page size
100; three pages crossing gaps; boundary exclusion; existing live authority;
empty/failed/unchanged cursor boundedness; same symbol/interval ownership.

Audit validation: source-reference existence/range check and a local pure
Python model of the exact inclusive-end/descending-limit/reverse operations.
No Cargo, API/provider calls, services, database access, Git operations,
production/backend edits or shared progress/spec edits were performed.

## Final Mobile integration review

Reviewed the implemented `mobile/src/api/market.ts`,
`mobile/src/core/marketChartHistory.ts`, and
`mobile/src/components/MobileMarketChart.vue`, plus their actual page callers
and existing focused tests. No concrete defect was identified in this narrow
API/history-session ownership review.

- **API contract honored:** `mobile/src/api/market.ts:93-122` uses the same
  end-only query for initial and historical pages. Historical end is before-1
  millisecond, positive safe-integer cursors are checked, request limits are
  floored/clamped to 1..100, and mapped rows beyond end are excluded. There is
  no artificial lookback start and transport failures remain rejected.
- **Timestamp domains remain distinct:**
  `mobile/src/core/marketChart.ts:23-26` normalizes renderer state to Unix
  seconds. `mobile/src/core/marketChartHistory.ts:58,65` converts those
  renderer-state seconds back to milliseconds at the API boundary. The API
  itself accepts/returns millisecond points, so this is not an extra
  conversion of an already-millisecond renderer cursor.
- **Request ownership:** `mobile/src/core/marketChartHistory.ts:33-50,54-73`
  increments generation on symbol/interval replacement or initial reload;
  both fulfillment and failure check the captured generation. Dispose also
  invalidates it. Loading, waiting-for-replacement-points and non-idle states
  suppress dispatch; key-only replacement clears old displayed rows.
  `mobile/src/components/MobileMarketChart.vue:22-32` watches all four owned
  inputs and disposes the session on unmount.
- **Actual caller integration:**
  `mobile/src/views/MarketDetailView.vue:156-194,251-258` clears/replaces the
  stream's current points during symbol/period reload and passes points,
  symbol, interval and chartLoading into the shared wrapper at 626-631.
  `mobile/src/views/TradeView.vue:444-470,488-504` likewise clears points and
  replaces the stream context. Their existing stream generation and K-line
  request generation gates reject old initial REST/live results before those
  points reach the history session; the wrapper separately gates older pages.
- **Sparse and short pages:** `mobile/src/core/marketChartHistory.ts:65-70`
  filters strictly older rows and requires new timestamp progress. It does
  not infer exhaustion from fewer than 100 mapped rows. Each progressed short
  page stays idle for the next user demand; empty/invalid/overlap-only pages
  terminate without automatically requesting more. A network error preserves
  rows and waits for explicit retry (`79-82`).
- **Current/live retention:** history browsing merges the retained session
  before each replacement parent array (`49`), so the parent's bounded rolling
  tail cannot remove the browsed prefix. Current rows follow REST rows when a
  pending page resolves (`68-70`), preserving updates received while loading.
  `normalizeMarketChartPoints` keeps the last row for a timestamp and sorts
  ascending (`mobile/src/core/marketChart.ts:29-51`). Before browsing begins,
  ordinary parent updates still replace rather than accumulate the session.
- **Wrapper loading separation:** older requests change history.status, not
  the parent's chartLoading; renderer `history-loading` remains bound to the
  initial loading prop (`mobile/src/components/MobileMarketChart.vue:50-58`).
  Older network activity therefore does not accidentally restart initial
  hydration/fit behavior. Gesture/viewport behavior is reviewed separately by
  the renderer/browser owner.

Focused regression evidence inspected, not rerun here:

- `mobile/tests/market-kline-history-api.test.ts:39-95`: real API function
  extraction, end-only sparse 276-row traversal, exclusive cursor, clamp,
  invalid cursor dispatch suppression, payload/bound checks and rejection.
- `mobile/tests/market-chart-history.test.ts:29-140`: demand-only single
  flight, short pages, live overlap and rolling retention, no-progress stop,
  explicit retry, stale success/failure across A/B/A, reload and unmount.
- `mobile/tests/mobile-market-chart-history.test.ts:17-97`: actual component
  demand bridge, independent loading, retention, retry/status UI and stale
  ownership across reload/symbol replacement/unmount.

This final pass was static and read-only outside this research document. It
did not rerun gates, launch services, call APIs, inspect live backend data,
or duplicate the root/renderer worker's verification work.
