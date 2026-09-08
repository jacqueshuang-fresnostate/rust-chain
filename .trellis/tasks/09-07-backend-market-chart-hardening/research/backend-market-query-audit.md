# Backend market-query and provider-path audit

## Scope / evidence level

Read-only inspection of the Mobile public K-line REST path, Mongo history reads,
synthetic aggregation, external provider REST fallback and its ingestion/cache
boundary. No production/test source edits, database/network access or Cargo
commands were performed. Findings below are source-confirmed control-flow
behaviors; reproduction instructions are **not** claims of an executed runtime
regression. Existing dirty files were preserved.

The Mobile-visible periods are `1m/5m/15m/1h/1d`
(`mobile/src/api/marketSocketProtocol.ts:24-28`); backend `4h` is also accepted
but is not currently a Mobile tab. Public REST reads only Mongo plus the current
synthetic higher-period Redis slot; it does not call a provider on demand.

## 1. P1: rejected external K-line frames still publish as successful data

### Source chain

- `src/modules/market/infrastructure/cache.rs:446-478`: the Redis Lua compares
  `(open_time, observed_at)` and returns rejection for an older slot, older
  observation, or equal-time conflicting payload.
- `src/modules/market/infrastructure/adapters/ingestion.rs:365-376`:
  `MarketIngestionService::ingest_kline` upserts Mongo only for accepted or
  identical-replay cache results, but returns `Ok(())` for every stale rejection.
- `src/modules/market/infrastructure/adapters/feed.rs:848-859`:
  `MarketFeedWorker::ingest_frame` interprets that `Ok` as publish permission and
  broadcasts the rejected frame through its hub anyway.
- `feed.rs:818-842`: REST fallback also increments `summary.ingested` for it,
  despite neither Redis nor Mongo accepting the data.

### Consequence / assertion points

With a real `MarketIngestionService` and broadcast hub, store current candle
T/observation U; feed a same-slot older observation, or a conflicting same-slot
payload at the same U through `MarketFeedWorker::ingest_frame`. Current behavior
keeps Redis/Mongo unchanged but still emits the rejected payload. This exposes a
WS state that differs from the REST history and can overstate recovery health.
Whether a particular Mobile screen visibly regresses also depends on its own
observation guards; the server-side publication mismatch does not depend on UI.

Add integration assertions for: accepted -> store and one frame; stale -> no
store/no frame; same-time conflicting -> no frame; identical replay -> existing
idempotent behavior; summary counts distinguish accepted from skipped. Existing
`tests/market_ingestion.rs:628` protects the **synthetic** entry, not this ordinary
external worker/sink boundary.

### Bounded repair

Carry the actual ingestion outcome through the adapter-neutral sink contract
(or add a narrowly typed external-K-line entry) so publishing is explicitly
allowed only by accepted outcomes. Preserve synthetic lease/version fences,
archive-before-financial-trigger behavior and same-slot conflict semantics. Do
not make a normal stale frame tear down the whole provider connection merely to
avoid broadcasting it; a skipped outcome is not a network failure.

## 2. P1/P2: REST historical candles are discarded behind latest-slot CAS

### Source chain

- `provider.rs:730-749`, `provider.rs:768-786`, `provider.rs:822-848` (under
  `src/modules/market/infrastructure/adapters/`): REST candle wrappers preserve
  supplied row order and produce a frame per historical row.
- `feed.rs:1088-1124`, `feed.rs:818-842`: fallback fetches these frames then
  feeds each through the same ordinary sink.
- `ingestion.rs:365-375`: Mongo history writes are conditional on acceptance by
  the **single latest candle** Redis key, not on the historical slot's own
  freshness.
- `src/modules/market/infrastructure/persistence.rs:278-301`: Mobile REST later
  reads Mongo alone for external pairs, so skipped rows do not appear after a
  screen reload.

### Deterministic reproduction

With empty Redis/Mongo, make the recorded REST client return three valid candles
T+2m, T+1m, T, in that order. Use the real ingestion sink, not the existing
`RecordedIngestionSink`. Only T+2m is stored; the older two are acknowledged but
omitted. With Redis already on T+3m, none of the missing T..T+2m history is stored,
even if the REST array is sorted ascending. This is a batch-order/current-cache
problem, not a reason to weaken the public query's latest-N ordering.

The existing fallback tests at `tests/market_feed_worker.rs:550` and `:622`
check a recording sink, so they demonstrate dispatch but do not prove Mongo
history completeness or cache non-regression.

### Bounded repair / compatibility boundary

