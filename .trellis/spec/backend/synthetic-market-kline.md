# Synthetic Market and Manual K-line Recovery Contracts

## Scenario: Deterministic strategy-market OHLCV and explicit historical recovery

### 1. Scope / Trigger

- Trigger: any change to `market_type = "strategy" | "internal"` market
  strategies, `market_strategy_nodes`, strategy versions/runs, the synthetic
  generator or realtime worker, market ingestion, K-line aggregation, admin
  strategy APIs, or manual K-line recovery.
- This feature owns synthetic ticker/K-line production for active strategy
  pairs. It does not create L2 depth or synthetic trades, and it does not
  change the spot-order reservation and settlement contracts.
- Realtime publication and historical recovery are deliberately asymmetric:
  realtime uses the existing Redis/Mongo/WebSocket ingestion path, while
  manual recovery is an administrator-confirmed Mongo history operation.
- A process start or restart must not scan or fill historical gaps. Missing
  historical minutes remain visible to the admin gap API until an
  administrator previews and executes a recovery.
- Traceability source: task PRD
  [`08-12-synthetic-new-coin-market`](../../tasks/08-12-synthetic-new-coin-market/prd.md).
  If code, migration, or UI behavior diverges, update this executable contract
  and the PRD wording together; acceptance checkboxes remain evidence-driven.

### 2. Signatures

#### Database and storage

- Existing MySQL tables used by this feature:
  - `trading_pairs`: `id BIGINT UNSIGNED`, `symbol VARCHAR(64)`,
    `price_precision INT`, `status VARCHAR(32)`, `market_type VARCHAR(32)`.
  - `market_strategies`: `id BIGINT UNSIGNED`, `pair_id BIGINT UNSIGNED`,
    `strategy_type VARCHAR(32)`, `start_price/target_price DECIMAL(38,18)`,
    `start_time/end_time TIMESTAMP(6)`, `volatility DECIMAL(18,8)`,
    `volume_min/volume_max DECIMAL(38,18)`, `status VARCHAR(32)`.
  - `strategy_versions`: `id BIGINT UNSIGNED`,
    `strategy_id BIGINT UNSIGNED`, `version INT`,
    `effective_time TIMESTAMP(6)`, `config_json JSON`, `seed VARCHAR(128)`,
    `created_by BIGINT UNSIGNED NULL`, `created_at TIMESTAMP(6)`; unique
    `(strategy_id, version)`.
  - `strategy_runs`: existing checkpoint columns plus
    `active_version INT NOT NULL`, `lease_owner VARCHAR(128) NULL`, and
    `lease_expires_at TIMESTAMP(6) NULL`; index
    `(lease_expires_at, strategy_id)`; foreign key
    `(strategy_id, active_version) -> strategy_versions(strategy_id, version)`.
  - `strategy_events`: `strategy_id BIGINT UNSIGNED`,
    `event_type VARCHAR(64)`, `payload_json JSON`, `created_at TIMESTAMP(6)`.
- `market_strategy_nodes`:
  - `id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT`
  - `strategy_id BIGINT UNSIGNED NOT NULL`
  - `sequence_no INT UNSIGNED NOT NULL`
  - `target_time TIMESTAMP(6) NOT NULL`
  - `target_type VARCHAR(32) NOT NULL`
  - `target_value DECIMAL(38,18) NOT NULL`
  - `execution_mode VARCHAR(16) NOT NULL`
  - `tolerance DECIMAL(18,8) NOT NULL DEFAULT 0`
  - `volatility DECIMAL(18,8) NOT NULL`
  - `volume_min DECIMAL(38,18) NULL`
  - `volume_max DECIMAL(38,18) NULL`
  - `created_at`, `updated_at TIMESTAMP(6)`
  - unique `(strategy_id, sequence_no)` and `(strategy_id, target_time)`;
    index `(strategy_id, target_time)`; foreign key to
    `market_strategies(id)` with `ON DELETE CASCADE`; checks enforce the three
    target types, three execution modes, non-negative tolerance/volatility,
    positive absolute targets or percentage targets greater than `-100`, and
    paired non-negative volume bounds with `volume_max >= volume_min`.
