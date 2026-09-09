# Business Governance Audit Contracts

## 1. Scope / Trigger

This contract applies when changing financial idempotency checks, settlement
queue visibility, market-feed health or K-line recovery monitoring, new-coin
distribution/refund reconciliation, Admin permission mapping/audit display,
or the CI dependency and publication gate. It covers the read paths and
operational guardrails that tie those areas together; it does not replace the
write-side order, wallet, lifecycle, or settlement contracts.

The governance views are diagnostic and fail closed. Opening or refreshing a
view must not settle an order, distribute a token, refund a user, write a
wallet ledger, alter a market price, or silently grant a permission. A write
operation discovered by an audit remains the responsibility of its owning
workflow and must retain its own idempotency key, transaction, and audit
record.

## 2. Signatures

### Financial idempotency audit

`GET /admin/api/v1/governance/financial-idempotency` requires an authenticated
Admin session with `governance.financial.read` and returns:

```json
{
  "status": "balanced|attention",
  "duplicate_idempotency_groups": 0,
  "missing_idempotency_keys": 0,
  "orphan_ledger_entries": 0,
  "expired_seconds_orders": 0,
  "expired_prediction_orders": 0,
  "pending_loan_orders": 0,
  "pending_new_coin_subscriptions": 0,
  "anomaly_count": 0,
  "anomalies": [],
  "checked_at": 1778400000000
}
```

Counts are non-negative integers. The first five integrity/settlement counts
produce `anomalies`; pending loan and manual new-coin queues are informational
and do not by themselves make the status `attention`. The query is read-only
and uses the MySQL schema's actual ledger reference conventions, including
composite spot-trade references.

### Market health and K-line recovery

`GET /admin/api/v1/market-feed/status` returns the saved subscription config,
runtime snapshot, and `health`:

```text
health.status = healthy | stale | degraded | not_configured
health.healthy = (health.status == healthy)
health.stale_symbols = normalized configured symbols without a recent upstream event
health.kline_gap_count = active strategy checkpoint-lag count (strategy units)
health.kline_recovery_failed_count = failed strategy/job count
```

The stale threshold is exposed as `stale_after_seconds`; `last_observed_at`
comes from append-only provider events and `last_ingested_at` is a separate
local-write diagnostic. The independent recovery supervisor uses
`KLINE_RECOVERY_ENABLED`, `KLINE_RECOVERY_INTERVAL_SECONDS` (clamped to
`1..=3600`), and `KLINE_RECOVERY_BATCH_LIMIT` (clamped to `1..=100`). It scans
closed 1m gaps only, writes deterministic Mongo history, and performs an
optimistic MySQL checkpoint update. It does not use Redis or publish a live
event.

### New-coin reconciliation

`GET /admin/api/v1/new-coins/:project_id/reconciliation` requires
`new_coin.distributions.read`. `project_id` is a positive decimal path segment.
The response preserves Decimal values as strings and includes supply,
subscription, distribution, wallet-ledger, and manual quote/refund deltas:

```text
project_id, symbol, lifecycle_status
total_supply, reserved_supply, allocated_supply, remaining_supply, supply_delta
subscription_count, pending_manual_count, requested_quantity
subscription_allocated_quantity, distribution_quantity
linked_distribution_quantity, unlinked_distribution_quantity
distribution_ledger_quantity, invalid_subscription_link_count
subscription_distribution_delta, distribution_ledger_delta
manual_quote_amount, manual_frozen_quote_amount, manual_settled_quote_amount
manual_refunded_quote_amount, manual_quote_delta
status, anomaly_count, anomalies, checked_at
```

Distribution links count as valid only when subscription, project, user, base
asset, and (when configured) quote asset identities agree. Unlinked grants and
zero-quantity refund receipts are reported separately; the endpoint never
infers a refund from a receipt count.

### Admin permissions, audit, and CI

Backend `required_admin_permission(method, path)` and the Web permission mirror
must use exact path-segment matching. Dynamic project IDs are positive numeric
segments; malformed paths fall back to `admin.unmapped.read|write`, not to a
neighboring resource. Governance pages use strict DTO parsers, shared Chinese
field/status labels, and preserve raw Decimal/timestamp values for requests
and exports. `GET /admin/api/v1/audit-logs` remains the read-only source for
operator actions and reasons.

The Docker publication workflow must run an `integration-smoke` job with real
MySQL, Redis, and Mongo services, apply migrations, and run the real-dependency
smoke tests before publish jobs or the manifest job. Publish jobs depend on
that gate; pull requests remain read-only. The Dockerfile uses the bundled
BuildKit frontend and does not resolve a remote Docker Hub syntax image.

