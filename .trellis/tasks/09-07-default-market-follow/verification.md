# Verification — default external-reference follow

## Scope

Local implementation only. Preserved previous uncommitted default-generator,
Admin/mobile/chart and unrelated backend slices. No production login/configuration,
activation, order/wallet/settlement operation, history rewrite, commit or push.
No new migration; 0124 is unchanged by this task. No mobile source changed.

## Checks completed

- Pure `synthetic_follow` 7/7: 1x relative returns, multiplier and limits,
  duplicate evidence, stale independent motion and continuous recovery, regressed
  and conflicting reference watermarks, restored frozen seconds, provider changes,
  observed OHLC and own-volume conservation, next-minute rebase, subsecond cutoff.
- Config 4/4 + reference selector 4/4 + original independent generator 7/7.
- `cargo test --offline --lib --test backend_architecture --test synthetic_market
  --test synthetic_market_details --test synthetic_market_worker
  --test synthetic_market_migration --test synthetic_seconds_settlement_migration`:
  library 358/358 + six integration suites 42/42. Initial sandbox run had 5
  local wiremock bind denials; rerun with local-service permission fully passed,
  no external live business API calls or test changes to mask failures.
- Real isolated MySQL `admin_routes default_market::` 17/17, rerun after final
  runtime fixes: permissions, precise active external reference, immutable config,
  read-state Chinese degradation, 60-minute archived model replay, future/platform
  exclusion, conflicting/stale/missing references, >20,000 row overflow, no writes.
- `cargo fmt --all -- --check`, `cargo check --offline --all-targets`,
  `cargo clippy --offline --all-targets -- -D warnings`: passed after final
  production edits (final runtime-test assertion extension also passed fmt and all-target clippy).
- Web focused defaultMarket 24/24, full 82 files / 639 tests, typecheck,
  whole-package lint, production build and bundle budgets passed. Existing
  lottie direct-eval and large-chunk warnings remain; no budget relaxation.
- Review caught and fixed pending-vs-archived frames, pause-invalidated generation
  provenance, manual takeover of failed-frame OHLC, reference watermark replay,
  Select accessible Chinese labels and misleading shared/fallback parameter groups.
- Trellis context 7+7 and git diff --check passed.

## Runtime integration and cleanup

- Dedicated follow integration initially and after harness adjustment: 2/2 passed
  with all three local-service environment variables set, non-skipped execution.
  Includes injected ticker archive failures, own-price fallback, pause-generation
  recovery and failed default-frame exclusion during manual takeover.
  Final trade/depth/all-five-aggregate assertions also passed 2/2 (40.47s).
- Legacy real-store runtime 1/1 (6.27s), default→manual→default transition 1/1
  (96.46s across actual UTC minute boundaries), ingestion 4/4 (5.71s), no skips.
  Root used three dedicated databases, separate from the follow scenarios.
All random follow fixture databases, the Admin fixture and three legacy regression
MySQL databases were removed after verification. Dedicated Mongo27039/Redis16390
were stopped and downloaded Mongo/runtime data directory removed; no remaining
fixture databases or service listeners. Existing shared local MySQL was not stopped.

## Delivery status

Local review-ready code only. No Git commit/push or deployment. Follow defaults
are configurable in the existing default-market editor after deployment; no
online HIPPO-USDT settings were changed. Mobile source and its previous release
gate were not rerun in this follow-only slice. See DEFAULT_MARKET_HANDOFF.md.
