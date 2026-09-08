# Default Market Generation for Platform Pairs

## Scope and priority

Applies to `strategy/internal` pair baseline generation, scheduled strategy
handoff, Admin default-generator APIs, and default-source price capability.
External-provider failover is a different feature; follow mode below reads external evidence only and remains a platform-generated source.

`pair disabled / all-market pause > currently effective active manual schedule > enabled default`.
An active schedule with a failed/missing run is **not** absence of a schedule:
fail closed, never start a second writer. Draft, future and ended schedules do
not suppress default generation. Existing pairs start unconfigured/disabled;
no migration implicitly enables a live generator or invents its initial price.

## Storage and APIs

Migration `0124_default_market_generators.sql` is immutable once applied:

- `market_default_generators`: pair PK, explicit enabled/all_market_paused,
  nullable DECIMAL(38,18) initial_price, config_json, positive version, stable
  seed, updater and timestamps.
- `market_default_generator_versions`: immutable `(pair_id, version)` snapshots.
- `market_pair_generation_runs`: pair PK, generation, source
  none/default/strategy, exact source version, lease owner/expiry, fixed-minute
  state JSON, last committed price/tick and error.
- `market_price_ticks.source='default'`: no artificial strategy ID/version.
  `source_version=default:{pair_id}:g{generation}:v{config_version}`. Existing
  manual archive generation remains its strategy version, not the pair epoch.

Exact `market.pairs.read/write` permissions protect these Admin routes:

```text
GET   /admin/api/v1/market-pairs/:id/default-generator
PATCH /admin/api/v1/market-pairs/:id/default-generator
POST  /admin/api/v1/market-pairs/:id/default-generator/preview
PATCH /admin/api/v1/market-pairs/:id/default-generator/pause-all
```

Save requires expected_version, enabled, initial_price, complete config and
reason. Pause requires expected_version, all_market_paused and reason. Both
append a version and audit in the same transaction; stale versions return 409,
audit failure rolls back. Saving parameters preserves the all-pause flag.
Preview takes expected_version, initial_price/config and returns 60 samples;
it writes no config, runtime, cache, candles, orders or audit mutation.
All decimal values are strings; timestamps are Unix milliseconds.

Parameters: volatility/mean_reversion/wick_strength ratios 0..1;
optional positive ordered price_min/max; nonnegative ordered volume_min/max;
depth_levels 1..20. Enforce pair precision 0..18 and DECIMAL integer capacity
before expensive arithmetic; do not silently round submitted settings.
Code defaults are 0.0015 / 0.05 / 0.25, no price limits, volume 10..20, 20 levels.
Zero volume produces no trade; a coarse price tick can make small volatility
invisible. These defaults make no directional-return guarantee.

## Runtime invariants

1. One named MySQL lock per database+pair spans archive, cache, candle,
   publication and checkpoint. Manual and default workers share it; all-pause
   and default saves drain it before committing. Lock order is named lock →
   pair row → config → runtime. A detached connection plus semaphore (8 per
   process) prevents holders exhausting the business pool while waiting for
   another transaction connection. Never return a lock-bearing connection to
   the pool. Test max_connections=1, cancellation/drop and contention.
2. Re-read default configuration **after** acquiring the lock. Activate/archive
   transactions recheck pair status, pause, schedule, version, DB-clock lease
   and actual `IS_USED_LOCK` connection identity. Legacy manual callers without
   a pair fence are rejected once a shared runtime exists. Global symbol
   archive time is monotonic across sources; old/different equal-time events
   fail before cache/financial price consumers.
3. Persist the complete deterministic current minute, parameters, seed/version
   and anchor **before** publishing its first event. A reserved current minute
   blocks a different source/version even if the ticker committed but the first
   checkpoint failed. Changes and handoff wait for the next UTC minute; all-pause
   stops immediately. Same default minute resumes unchanged after pause/restart.
4. At a new default minute use the last accepted archive price, falling back to
   a valid checkpoint then explicitly configured initial price. A continuous
   online previous close may supply its final value. Never reset hourly or use
   a manual strategy's nominal target as the default resume price. Manual
   algorithms preserve saved start/target contracts; activation must warn of
   discontinuity and require deliberate confirmation, not silently re-anchor.
5. Ticker close, 1m close, simulated prints and depth come from the same fixed
   candle/second. Cumulative default volume is quantized before differencing;
   60 per-second quantities sum exactly to its configured minute volume.
   5m/15m/1h/4h/1d are derived from authoritative 1m roots; never run independent
   random paths per chart period. Synthetic trades do not create spot fills.
6. Startup never scans/fills missing historical minutes. Only a same-process
   ≤5-second continuous boundary can register a new previous-minute closure.
   Persist that bounded `pending_close` before replacing the minute state;
   any subsequent owner may retry the already registered adjacent closure
   during the current minute. Clear it only after old 1m and its complete
   higher aggregates succeed. This repairs a partial publication, not missing
   historical data; a later restart creates no past candles or full-window fill.
