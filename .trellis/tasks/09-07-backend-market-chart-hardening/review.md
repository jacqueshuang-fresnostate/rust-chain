# Backend and Mobile Market Chart Review

## Delivered scope

1. **Risk configuration actually executable**: shared known-field validation on
   create and locked enable; legacy disable remains possible. Numeric Admin
   pair rules match exact canonical spot identities alongside legacy symbols.
   Successful order replay and financial/audit transaction boundaries remain.
2. **Fallback windows stay current**: Coinbase K-line start/end refresh at each
   real HTTP dispatch. Other providers/channels, generation fences and failure
   isolation are unchanged.
3. **Mobile chart and data lifecycle**: independent REST channel settlement;
   symbol-owned book/trades survive interval changes, stale candles do not;
   live-authoritative merges and individual loading indicators remain correct.
   Explicit first-history hydration respects user gestures. Bounded/batch updates
   follow only the visible latest tail while historical browsing stays anchored.
4. **Chinese errors**: seven exact new risk validator messages are translated
   through the existing Admin error mapper, with identifiers and bounds retained.

Default 1m, hidden wicks, original OHLCV, current-price line, MA/volume, initial
all-history density, resize policy and attribution remain unchanged. No schema
migration, dependency, production configuration, market-history or funds edit.

## Verification

| Check | Actual result |
|---|---|
| Rust library | 348/348 |
| Rust architecture + documentation | 11/11 + 1/1 |
| fmt, all-target/all-feature check, Clippy `-D warnings` | Pass, offline dependencies |
| Market dispatch/provider focused suites | 5 + 33 + 5 pass; original dispatch regression red first |
| Real MySQL risk regressions | 5/5 after fixture-only raw DDL correction |
| Existing Admin risk CRUD + spot price-deviation with isolated Redis | 1/1 + 1/1, not skipped |
| Admin error mapper | 22/22, including 7 new initially-red cases; typecheck/lint pass |
| Initial Mobile release gate | 728/728 + types/builds/artifact/bundle/governance pass |
| Final Mobile release gate after clearing prior-period candles | 730/730; types, PWA/Tauri web builds, artifacts, bundle/source/test-quality budgets pass |
| Real-browser chart check | Actual bundled chart at 320/390/448px; hydration, tail/history viewport and real wheel/drag checks pass; see research/chart-browser-verification.md |

The final period-clear test's first typecheck exposed TypeScript narrowing an
empty-array equality assertion to never[]; checking length preserves the exact
runtime assertion without unsafe casts. Earlier Mobile failures were stale source
patterns/template fingerprints, narrowly updated without lowering any gate. The
original fingerprint remains protected through one explicit binding normalization.

## Review and prior-work preservation

- Renderer worker independently reviewed actual view-loader lifecycles: six
  additional deferred-request scenarios pass. Root reviewed risk/spot identity,
  status locks, replay order and provider dispatch paths.
- A SHA-256 inventory of 51 prior dirty files exists in research/baseline-files.json.
  Only explicitly shared files changed in this slice; prior financial/resource
  work and the exact wick setting/test remain. No prior task file was rewritten.
- Current task and previous task share Rust tests, Mobile renderer/specs and the
  Admin error mapper. The previous commit plan is not a blind staging recipe for
  this expanded working tree; any future split needs reviewed hunks or a revised
  combined manifest. No commit/push/archive/journal auto-commit has occurred.
- Real-test MySQL schema codex_backend_chart_20260907_0040 was dropped; trigger
  count before removal and matching schema count after removal were both 0.
  Dedicated no-save Redis on 16385 was stopped. Browser cleanup is recorded in
  its own report. Existing user services were not touched.

## Deferred coherent work (not claimed fixed)

- External historical REST rows currently share latest-cache CAS with live data,
  which can leave Mongo history gaps; rejected-stale ingestion can still be
  published/count as ingested. A correct repair requires explicit per-slot
  freshness and typed ingestion outcomes across storage and publication, not
  dropping the latest-CAS guard or merely sorting a batch. Preserve strategy
  generation/recovery leases and avoid old frames triggering financial paths.
- Provider interval capabilities (including legacy 4h mappings) need a separately
  verified mapping/rejection contract; the Coinbase time-window fix does not
  correct them or claim to explain default-provider strategy chart anomalies.
- Withdrawal's missing Redis rate-counter context, numeric-only legacy symbol
  namespace ambiguity and wider settings/approval concurrency debt remain
  recorded; this slice does not claim universal risk enforcement.

Deployment/live backend acceptance, full Admin multi-page UI testing and native
Android/iOS binaries were not run. Full Admin tests/builds from the prior slice
are not presented as newly rerun here; this slice changed only its error mapper.

## Final verification addendum

Final Mobile gate exited 0 on the exact period-clear implementation: 730 tests,
0 failures. TradeView remains within the unchanged budget at 6131/6131 lines and
179598/179654 bytes. Root's real-DB suites and final Rust fmt/lib/architecture/
documentation/check/Clippy all exited 0. Trellis context 10+10 and diff checks pass.
Full browser route/production data acceptance and native binaries remain untested;
the fixture exercises the actual chart package/components, not backend market data.