- `kline_recovery_jobs`:
  - `id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT`
  - `strategy_id`, `requested_by BIGINT UNSIGNED NOT NULL`
  - `config_version INT NOT NULL`
  - `range_start`, `range_end TIMESTAMP(6) NOT NULL`
  - `preview_token_hash CHAR(64) NOT NULL UNIQUE`
  - `reason VARCHAR(512) NOT NULL`
  - `status VARCHAR(32) NOT NULL DEFAULT 'pending'`
  - `expected_1m_count`, `actual_1m_count`,
    `actual_aggregate_count INT UNSIGNED NOT NULL`
  - `error_message TEXT NULL`, `started_at`, `completed_at`, `created_at`,
    `updated_at`
  - unique `preview_token_hash`; indexes `(strategy_id, created_at)` and
    `(status, created_at)`; checks require `range_end > range_start`,
    `expected_1m_count > 0`, `actual_1m_count <= expected_1m_count`, and
    status `pending | running | completed | failed`; foreign keys to the
    strategy and requesting admin.
- Mongo collection: `kline_collection_name(ValidatedMarketSymbol)`.
  A candle is idempotently addressed inside that symbol collection by
  `(interval, open_time)`. Decimal OHLCV values are stored as strings; manual
  writes also store `source = "strategy"` and `updated_at`.
- Realtime Redis keys remain:
  `market:ticker:{SANITIZED_SYMBOL}` and
  `market:kline:{SANITIZED_SYMBOL}:{interval}`.

#### Strategy APIs

All timestamps below are Unix-millisecond JSON integers and all decimal fields
are JSON decimal strings unless stated otherwise. Every route is
admin-authenticated.

- `GET /admin/api/v1/market-strategies?pair_id?:u64&status?:string&limit?:u32&offset?:u32`
  -> `{ strategies: StrategySummary[], total: i64 }`.
- `GET /admin/api/v1/market-strategies/:id`
  -> `StrategyDetail`, which flattens all `StrategySummary` fields and adds
  `nodes: StrategyNode[]`.
- `POST /admin/api/v1/market-strategies` request:

  ```json
  {
    "pair_id": 21,
    "strategy_type": "price_path",
    "start_price": "1.000000000000000000",
    "target_price": "2.000000000000000000",
    "start_time": 1775023200000,
    "end_time": 1775030400000,
    "volatility": "0.01000000",
    "volume_min": "10.000000000000000000",
    "volume_max": "20.000000000000000000",
    "nodes": [],
    "status": "draft",
    "reason": "create strategy"
  }
  ```

  `status` and `reason` are optional on create; missing `status` becomes
  `draft`. Response is `StrategyDetail`.
- `PATCH /admin/api/v1/market-strategies/:id` accepts the same configuration
  fields except `pair_id` and `status`; `nodes` defaults to `[]` when omitted,
  and `reason` is required and trimmed. Response is `StrategyDetail`.
- `PATCH /admin/api/v1/market-strategies/:id/status` request
  `{ "status": "draft|active|paused|disabled", "reason": "..." }`
  -> `StrategySummary`.
- `StrategySummary` fields:
  `id`, `pair_id`, `symbol`, `market_type`, `strategy_type`, `start_price`,
  `target_price`, `start_time`, `end_time`, `volatility`, `volume_min`,
  `volume_max`, `status`, `run_status?:string`, `active_version?:i32`,
  `current_price?:decimal`, `last_generated_at?:milliseconds`,
  `last_kline_open_time?:milliseconds`, `recovery_status?:string`,
  `created_at`.
- `StrategyNode` fields:
  `id`, `sequence_no`, `target_time`, `target_type`, `target_value`,
  `execution_mode`, `tolerance`, `volatility`, `volume_min?`, `volume_max?`.
  A node write uses the same fields without `id` and `sequence_no`; request
  array order assigns `sequence_no` starting at zero.

#### Gap and recovery APIs