7. Individual errors are recorded without resetting accepted prices. Default
   candidates use oldest checkpoints first, with a separate 1..100 bound from
   the manual scan. A connected socket is not a health guarantee.

## External-reference follow mode

- Existing JSON with no `mode/follow` remains exactly `independent` with
  `follow:null`; explicit follow uses `{ reference_pair_id, multiplier,
  max_move_ratio, stale_after_seconds }`. Ratios are exact decimal strings,
  multiplier `(0,3]` (default 1), per-minute movement cap `(0,1]` (default .05),
  freshness 15..300 seconds (default 60). Save/preview reject mismatched modes,
  self/missing/inactive/non-external references. No new migration or implicit
  configuration activation is required for these additive JSON fields.
- Read positive non-future bitget/htx/coinbase archive observations for the
  selected external pair; normalize only that pair's known symbol. These are
  existing collector observation timestamps, not guaranteed exchange trade
  times or final candles. Bound each provider lookup to the freshness window.
  Keep the healthy preferred provider, otherwise use the shared deterministic
  selector. Detect every conflicting same-provider/latest-time price (runtime
  SQL EXISTS, preview full ties); never hide a conflict with another source.
- Floor the effective time to UTC seconds **before** reading evidence. Freeze
  the selected observation, own frame, anchors, previous price and status
  before publishing. Same-second retry reuses the frame. For a new second,
  reconcile the frozen frame against its immutable archive identity, including
  original pair/version and archive-row generation, not the current generation
  invalidated by pause. A failed unarchived frame is discarded in favor of
  `accepted_follow`; an archived frame survives cache/checkpoint failure.
  Apply this reconciliation before both default advancement and manual
  takeover so pending prices never enter the old minute's closed OHLC.
- Map anchored relative returns onto this pair's price, not reference absolute
  price. Duplicate observations do not compound. Preserve the consumed
  reference watermark during fallback; same-provider time regression or
  equal-time price conflict cannot re-anchor into duplicate returns. Provider
  changes, stale recovery and long interruptions re-anchor to accepted own
  price without missed-move catch-up. Each new minute rebases the own anchor
  and enforces the configured minute movement cap plus global price bounds.
- Follow OHLC is accumulated only from actually accepted/generated frames;
  no reference future path or unobserved high/low is invented. Independent
  deterministic minute plans supply fallback price motion and own simulated
  volume. Shared price limits/volume/depth settings still apply in follow mode.
  On reference loss, enter independent motion continuously and persist a
  Chinese fallback reason; recovery clears it and records switched_at.
- Admin `reference_pair` is directory metadata, `runtime.follow` is the last
  saved run snapshot, not live monitoring. Preview replays the preceding 60
  completed minute windows from bounded archived observations using the same
  per-second state machine and selector. It is a model replay based on the
  selected own starting price, neither actual past own prices nor a forecast.
  `follow_preview` includes kind, reference ID/symbol, range, used sample count
  and warnings. Missing evidence/over 20,000 rows yields explicitly labeled
  independent fallback, never a truncated replay presented as complete.
- Required follow regressions: `synthetic_follow`, `default_market_follow_config`,
  `default_market_follow_reference`, `default_market_follow_runtime` and Admin
  default routes. Cover stale/recovery, subsecond cutoffs, replay, pending writes,
  pause generation changes and cross-minute/manual closures.

## Consumers and rollout boundaries

Default ticker ingestion retains existing price-trigger integration. Simulated
print generation never inserts user orders/fills. Seconds capability is separate:
default enabled, not all-paused, healthy exact source/version/generation, live
owner lease, and matching non-future archived evidence in the latest 60 seconds.
Existing settlement windows, outcomes and refund semantics are unchanged.
All-market pause stops producers, not existing orders/positions or historical
cache; it is not a universal trading-kill-switch.

Stop/drain old synthetic workers before migration/new-worker rollout. Do not mix
old publishers that lack pair fencing. Route platform symbols exclusively to
synthetic producers; external-feed overlap is not coordinated by this lock.
Deployment, numeric pair activation and real-order validation require a separate
administrator handoff. Keep generator disabled until reviewed.

## Required regression evidence

- Pure defaults + forming volume: `synthetic_default`, private generator tests.
- Reservation, source/version identity, pause, restart and durable pending:
  `tests/unit_src/src_workers_synthetic_market_default_market_tests.rs`.
- Real local MySQL/Mongo/Redis: `default_market_runtime`,
  `default_market_transition`; isolated MySQL `default_market_ownership` and
  `admin_routes::default_market` including version, permissions, atomic audit,
  price selection, pause drain and seconds capability failures.
- Existing synthetic strategy/details/worker/migration and seconds capability
  tests; architecture, fmt/check/clippy, Web interaction and mobile release gate.
