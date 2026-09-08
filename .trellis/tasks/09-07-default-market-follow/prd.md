# Default market following an external reference pair

## Goal / confirmed behavior

The user wants default generation to follow BTC (or another configured external
pair). Follow **percentage movement**, not BTC's absolute price. Existing manual
strategy priority and all-market pause remain unchanged. The user explicitly
selected automatic independent oscillation when the reference is missing/stale,
with an Admin warning. Recovery must re-anchor to this pair's current price,
not catch up the full missed move or jump back to its initial price.

## Requirements

- Backwards-compatible independent mode and new follow mode in the existing
  versioned default-generator config. Existing saved configs remain independent.
- Follow configuration: reference_pair_id, multiplier (positive <=3, default1),
  max_move_ratio (positive <=1, default0.05 per minute), stale_after_seconds
  (15..300, default60). Independent volatility/volume/wick parameters remain the
  explicitly configured fallback; follow mode does not copy external volume.
- Only active external pairs can be followed; reject self/strategy/internal and
  missing pair IDs. This avoids reference loops and never silently selects BTC
  by symbol guess or page order. Read committed provider ticks from existing
  MySQL, no new exchange integration or live-network reads.
- Use actual fresh, positive, non-future bitget/htx/coinbase price evidence, not
  24h percent or unproven closed candles. Freeze one reference observation and
  resulting frame before publication; same-second retry/owner change must not
  recompute a different frame from a newer reference event.
- Following uses anchored relative returns and accumulates OHLC from actually
  generated frame prices; apply pair precision, positive price limits and
  per-minute move limits. Separate deterministic quantity schedule keeps candle,
  simulated trades, depth, ticker and derived periods coherent.
- Missing/stale/invalid reference switches to independent oscillation without
  resetting price. Recover to following by re-anchoring at the last accepted own
  price and current reference. Do not backfill outages or create past BTC paths.
- Admin supports reference selection, multiplier/limits/freshness, preview
  explanation, configured-vs-effective mode and fallback reason/time. Preview
  is a replay of available historical reference evidence, not a forecast; if
  reference evidence is absent, clearly show the independent fallback sample.
- Preserve reason/audit/version/dirty guards, minute-boundary config changes,
  synthetic provenance and financial consumer contracts. No mobile price math.

## Implementation plan / ownership

1. Root: follow pure state transition + runtime reference reads/persistence,
   simulation-detail reuse, regression/integration verification and docs.
2. Backend worker: config types/validation + Admin reference validation,
   preview/status response and tests, no runtime edits.
3. Web worker: mode/reference form, read-time following/fallback labels, tests.
4. Reviewer/runtime-test worker: reference evidence, invariant audit, dedicated
   MySQL/Mongo/Redis follow and injected-failure integration tests.

## Acceptance

- [x] Legacy independent config/output unchanged.
- [x] Follow BTC 1% => target 1% at multiplier1 unless explicit limits clamp it;
  multiplier, precision, self/cycle and invalid configuration tests.
- [x] Stale => continuous independent fallback; recovery => continuous re-anchor.
- [x] Same-second frame replay/restart and source handoff do not rewrite prices.
- [x] Shared candles/ticker/depth/prints/periods and explicit provenance hold.
- [x] Admin preview/status/permissions and focused tests pass.
- [x] Closest backend/Web validation and PROGRESS/spec handoff recorded.

## Out of scope

Online configuration/activation, production credentials, external exchange
requests, orders/wallets/settlement operations, deployment, commit/push, historical
rewrites, negative/inverse following, multiple simultaneous reference baskets.
All previous uncommitted slices remain untouched except required additive hooks.

## Verification / delivery

All acceptance checks passed locally; see verification.md. Task remains review
for commit/deployment handoff. No online configuration or activation performed.