- `GET /admin/api/v1/market-strategies/:id/kline-gaps?range_start?:milliseconds&range_end?:milliseconds`
  ->

  ```json
  {
    "strategy_id": 21,
    "config_version": 3,
    "gaps": [{
      "range_start": 1775023500000,
      "range_end": 1775023800000,
      "one_minute_count": 5
    }],
    "total_1m_count": 5
  }
  ```

- `POST /admin/api/v1/market-strategies/:id/kline-recovery/preview` request
  `{ "range_start": milliseconds, "range_end": milliseconds }` ->
  `strategy_id`, `config_version`, `range_start`, `range_end`,
  `one_minute_count`, `aggregate_intervals`, `first_price`, `last_price`,
  `samples[]`, `preview_token`, and `expires_at`.
  Each sample contains `open_time`, `open`, `high`, `low`, `close`, `volume`.
  `aggregate_intervals` is exactly `["5m", "15m", "1h", "4h", "1d"]`;
  samples contain at most 12 candles (first six and last six for a longer
  range).
- `POST /admin/api/v1/market-strategies/:id/kline-recovery/execute` request
  `{ "preview_token": "...", "reason": "..." }` -> one recovery job.
- `GET /admin/api/v1/market-strategies/:id/kline-recovery/jobs?status?:pending|running|completed|failed&limit?:u32&offset?:u32`
  -> `{ jobs: RecoveryJob[], total: i64 }`.
- `RecoveryJob` fields:
  `id`, `strategy_id`, `requested_by`, `config_version`, `range_start`,
  `range_end`, `reason`, `status`, `expected_1m_count`, `actual_1m_count`,
  `actual_aggregate_count`, `error_message?`, `started_at?`, `completed_at?`,
  `created_at`, `updated_at`. Neither the token nor its hash is returned.
- Errors use the shared JSON envelope
  `{ "code": "...", "message": "..." }`.

#### Startup-compatible environment

- `KLINE_RECOVERY_ENABLED` remains the compatibility switch for starting the
  **realtime synthetic worker**. Default: `true`. It no longer starts any
  historical-recovery loop.
- `KLINE_RECOVERY_BATCH_LIMIT` remains the realtime strategy scan limit;
  default `100`, runtime-clamped to `1..=100`.
- `KLINE_RECOVERY_INTERVAL_SECONDS` is still parsed for deployment/config
  compatibility, default `30`, but it no longer controls a worker and must not
  trigger automatic compensation. The synthetic worker cadence is one second.
- Realtime startup additionally requires configured MySQL, MongoDB, and Redis.
  Manual gap/preview/execute routes require MySQL and MongoDB but deliberately
  do not require Redis.
- Existing required `DATABASE_URL`, `MONGODB_URI`, `MONGODB_DATABASE`, and
  `REDIS_URL` supply those stores. `JWT_SECRET` signs the ten-minute
  HMAC-SHA256 preview token; no separate recovery secret is introduced.

### 3. Contracts

#### Versioning, nodes, and deterministic seed

- Strategy configuration is immutable by version. A successful create writes
  version `1`; a successful update appends `max(version) + 1`, writes the full
  node array into `strategy_versions.config_json`, and changes
  `strategy_runs.active_version` in the same MySQL transaction.
- Create/update must atomically persist the strategy row, relation nodes,
  version snapshot, run/checkpoint state, strategy event, and admin audit.
  Updating replaces the full relation-node set only after the strategy row is
  locked. An active strategy must first be paused or disabled.
- Runtime and recovery must load the version selected by
  `strategy_runs.active_version`; never infer the active configuration with
  `MAX(strategy_versions.version)`. Version JSON `nodes` is authoritative when
  present. Absence of that key may fall back to relation rows for legacy data;
  an empty array is an intentional no-node snapshot.
- Nodes are ordered by request order and strictly increasing `target_time`.
  Times are UTC-minute aligned and lie strictly inside
  `[start_time, end_time)`. The strategy `start_price` is the first anchor and
  legacy `target_price` at `end_time` is the compatibility final anchor.
- `target_type` is one of:
  - `absolute_price`: `target_value` is a positive target price.
  - `percent_from_start`: target is
    `start_price * (1 + target_value / 100)`.
  - `percent_from_previous`: target is the previously resolved node price
    multiplied by `1 + target_value / 100`.