Separate explicit external REST historical ingestion from live cache advancement:
retain per-slot Mongo freshness/conflict checks, preserve the latest Redis slot,
and do not replay historical rows as live broadcasts or financial events. Merely
sorting the batch is insufficient, and unconditionally writing every generic
stale frame would weaken same-slot protection. Assert all historical slots are
stored once, current cache unchanged, no stale WS frames, no ticker/order/ledger
side effects, and no older payload overwrites a newer version of an existing
historical slot.

The no-automatic-gap-fill rule in `synthetic-market-kline.md` belongs to the
strategy/internal recovery protocol. Keep that protocol untouched; do not route
strategy recovery through a new provider fallback. If the desired external
fallback is intentionally latest-only, instead request and count only the latest
row and document that narrow contract rather than presenting a multirow history
batch as fully ingested. This choice should be explicit before implementation.

## 3. P2: Coinbase fallback time window freezes at supervisor generation start

### Source chain

- `provider.rs:216-237`: Coinbase URL embeds `start` and `end=Utc::now()` when
  the fallback config is constructed; window size is 300 candles.
- `src/workers/market_feed.rs:972-995`: construction occurs once when a provider
  generation starts.
- `src/workers/market_feed.rs:1150-1188`: that config is stored in the reconnect
  runtime; `:1313-1323` clones it in every retry loop.
- `feed.rs:1111-1113`: the stored URL is sent unchanged at fallback time.

After a long-lived generation loses its socket, the fallback asks for the old
startup window, not the current one. This contradicts the explicit URL-lifetime
comments at `provider.rs:215` and `feed.rs:378-381`. It affects Coinbase only;
Bitget/HTX URLs do not embed fixed start/end. Coinbase is opt-in, so do not claim
it explains the default strategy/Bitget chart.

Reproduce with a clock-injected URL builder or a recording HTTP client: construct
at T, invoke two fallback attempts at T+Δ and T+2Δ, and compare captured windows.
Desired end tracks request time, start=end-(seconds*300), while base URL, symbol,
period, generation fence and configured timeout remain unchanged. Refresh only
time-dependent request values immediately before a fallback attempt; do not
restart provider generations or make unrelated cache/schema changes.

## 4. P2, adjacent/non-Mobile: accepted backend 4h does not round-trip providers

- `src/modules/market/domain.rs:99-109` and `feed.rs:1265-1275` accept `4h`.
- Bitget inbound map `provider.rs:1105-1116` and HTX map `:1121-1133` reject
  four-hour frames; their outgoing mappings also omit explicit 4h handling.
- Coinbase `provider.rs:370-377` requests `ONE_MINUTE` for `4h`; the REST wrapper
  adds `interval="4h"` at `:835-836`, but `:923-935` then silently maps it to
  `5m`. Thus a valid configured backend interval can become one-minute data
  mislabeled as five-minute history, rather than just missing 4h history.

A no-network reproduction can reuse `provider_rest_fallback_configs_for` and a
recorded Coinbase candle response, requesting only `4h`; assert the URL's
window/granularity and the sink's resulting interval. Add all-six-period
round-trip matrices, explicitly covering unknown explicit interval fields versus
the documented missing Coinbase WS granularity default.

Before implementing, verify each provider's supported wire granularity with its
primary documentation. Either map an actually supported period correctly or
reject an unsupported provider/period combination before network requests;
never remap an explicit 4h into 1m/5m. Do not add a Mobile 4h tab or change the
working synthetic 4h generator as part of this repair.

## Query/aggregation optimizations: evidence, not speculative migration

### Small, compatible public-query cleanup

`application.rs:155-158` checks market listing in MySQL before validating the
interval. Moving `KlineQuery::new` ahead of I/O makes invalid intervals fail
without a database round trip and prevents database outages from masking an
invalid interval as 500. Keep symbol-shape validation first and current listed
market behavior for valid requests.

For non-1m requests with Redis, `application.rs:156` and `:166` issue two MySQL
checks with the same normalized symbol predicate. Both are COUNT scans
(`persistence.rs:172-195`). A combined read of active existence and synthetic
existence can remove one round trip without indexes/cache or changing normalized
symbol semantics. Preserve the current "any active match / any active synthetic
match" behavior if historical normalized-symbol duplicates exist; `LIMIT 1`
would silently change that behavior. Benchmark only if selected for work; this
audit did not measure production latency.

### Existing bounded behavior to preserve

- Public limit is clamped to 1..=100 (`domain.rs:155`); Mongo selects descending
  latest N and then reverses (`persistence.rs:289-300`). No unbounded public
  limit or oldest-N truncation defect remains.
