# Financial Reconciliation and Manual Evidence

## Scope

Applies to the focused Admin `financial_reconciliation` application,
infrastructure, presentation, routes and Web workbench. This is a diagnostic
sidecar, not settlement, automatic daily accounting close, backfill, custody
verification, proof of reserves or a solvency calculation.

## Interfaces

Prefix: `/admin/api/v1/financial-reconciliation`.

| Method and Suffix | Permission | Contract |
| --- | --- | --- |
| GET root | `governance.financial.read` | Current asset directory and optional selected report |
| GET `/snapshots` | `governance.financial.read` | Asset-filtered immutable capture summaries |
| POST `/snapshots` | `governance.financial.operate` | Server-calculated manual capture with atomic audit |
| GET `/snapshots/{id}` | `governance.financial.read` | Original saved JSON, never recalculate |
| GET `/snapshots/{id}/follow-ups` | `governance.financial.read` | Append-only follow-up history and latest version |
| POST `/snapshots/{id}/follow-ups` | `governance.financial.operate` | Append owner/deadline/notes with version and audit |

IDs are positive numeric path segments. Exact path and method matching must
agree in backend and Web; unsupported methods/extra segments stay unmapped.
Report/history limits are 1–100 (default 20), offsets 0–100000.
OpenAPI `/openapi.json` and `/api/openapi.json` expose identical contracts.

Capture request: `{asset_id, reason, idempotency_key}`. Client report JSON and
unknown fields are rejected. Follow-up request:
`{expected_version, owner_admin_id, due_at, notes, reason, idempotency_key}`.
Owner/deadline fields are required but nullable; null explicitly clears them.
Reason is trimmed 1–500 characters; notes trimmed 1–2000. The raw key is 1–128
visible ASCII bytes and is never trimmed into a different identity.

## Evidence Contracts

- Current GET retains `REPEATABLE READ`, `WITH CONSISTENT SNAPSHOT, READ ONLY`.
  Directory, counts, report and bounded arrays share this transaction. It never
  writes an audit, locks a financial row, or calls a worker.
- Infrastructure report readers take `&mut MySqlConnection`. Manual capture
  reuses them inside a consistent `READ WRITE` transaction that writes only
  sidecar evidence and standard admin audit. Do not weaken the original GET to
  enable capture writes.
- Keep `coverage=partial` and all limitations. First observed journal time is
  not the beginning of complete coverage; no journal rows does not mean no
  business. A report does not test that every source business has every expected
  journal leg.
- Check each transaction key within one asset separately; opposite bad
  transactions/assets cannot cancel the anomaly count. Context/account net
  movement is not actual custody stock or an opening balance.
- Spot and margin independently compare current wallet buckets to the latest
  ledger ID per user/asset/account. Missing sides are explicit nulls, never
  invented zero. Internal wallets are included and not classified as customer
  liabilities.
- Thirteen independent obligation metrics remain separate. Principal, frozen
  funds, collateral and conditional payouts overlap and must not be totalled
  together, added to wallets or summed across assets. No inferred unrealized PnL,
  future interest, missing payout asset, or not-yet-generated commission.
- Difference and account movement arrays have a 100-row cap with exact total
  counts. Save these arrays and totals unchanged; a saved report is bounded
  evidence, not a full discrepancy export.

## Persistence and Concurrency

Migration `0140_financial_reconciliation_snapshots.sql` creates:

- `financial_reconciliation_snapshots`: asset identity/precision, schema version,
  server capture time, actor, reason, raw key, key/request/report SHA-256 digests
  and JSON evidence. Database triggers reject UPDATE and DELETE.
- `financial_reconciliation_followups`: parent snapshot, monotonically increasing
  version, nullable owner/deadline, notes, actor, reason, server time and raw
  key/key hash/request hash. Triggers reject UPDATE and DELETE. Current ownership
  comes from the latest version, not an in-place editable column.

Evidence is immutable JSON content, not original textual whitespace/key ordering.
Hash a recursively key-sorted JSON value because MySQL normalizes object order.
The digest detects application-level inconsistency; it is not a signature or
protection against a privileged database administrator replacing evidence.