- `execution_mode` is one of `hard`, `soft`, `range`. `hard` makes the candle
  whose close time equals `target_time` close exactly at the precision-rounded
  target. `soft` and `range` use deterministic displacement inside the
  percentage tolerance band; `soft` uses half the range displacement.
- A node either supplies both non-negative `volume_min` and `volume_max` with
  `max >= min`, or supplies neither and inherits the strategy range. Node
  `volatility` and `tolerance` are non-negative.
- Every stochastic component is derived independently with SHA-256 from the
  byte sequence `seed`, delimiter, big-endian `version`, delimiter,
  normalized uppercase `symbol`, delimiter, big-endian
  `open_time.timestamp_millis()`, and a component label. Labels isolate price,
  anchor, high wick, low wick, and volume draws.
- Therefore identical `(seed, active_version, symbol, open_time, full version
  config)` produces byte-for-byte equal decimal OHLCV in domain generation,
  realtime finalization, preview, retries, and manual execution, regardless of
  restart, instance, scan order, or batch boundary.
- Prices are rounded with half-up mode to `trading_pairs.price_precision` and
  remain at least one precision unit. Every generated candle satisfies:
  `open > 0`, `close > 0`, `low > 0`,
  `high >= max(open, close)`, `low <= min(open, close)`, and `volume >= 0`.
  Adjacent deterministic 1m candles satisfy `previous.close == next.open`.

#### Authoritative 1m and aggregation

- Deterministic closed `1m` candles are the sole history authority.
  `5m`, `15m`, `1h`, `4h`, and `1d` must never run independent random
  generation.
- A higher interval consumes exactly one complete UTC-aligned, ascending,
  continuous 1m window. Aggregate values are first open, maximum high, minimum
  low, last close, and sum of volume. A missing, unaligned, invalid, or
  open/close-discontinuous window is rejected rather than partially published.
- Realtime closes the previous minute and rebuilds a completed aggregate only
  when this same process successfully published the immediately preceding
  minute, the configuration version is unchanged, the next tick enters the
  adjacent minute, and the observation gap is at most five seconds.
  Lack of in-memory continuity after restart is intentional and must not be
  reconstructed from the checkpoint.

#### Realtime worker, `active_version`, and lease

- A realtime scan selects only active pairs, active strategies, pair
  `market_type IN ('strategy','internal')`, run state `running|live`, and
  `start_time <= now < end_time`, joined to the exact `active_version`.
- Each process keeps one stable non-blank lease owner. A strategy has a
  60-second renewable lease in `strategy_runs`; acquisition succeeds only
  when the lease is absent/expired or already owned by the caller and the
  expected `active_version` is still current. At most one owner publishes a
  strategy at a time.
- The worker generates only `floor_utc_minute(now)`. It never loops from
  `last_kline_open_time` or `last_generated_at` toward the present. On restart
  it publishes the current forming minute and leaves every stopped minute as a
  detectable gap.
- A forming minute follows a deterministic second-level path through the
  candle extremes toward the final close; volume accumulates by observed
  seconds. Observations within the same second are equal, and second 59 equals
  the deterministic closed 1m candle exactly.
- Realtime calls the existing `MarketIngestionService` in this order:
  `ingest_and_publish_kline`, then `ingest_and_publish_ticker`, then checkpoint
  update. K-line ingestion writes Redis then Mongo and publishes the existing
  K-line event only after storage succeeds. Ticker ingestion writes Redis,
  retains the existing spot limit-order trigger side effect, and publishes the
  existing ticker event. There is no new cache or WebSocket protocol.
- Ticker `last_price` equals the forming 1m close from the same plan. The 24h
  open/high/low/volume/change values combine the current forming candle with
  up to the preceding 1,440 stored 1m candles.
- Public events retain `provider = "strategy"`, ticker topic
  `public:ticker:{SYMBOL}`, and K-line topic
  `public:kline:{SYMBOL}_{interval}` with the payload fields documented in
  [Realtime WebSocket Contracts](./realtime-websockets.md).