- Mongo time predicates are typed BSON milliseconds and inclusive on both
  bounds; an inverted range intentionally returns no rows under current
  comments (`domain.rs:142`, `persistence.rs:304-318`). Treat a new 400 policy
  as a contract change, not an unproven existing bug.
- Current synthetic higher slots are checked for current UTC bucket, requested
  bounds and duplicates by `service.rs:46-75`. Do not turn partial windows into
  closed Mongo history or revive expired cached slots.
- `synthetic_market.rs:303-317` loads the 24h root history once per tick and
  `:368-378` reuses it for all five forming intervals. It does **not** execute
  five separate full-day reads for forming candles. `:789-834` has a 1440-row
  cap and a 24h window. Extra completed-window reads happen only at period
  close (`:538-604`); removing them or caching roots across manual repairs needs
  stronger evidence than a source-level preference.
- `ingestion.rs:420-423` calls `ensure_kline_indexes` for each accepted write,
  and `src/infra/mongo.rs:37-40` sends `create_index` each time. This is a visible
  per-write administrative round trip, but changing it to initialization/cache
  requires an index-existence, retry/failure and collection-recreation contract.
  Do not silently memoize forever or skip the unique-index gate in this bounded
  chart repair. No schema/index rewrite is proposed.

## Suggested convergence

Prioritize the external ingestion outcome/publication mismatch if backend work
is selected: it is deterministic, affects the active data boundary, and can be
validated with existing sink/hub/Redis/Mongo fixtures. Keep historical ingestion
intent explicit. Coinbase lifetime/4h issues are confirmed but provider-specific
and need separate acceptance criteria. Public query validation ordering and a
combined metadata lookup are narrow optimizations, not claimed production
bottlenecks.

Report-only whitespace validation passed. Root owns task PROGRESS/spec updates
and any selected regression/implementation/database verification.

## Selected-slice proposal: Coinbase request-time window only

Root selected finding 3 as the bounded backend candidate; findings 1/2 need a
coherent dedicated ingestion-outcome/history contract and remain deferred.
No CAS guard, history write policy, broadcasts, provider connection generations,
4h mapping, or synthetic recovery behavior should change in this slice.

Affected implementation boundaries:

1. `src/modules/market/infrastructure/adapters/provider.rs`: introduce/reuse a
   pure Coinbase K-line URL-window materializer taking an explicit UTC timestamp
   and the configured platform interval. Reuse the existing granularity/window
   mapping (including unchanged legacy 4h behavior in this slice), preserve base
   endpoint/product/granularity and any unrelated query fields, and set exactly
   one `start` and one `end` in epoch seconds. Use URL parsing/query-pair APIs,
   not substring replacement. No HTTP happens in the helper.
2. `src/modules/market/infrastructure/adapters/feed.rs`: immediately before each
   K-line HTTP request is actually dispatched, materialize the Coinbase window
   from request metadata and `Utc::now()`. Do not only regenerate once per
   reconnect or at the beginning of a long serial queue. Keep Bitget, HTX and
   Coinbase ticker URLs byte-for-byte unchanged. Failure context must record the
   URL actually attempted; materialization failure uses existing per-request
   isolation rather than aborting unrelated fallback requests.
3. `tests/market_feed_worker.rs` plus a standalone private helper test module
   under `tests/unit_src/` if needed: retain existing all-provider URL tests,
   give the new materializer fixed times T/T+Δ, and add a recorded HTTP test
   proving the dispatcher does not send an intentionally old configured window.
   Existing Coinbase recording currently indexes responses by exact URL captured
   during config construction (`tests/market_feed_worker.rs:105-123,622-659`).
   Adapt only this Coinbase fixture to validate dynamic start/end explicitly and
   match stable endpoint/granularity; do not weaken URL checks globally or rely
   on both calls happening in the same wall-clock second.

Required assertions:

- `end=provided_now.timestamp()`, `start=end-seconds*300` at both fixed times;
  interval window widths for supported 1m/5m/15m/1h/1d remain unchanged.
- Two materializations of the same long-lived config move the window forward;
  product/path, granularity, other query values and fragments remain intact;
  no duplicate start/end query pairs survive.
- Dispatch records the refreshed URL and ingests the same fixture payload;
  request errors still report provider/channel/symbol/interval and attempted URL.
- Static provider/ticker URLs and existing custom base settings remain unchanged;
  no real network, Redis, Mongo or MySQL is necessary for this slice's tests.

Expected gates after ownership assignment: focused provider/adapter/worker tests,
Rust fmt, backend architecture/documentation, then root-owned check/clippy. This
report does not claim those tests have been added or executed.