Unique replay scopes are actor + key hash separately for capture and follow-up.
Compare the **raw key and normalized request fingerprint** after locating a
candidate. A different raw key with the same hash must conflict. Same key and
request returns the original response even after live wallets or later follow-ups
change; do not add an audit or capture again. On an insert unique-key race,
rollback and reread on the connection before comparing the original request.

Follow-up uses a fresh repeatable-read transaction. Its first database read
locks the existing parent snapshot by primary key. Only after obtaining that
lock may the first nonlocking consistent read establish the latest follow-up
version. Do not lock an absent child range with `FOR UPDATE`: different parents'
first follow-ups can otherwise deadlock on next-key insertion. All application
follow-up writers use the same parent lock; no business balance rows are locked.
Replay precedes optimistic version rejection. New writes validate an active Admin
owner under a shared identity lock. UTC deadline milliseconds must round-trip
through DATETIME(3) across years 1000–9999; do not use UNIX_TIMESTAMP, which can
return null outside its supported epoch range.

Capture/follow-up and their `financial_reconciliation.capture|followup` audit
record commit together. Audit failure rolls back the sidecar write. Follow-up
notes do not change a financial exception into a resolved financial state.
There is no midnight scheduler or reconstructed historical closing balance.

## Error Matrix

| Condition | Result |
| --- | --- |
| No authentication / missing permission | 401 / 403 on registered routes, no side effects |
| Unknown field, invalid ID/page/reason/key/date/owner | 400; application validation before writes |
| Missing capture or selected asset | 404; never create replacement evidence |
| Raw-key collision or altered request using same key | 409; no additional capture/follow-up/audit |
| Stale expected version with a new key | 409; no write, UI preserves draft |
| Audit failure | Whole sidecar transaction rollback |
| Corrupt saved identity/schema/digest | Fail closed, no live-report fallback |
| Malformed Web report/count/null/version | Contract error, never all-clear or empty evidence |

## Admin Behavior

Show Chinese `人工采集时间`, `采集历史`, and append-only follow-up history.
Never label captures daily close. Show partial coverage, missing custody/history,
internal-wallet ownership uncertainty and detail truncation. Preserve decimal
strings through parsing; formatting is presentation-only.

Use existing ConfirmAction, tables, SideSheet and date controls. Writes require
operate permission, a reason, session ownership and single-flight submission.
Recoverable command intents persist by session/actor/asset/capture. Unknown
outcomes and invalid receipts retain the original key. Confirmation validates
receipt actor, identity, reason, key and follow-up fields/version. A 409 keeps the
draft and original version; reload requires abandoning it, not silently rebasing.
Guard browser/navigation and explicit draft discard.

## Required Regressions

Use only the named local `hardening_reconciliation_test` database and explicitly
set its verified port. Tests reject other database names; never run fixtures
against production. Cover migration runner replay, READ ONLY enforcement,
per-asset/noncancelling evidence, all 13 nonzero obligation metrics, capped
105/100 details, immutable replay after wallet changes, raw-key hash collisions,
same-key altered payloads, concurrent same-key capture/follow-up, old receipt
after later versions, audit rollback, active owner validation, explicit clears,
deadline bounds and unchanged wallets/ledgers/journals.

For independent first follow-ups, hold both parent locks and read both absent
child histories before parallel insertion, plus run two fresh application-level
first follow-ups concurrently. This must pass without retrying a gap-lock
deadlock into an apparent success.

Verify actual HTTP auth/read/operate/revocation, fail-closed permission mapping,
both OpenAPI aliases, strict frontend parsers, unknown-result stable keys,
conflict draft retention, read-only actions, capture/history/follow-up browser
flows and narrow/desktop layouts. Mock-HTTP browser checks are not full-shell
browser-to-real-API E2E and must be labeled accordingly.

## Wrong vs Correct

Wrong: infer full historical coverage from a balanced current journal; sum all
assets/overlapping obligations; reconstruct an old day's closing balance from
current wallets; accept uploaded client evidence; compare hashes without raw
keys; lock an absent follow-up range.

Correct: save one server-observed, per-asset, partial and bounded report at an
explicit manual capture timestamp; replay immutable evidence; append audited
metadata under the parent lock without changing balances or claiming resolution.