- Checkpoint update is valid only for the same strategy, lease owner,
  unexpired lease, run state, and `active_version`. Only after K-line and
  ticker ingestion succeed may it set `current_price`, `last_tick_at`,
  `last_generated_at`, `last_kline_open_time`, `recovery_status = 'live'`,
  and clear the error. A failed item records a bounded error and does not stop
  later strategies.
- Redis/Mongo/MySQL are not one transaction. Slot upserts, lease/version guards,
  and deterministic replay provide convergence; code must not claim rollback
  of already completed cache/history writes.
- Redis freshness scripts must decode their integer `0|1` reply as an integer,
  never as `Option<i64>`: RESP integer `0` is a real stale result rather than
  nil. Ticker CAS compares JSON `observed_at`; K-line CAS compares the pair
  `(open_time, observed_at)` kept in an internal sequence key while preserving
  the public K-line JSON shape. A rejected write must stop Mongo, spot-order,
  WebSocket, and checkpoint side effects.

#### Detection, preview, and manual execution boundary

- Gap ranges use `[range_start, range_end)` over closed, UTC-minute-aligned 1m
  slots inside the strategy's half-open time range. Detection defaults to the
  strategy start and `min(strategy.end_time, floor_utc_minute(now))`, clamps
  optional bounds to those limits, merges adjacent missing minutes, and has no
  write side effects.
- Preview requires an explicit non-empty range of at most 10,080 minutes and
  requires every minute in that range to be absent. It reads the selected
  active version and seed, generates the exact execution OHLCV, and returns a
  bounded sample. Preview writes no Mongo document, Redis key, WebSocket
  event, checkpoint, recovery job, strategy event, or admin audit.
- A preview token is URL-safe payload plus HMAC-SHA256 signature. It binds
  token version, strategy ID, `active_version`, range, expected root count, a
  SHA-256 digest of sorted/deduplicated missing open times, and a ten-minute
  expiry. Execution must revalidate all of them against current storage.
- Execute requires a trimmed, non-blank audit reason of at most 512 characters.
  It stores only `SHA256(preview_token)` and uses its unique constraint as the
  request idempotency key.
- First execution creates the pending job, the
  `market_strategy.kline_recovery.requested` event, and the admin audit action
  `market_strategy.kline_recovery.execute` in one MySQL transaction. It then
  atomically claims `pending -> running`, performs Mongo writes, and commits a
  `completed` or `failed` job plus the matching strategy event in a terminal
  MySQL transaction. Because Mongo and MySQL cannot share a transaction, a
  failed job records the counts already upserted.
- Manual recovery upserts only historical closed `1m` documents and affected
  complete `5m/15m/1h/4h/1d` windows. It reads each complete 1m window back
  from Mongo before aggregation. Re-execution converges on
  `(interval, open_time)` without duplicate documents.
- **Manual recovery must not write any Redis ticker, Redis K-line snapshot,
  WebSocket event, market ingestion call, spot-order trigger, or
  `strategy_runs` checkpoint.** It therefore cannot replace or move the live
  price backward.
- Replaying a token whose job is `completed` or `failed` returns that same job
  without another Mongo pass, event, or audit. A `pending` job resumes from its
  original full half-open range so a crash after partial Mongo writes can
  converge through idempotent upserts. A `running` job younger than 15 minutes
  returns conflict; a stale `running` job may be atomically reclaimed and
  resumed from the original range.

### 4. Validation & Error Matrix