## 3. Contracts

1. **Read-only audit boundary.** Every governance query is a `SELECT` (or a
   read-only transaction where supported). It must not acquire a business write
   lock, mutate `_sqlx_migrations`, refresh a settlement lease, or call a
   worker. Errors are returned in the shared error envelope and never converted
   into an all-clear response.
2. **Idempotency evidence.** Duplicate groups are counted within the owning
   business scope (for example user plus key where that is the write contract).
   Blank or null keys are anomalies, not deduplicated into one synthetic key.
   Ledger references are checked against the correct table and identifier
   format; an unknown reference type is not silently treated as valid.
3. **Market observation versus recovery.** Provider observation time is the
   freshness authority; a repeated local ingestion timestamp cannot mask an
   upstream outage. A stale or failed health result is informational and does
   not itself change the producer, pair status, or order behavior. Automatic
   recovery is bounded, version-fenced, deterministic, and Mongo-only plus a
   historical MySQL CAS; ordinary realtime/manual K-lines are authoritative and
   win an automatic write race.
4. **K-line continuity.** Only closed UTC-minute slots inside the strategy's
   half-open range are eligible. Current/forming minutes, active-version
   mismatches, incomplete aggregate windows, and newer checkpoint values are
   skipped or recorded as conflicts. Replays converge on `(interval,
   open_time)` and retain automatic provenance; manual writes clear that
   provenance.
5. **New-coin identity and money conservation.** Reconciliation joins by the
   project row and validates project/user/asset/quote identity before adding a
   distribution to the linked total. All monetary and quantity arithmetic uses
   database Decimal expressions and string serialization. A partial allocation
   is compared with its frozen, settled, and refunded quote amounts; no page
   invents a full-allocation assumption.
6. **Permission and presentation parity.** Backend and frontend path matching
   agree for exact prefixes, dynamic IDs, and unmapped routes. The frontend
   rejects missing or malformed governance DTO fields before rendering a
   healthy/balanced state. Chinese labels are presentation-only; submitted
   enum values, raw reasons, IDs, symbols, Decimal text, and API error codes
   remain unchanged.
7. **Release dependency gate.** A migration or service smoke failure is a
   release failure. The API image is not published or promoted when the
   integration-smoke job is red. Tests that need real services opt in through
   `RUN_REAL_DEPENDENCY_SMOKE=1`; ordinary unit tests remain deterministic and
   do not accidentally connect to a developer's local database.

## 4. Validation & Error Matrix

| Condition | Required result |
|---|---|
| Missing/expired Admin session | `401`/`403`; no governance read |
| Missing governance permission | `403 FORBIDDEN`; no query or page action |
| Unknown backend path or malformed project ID | `admin.unmapped.*`; never inherit a neighboring resource |
| Financial audit query/schema dependency fails | `500` diagnostic error; never return `balanced` |
| Negative count, unknown status, missing anomaly, or invalid timestamp in Web DTO | `API_CONTRACT_ERROR`; keep previous snapshot or show failure |
| Provider event absent or older than stale threshold | Include symbol in `stale_symbols`; status `stale` |
| Recovery checkpoint lag or failed recovery job | Status at least `degraded`; expose strategy/job counts |
| Recovery dependency lacks MySQL/Mongo | Do not start that supervisor; log and retry/alert; no Redis fallback |
| Recovery sees current/future minute, old version, or incomplete aggregate | Do not write it; leave a bounded retryable gap/error |
| Ordinary/manual K-line appears during automatic upsert | Preserve ordinary/manual document; report conflict or retry |
| Reconciliation path ID is zero, negative, non-numeric, or mismatched in DTO | Reject route/DTO; no project query or link |
| Distribution subscription/project/user/asset/quote identity differs | Count as invalid/unlinked; never add to linked allocation |
| Reconciliation Decimal or count cannot be parsed | Backend returns error or frontend contract failure; no rounding fallback |
| CI service, migration, smoke, or image build fails | Stop publish/manifest; retain failure logs and non-zero status |
| Docker Hub frontend resolution is unavailable | Build uses bundled frontend; no remote syntax dependency |

## 5. Good / Base / Bad Cases

- **Good:** The financial page shows `attention` for one duplicate key while
  still displaying pending loan work as an informational queue; refreshing it
  leaves orders and ledgers unchanged.
