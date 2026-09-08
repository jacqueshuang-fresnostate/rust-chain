# Local implementation verification — 2026-09-07 06:45 HKT

## Delivered

Versioned default parameters/preview/pause-all and deterministic generation;
pair-scoped locks/epochs, manual/default UTC-minute handoff, persisted minute and
pending close; shared ticker/OHLCV/depth/simulated prints and 5m/15m/1h/4h/1d;
strict seconds default-source capability; Admin editor and mobile provenance.
Precise operator/rollout limitations are in
`docs/superpowers/DEFAULT_MARKET_HANDOFF.md`.

## Evidence

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | pass |
| `cargo check --offline --all-targets` | pass |
| `cargo clippy --offline --all-targets -- -D warnings` | final pass |
| `cargo test --offline --lib` | 358/358; local Wiremock bind permitted on rerun |
| backend_architecture + six synthetic generator/worker/migration suites | 49/49 |
| Admin `default_market::` | final 12/12, real disposable MySQL |
| default_market_ownership | 2/2, real MySQL, business pool size 1 |
| default_market_runtime | final 1/1, real MySQL/MongoDB/Redis |
| default_market_transition | 1/1, 116.23 seconds at real minute boundaries |
| existing market_ingestion | final 4/4, real MySQL/MongoDB/Redis |
| existing seconds capability activation/opening guard | 1/1, real MySQL |
| Web `npm test -- --maxWorkers=2` | 80 files, 625/625 |
| Web final ConfirmAction/resourceConfigs | 79/79 after confirmation copy change |
| Web default editor + ConfirmAction focused suite | 13/13 |
| Web lint/typecheck/build/budget | pass; existing third-party eval/chunk warnings remain non-fatal |
| Mobile `npm run release:gate` | 759/759, both typechecks, PWA/Tauri builds, artifacts/budgets/governance pass |
| task context validation and `git diff --check` | 8+8 context entries, pass |

No skipped database test is represented as a real-service pass above. Local
fixture variables were explicitly supplied. Production credentials and URLs
were not used, and no user orders/wallets were operated.

The three disposable MySQL databases used the `codex_default_market_` /
`codex_default_runtime_` prefixes and have been dropped. Temporary MongoDB
27038 and Redis 16389 were stopped and their data/download directories removed.
The pre-existing MySQL service was not stopped. Diagnostic test logs remain in
the OS temporary directory; no test database or service is left running.

## Regression failures found and resolved

- Same-minute ownership originally relied only on completed checkpoints; now a
  persisted first-minute reservation also fences handoff after partial failure.
- Candidate scan/config-save race fixed by rereading under the common pair lock.
- Pending closure survived retry only for one owner; now already persisted
  adjacent closure can finish under the next owner, without historical gap fill.
- Dedicated lock connections initially consumed the business pool; detached,
  bounded connections now permit a size-1 pool transaction under lock.
- Manual archive originally checked only its strategy history. Global symbol
  monotonicity now blocks replay behind default-source history even after Redis
  loss, while preserving existing error strings and side-effect ordering.
- Admin preview/save now prefer newer archive evidence over an old checkpoint,
  and ignore future/undated checkpoints; aligns with actual runtime bootstrap.
- Seconds default capability rejects errored runs; retry no longer clears the
  error before the whole publish cycle reaches its successful checkpoint.
- Mobile source-contract regexes assumed the old exact chart prop string;
  updated to assert the new source binding without weakening layout fingerprints
  or source-size budgets.
- Runtime decimal assertions compare values rather than differing storage scales.
- Initial lib suite hit sandbox localhost bind denial, not code failures; real
  local-only rerun passed. Clippy issues were fixed rather than suppressed.

## Boundaries

No online configuration, activation, deployment, financial live validation,
historical rewrite, commit, push or task-archive auto-commit. Existing dirty work
was preserved. Manual starts remain authored values; no new automatic anchoring,
live price-gap calculator, global defaults editor, health dashboard or external
source failover is claimed. All-market pause is a generation stop, not order
cancellation/position closure.