| Condition | Required result |
|---|---|
| Missing/invalid admin token | `401 UNAUTHORIZED` or `403 FORBIDDEN`; no read/write |
| MySQL absent | `500 INTERNAL_ERROR`; no partial operation |
| Mongo absent for gap/preview/execute | `500 INTERNAL_ERROR` with market-recovery dependency message |
| Realtime Redis absent | worker is not started; direct construction returns `INTERNAL_ERROR` |
| Pair is missing or not active | create strategy -> `404 NOT_FOUND` |
| Active pair is not `internal|strategy` | create strategy -> `400 VALIDATION_ERROR` |
| Blank `strategy_type`, non-positive strategy price, negative volatility/volume, reversed volume range, or `end_time <= start_time` | `400 VALIDATION_ERROR` |
| Node time is unaligned, non-increasing, at/before start, or at/after end | `400 VALIDATION_ERROR` with node-time message |
| Unknown node target type or mode | `400 VALIDATION_ERROR` |
| `absolute_price <= 0`, negative tolerance/volatility, or only one node volume bound | `400 VALIDATION_ERROR` |
| Update while strategy is active | `409 CONFLICT`; strategy, nodes, version, run, event, and audit unchanged |
| Invalid/missing update or execute reason | `400 VALIDATION_ERROR`; no task/audit/write |
| Recovery range unaligned, empty/reversed, outside strategy, or includes current/future minute | `400 VALIDATION_ERROR` |
| Recovery range exceeds 10,080 1m roots | `400 VALIDATION_ERROR` |
| Detection has no closed effective range or no gaps | `200` with `gaps=[]`, `total_1m_count=0` |
| Preview range contains an existing 1m root or no gap | `409 CONFLICT`; no token-side write |
| Token malformed, signature invalid, expired, or for another strategy | `400 VALIDATION_ERROR` |
| `active_version` changed after preview | `409 CONFLICT`; no job or K-line write |
| Missing-time count/digest changed after preview | `409 CONFLICT`; no job or K-line write |
| Same token already `pending` | resume original full range with idempotent upserts |
| Same token already `running` within 15 minutes | `409 CONFLICT` |
| Same token already `running` for at least 15 minutes | atomically reclaim and resume original full range |
| Same token already `completed|failed` | `200` with the original job; no duplicate side effect |
| Unknown recovery-job status filter | `400 VALIDATION_ERROR` |
| Aggregate window is incomplete, unaligned, invalid, or discontinuous | do not write/publish that aggregate; execution records failure and actual progress |
| Lease lost/expired or version changed before checkpoint | `409 CONFLICT`; stale worker must not advance checkpoint |
| One realtime strategy fails | increment `failed`, record bounded error for the owned run, continue later strategies |
| Manual execution fails after some Mongo upserts | job becomes `failed`, preserves `actual_*` counts and error; no Redis/WS/checkpoint write |

### 5. Good / Base / Bad Cases

- Good: version 3 with a hard `percent_from_previous` node generates the same
  OHLCV in preview and execution; the hard close hits the resolved target,
  manual execution rebuilds complete aggregates, and retry returns the same
  completed job.
- Good: two instances scan one active strategy; only the lease owner ingests
  current-minute K-line/ticker and advances the version-guarded checkpoint.
- Base: a legacy strategy has no node rows and no `nodes` key in an old
  snapshot; it follows `start_price -> target_price` and continues to run.
- Base: the service restarts after eight hours; the first realtime pass writes
  only the current forming minute. The eight-hour gap remains in `kline-gaps`.
- Bad: start `kline_recovery::run_loop` from `main`, derive missing minutes from
  `last_kline_open_time`, or use `KLINE_RECOVERY_INTERVAL_SECONDS` as an
  automatic backfill timer.
- Bad: let manual recovery call `MarketIngestionService`; this can overwrite
  live Redis data, emit historical WebSocket updates, trigger spot orders, and
  move the checkpoint backward.
- Bad: independently generate a 15m candle or aggregate a partial 1m window;
  higher intervals would disagree with public 1m history.
- Bad: decode a Redis Lua `return 0/1` as an optional/string response; Redis
  returns an integer and a mismatched response type can fail the first normal
  ingestion before any stale comparison is reached.

### 6. Tests Required

Run the focused deterministic and boundary tests:

```bash
cargo test --manifest-path Cargo.toml --test synthetic_market
cargo test --manifest-path Cargo.toml --test synthetic_market_worker
cargo test --manifest-path Cargo.toml --lib workers::kline_recovery::tests
cargo test --manifest-path Cargo.toml --test synthetic_market_migration -- --nocapture
cargo test --manifest-path Cargo.toml --test market_redis_cache -- --nocapture
cargo test --manifest-path Cargo.toml --test market_ingestion -- --nocapture
```