- **Good:** A provider stops sending events. Health marks only its configured
  symbols stale, while the K-line supervisor independently repairs closed
  strategy gaps from the active version and never emits a historical WebSocket
  event.
- **Good:** An operator allocates 60% of a new-coin subscription and refunds
  the difference. Reconciliation shows requested, allocated, linked
  distribution, settled, and refunded amounts separately and balances only
  when the Decimal deltas are zero.
- **Good:** `/new-coins/12/reconciliation` maps to the distribution read scope,
  `/new-coins/12/reconciliation/extra` is unmapped, and the Web page displays
  the same Chinese status/field labels as the backend contract.
- **Good:** CI starts disposable MySQL/Redis/Mongo, runs migrations and the
  opt-in smoke queries, then allows both native image builds and the manifest
  only after the gate succeeds.
- **Base:** A legacy ledger row has a recognized reference type but no matching
  object; it is counted as an orphan and shown as an actionable anomaly, not
  deleted by the audit.
- **Base:** No provider is configured. Health is `not_configured`; a strategy
  recovery failure can still make the snapshot `degraded`.
- **Bad:** Treat a missing financial table as zero rows, sum a distribution by
  subscription ID alone, or infer a full payout from a receipt quantity.
- **Bad:** Let an automatic K-line upsert overwrite a manual candle, use the
  latest strategy version instead of the run's active version, or backfill the
  forming minute.
- **Bad:** Match `/users-export` to `/users`, translate `status=active` into a
  request value, or render malformed DTO data as a healthy page.
- **Bad:** Let publish jobs run when integration smoke is skipped, or make a
  normal unit test depend on a developer's local service.

## 6. Tests Required

Backend tests must cover:

- financial duplicate/blank/orphan/expired queries against a migrated disposable
  MySQL schema, including composite spot-trade references and read-only
  repeatability;
- market health status priority, stale threshold, future timestamps, empty
  configuration, strategy checkpoint lag semantics, automatic recovery bounds,
  active-version selection, closed-minute filtering, CAS races, and ordinary
  versus automatic Mongo provenance;
- new-coin linked/unlinked distribution identity, partial allocation, frozen /
  settled / refunded quote conservation, zero-refund receipts, malformed
  project IDs, and no-write reconciliation behavior;
- exact backend permission prefixes, dynamic-ID fail-closed behavior, audit
  log filters, and route coverage for every governance endpoint;
- `RUN_REAL_DEPENDENCY_SMOKE=1` migration plus financial/reconciliation query
  tests against MySQL, Redis, and Mongo service containers; ordinary tests must
  pass with the variable unset.

Admin tests must cover strict response parsers, stale/degraded/balanced labels,
Chinese field and status output, permission-gated links, refresh/error state
retention, raw Decimal/timestamp preservation, and malformed DTO fail-closed
behavior. Run the complete Admin quality gate from `admin/index.md`.

CI contract tests must assert service health checks, migration completion before
API, smoke-job ordering, publish `needs` dependencies, pull-request read-only
permissions, native architecture routing, and absence of a remote Dockerfile
frontend reference. Run `git diff --check`, Rust format/check/test/clippy, and
the Web typecheck/lint/test/coverage/build/budget gates before release.

## 7. Wrong vs Correct

### Wrong

```rust
// A dashboard read silently repairs an order and treats a failed query as zero.
let count = load_audit_count(&pool).await.unwrap_or_default();
settle_expired_orders(&pool).await?;

// Any candle at the key can be overwritten by an automatic retry.
collection.update_one(base_key, automatic_values).upsert(true).await?;
```

```ts
// Prefix text and presentation labels are confused with permission/value contracts.
if (path.startsWith('/admin/users')) allow('users.read');
request.status = chineseStatusLabel;
```

### Correct

```rust
// Read-only, fail-closed governance and provenance-aware recovery.
let audit = load_admin_financial_idempotency_audit(&pool).await?;
let health = load_admin_market_feed_health_data(&pool, &symbols, &providers, now).await?;
// automatic upsert matches only an automatic provenance or an absent key;
// a unique-key race with an ordinary/manual document is preserved as conflict.
```

```yaml
jobs:
  integration-smoke:
    services: [mysql, redis, mongo]
  publish:
    needs: [integration-smoke, build]
  manifest:
    needs: [publish-amd64, publish-arm64]
```

The correct implementation keeps diagnostic reads side-effect free, keeps
ordinary business data authoritative, makes every Decimal/identity/permission
boundary explicit, and prevents a release from bypassing real dependency
verification.