Assert identical slot replay, adjacent continuity, hard/percentage nodes,
soft/range tolerance, positive precision-rounded OHLCV, all five complete 1m
aggregations, partial/discontinuous rejection, forming-minute finalization,
ticker close equality, lease/version SQL guards, one-second cadence, and both
restart/skipped-slot no-auto-close cases. Redis/Mongo integration must assert
first/new writes are accepted, equal/older writes are rejected, 32-way ticker
writers converge on the newest timestamp, and rejected K-lines do not mutate
Mongo or publish a WebSocket event.

Run route and storage integration tests with isolated MySQL and Mongo fixtures:

```bash
DATABASE_URL="$DATABASE_URL" MONGODB_URI="$MONGODB_URI" \
  cargo test --manifest-path Cargo.toml --test admin_routes admin_market_strategy -- --nocapture
DATABASE_URL="$DATABASE_URL" MONGODB_URI="$MONGODB_URI" \
  cargo test --manifest-path Cargo.toml --test admin_market_recovery -- --nocapture
```

Assert relation-node order and JSON snapshot equality, `active_version`
changes with the new version, active-update rollback, gap half-open counts,
preview purity, ten-minute token binding, version/gap conflict behavior,
job transitions, event/audit names, all interval upserts, same-token replay,
and operation without Redis. Capture Redis keys, WebSocket hub messages, and
`strategy_runs` before/after manual execution and assert exact equality.

The migration test must apply the exact immutable SQL to a fresh MySQL 8
fixture and assert every column type/nullability/default,
check/unique/index/foreign-key constraint, `active_version` backfill, and
rejection of a run whose active version is absent. The repository migration
gate must also run the migrator twice to prove checksum cleanliness:

```bash
sqlx migrate run
sqlx migrate run
```

Run admin UI tests for the node editor and compensation SideSheet, then the
standard gates:

```bash
npm --prefix web run test -- \
  src/admin/components/MarketStrategyNodeEditor.test.tsx \
  src/admin/actions/MarketStrategyActions.test.tsx \
  src/admin/resources/resourceConfigs.test.tsx
npm --prefix web run typecheck
npm --prefix web run lint
cargo fmt --manifest-path Cargo.toml --all -- --check
cargo check --manifest-path Cargo.toml --all-targets
cargo test --manifest-path Cargo.toml --test backend_architecture
cargo test --manifest-path Cargo.toml --test backend_documentation
```

UI assertions must cover Chinese field names, repeated-row accessible names,
detail-before-edit node preservation, detect -> preview -> trimmed-reason
execute payloads, no-gap/loading/failure states, submit locking, history status
and progress, and stable single-line row actions.

### 7. Wrong vs Correct

#### Wrong

```rust
// Startup backfills every checkpoint gap and the old interval env drives it.
if settings.kline_recovery_enabled {
    kline_recovery::run_loop(state, settings.kline_recovery_interval_seconds, limit).await?;
}

// Historical recovery reuses realtime ingestion and can regress live state.
ingestion.ingest_and_publish_kline(&historical).await?;
update_checkpoint_to(historical.open_time()).await?;
```

```rust
// Latest is not necessarily the version atomically selected by the run row.
SELECT MAX(version) FROM strategy_versions WHERE strategy_id = ?;
```

#### Correct

```rust
// Compatibility env controls only the one-second realtime worker.
if settings.kline_recovery_enabled && mysql_ready && mongo_ready && redis_ready {
    synthetic_market::run_loop(state, 1, settings.kline_recovery_batch_limit).await?;
}

// Manual recovery has only Mongo and deterministic version config as inputs.
execute_manual_synthetic_recovery(&mongo, &active_config, &missing, observed_at).await?;
```

```sql
SELECT versions.version, versions.seed, versions.config_json
FROM strategy_runs runs
JOIN strategy_versions versions
  ON versions.strategy_id = runs.strategy_id
 AND versions.version = runs.active_version
WHERE runs.strategy_id = ?;
```

The correct path makes service restart inert with respect to historical gaps,
binds every writer to the selected configuration version, and keeps manual
history repair isolated from Redis, WebSocket, spot triggering, and checkpoints.
