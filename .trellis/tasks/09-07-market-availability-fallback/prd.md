# Default synthetic market generation for platform pairs

## Goal and corrected user intent

For HIPPO-USDT-like `strategy/internal` pairs, generate an ongoing baseline synthetic market even when no manually scheduled strategy exists. A scheduled strategy temporarily overrides baseline generation; baseline resumes after the scenario ends. This is not an external-feed-failover request or a request to leave the chart stopped.

The earlier availability-first proposal misunderstood the primary requirement. Its source audit remains useful, but its delivery priority is superseded by this PRD. The user approved implementation on 2026-09-07. Local code and tests are now in scope; online configuration, deployment, activation, orders, and funds remain out of scope.

## Proposed operating model

Priority: **pair disabled / all-market pause > currently effective manual strategy > enabled default generator**.

| Case | Proposed behavior |
|---|---|
| No manual strategy exists | Run the pair's default generator |
| Strategies exist only as drafts or future schedules | Run default generation until an eligible strategy starts |
| Scheduled strategy becomes effective | Transfer the pair's single writer to the strategy |
| Scheduled strategy ends | Transfer back to default generation using the latest committed price |
| Individual manual strategy is paused | Resume baseline; separate all-market pause stops everything (recommended rule accepted as part of proceeding) |
| Pair disabled or all-market pause set | Stop both generators |
| Missing first price / invalid config / storage or ownership failure | Explicit error and alert; do not invent a bootstrap price or bypass ownership |

A failing active strategy is distinct from no scheduled strategy: recover or transfer through the same explicit ownership policy, never launch a competing default writer simply because a tick is late. External-source pairs retain their existing behavior.

## Default generation requirements

- No compulsory end time and no administrator-created hourly strategy copies. Configurable system defaults with pair-level overrides.
- Bootstrap from the latest valid committed price/checkpoint for this pair. If no accepted price exists, require a positive, precision-valid initial price from an explicitly configured pair/project mapping; do not hard-code 0.1 or borrow a similar symbol's price.
- Generate bounded stochastic price variation, with configurable volatility, per-step limits, lower/upper price limits, and a mean-reversion option. Defaults do not prescribe a guaranteed rise, fall, or outcome.
- The last committed close is the next open; persist seed, sequence, generator version, anchor, and checkpoint so restart and lease transfer preserve reproducibility. Do not reset to the initial price on every restart or hour boundary.
- Create ticker, 1m OHLCV, and labeled simulated depth/trade activity from one coherent underlying event sequence. High/low enclose open/close; volume derives from the same simulated activity. Respect price/quantity precision, positive prices, and noncrossed depth.
- Aggregate 5m/15m/1h/1d from the same authoritative 1m candles. Keep one owner for a pair and generation epoch, including within a minute when source switches.
- No automatic historical rewriting/backfill on process restart. Generate current data only and preserve existing historical-recovery approval rules.
- Simulated depth/trade prints remain explicitly marked as simulated and separate from actual user orders and fills. Display generation must not silently enable new financial use of the source.

## Switching and continuity

- Both directions use a pair-level, versioned ownership transition with a fenced checkpoint. Two generators never publish or overwrite the same authoritative stream concurrently.
- Default-to-manual transition must respect the user-authored manual start-price contract. Preview the difference from the current price; offer an explicit current-price anchoring mode or require a deliberate discontinuity decision. Do not silently edit the saved target/start price or interpolate historical prices.
- Manual-to-default transition resumes from the actual last committed generated price, not the strategy's nominal target or the original default seed price.
- Transfer is idempotent; an old owner that resumes late is rejected before archive/cache/publication side effects. Existing post-archive in-flight effects require a deliberate drain/fencing design.

## Admin and mobile

- Pair configuration gets a separate default-market section: enablement, initial-price source, volatility/range, mean-reversion, simulated liquidity/volume, and system-default versus pair-override indication.
- Show current source as default generation / manual strategy / all-market paused / generator error, along with source version, last update, and planned next switch.
- Distinguish pausing a manual scenario from stopping all pair generation. Preserve reason confirmation, audit, and a preview before activation.
- Mobile continues consuming the shared market contract, shows generated-data provenance, and preserves historical candles. Health checks remain per pair and per channel; an open socket is not evidence of fresh data.

## Financial integration boundary

Existing ingestion can trigger spot/margin orders and archive prices for seconds settlement. Therefore source ownership and price provenance are part of the design, not just chart cosmetics. Default-generator capability must be separately represented in consumers; do not spoof an infinite manual strategy merely to satisfy existing seconds-contract capability SQL. Preserve existing settlement windows and idempotency. Local tests and previews precede any deployment; activation affecting actual orders/settlement remains an administrator handoff.

## Open decision

- Proceed with the recommended distinction: pause manual strategy returns to baseline; pair-wide pause stops both. Do not alter existing online state.
- Initial-price and financial-consumer semantics must be explicit before implementation/activation. No live numeric parameters are selected by this planning slice.

## Suggested implementation slices

1. Define pair-level default configuration, ownership/source model, deterministic baseline generator, checkpoint persistence, and isolated tests.
2. Integrate coordinated strategy/default switching and existing ticker/1m/derived-period/simulated-activity ingestion; test restart, contention, and no historical rewrites.
3. Add Admin configuration/preview/status and client provenance, then review price-consumer capability/freshness contracts before rollout.

## Acceptance criteria for the plan

- [x] Correct the request to always-on default synthetic generation, not external-source failover.
- [x] Define first-price, continuity, pause, provenance, and multi-period data rules.
- [x] Record bounded integration review and verify updated task documents.
- [x] Proceed with the recommended pause distinction under the user's approval.

## Implementation validation scope

No strategy, future start, exact strategy start/end, individual pause versus all-market pause, first-ever generation without seed price, decimals and range boundaries, same-minute transfer, concurrent owners, duplicate/old frames, restart with checkpoint, lost infrastructure, coherent OHLCV/depth/volume, all supported periods, unchanged historical candles, provenance in UI, and no accidental real fills from simulated trade prints. Price-consuming business paths require independent contract and transaction tests before activation.

## Out of scope for this turn

Online configuration/generation/activation, real orders or wallets, historical rewrite, deployment, commit/push, and external-provider failover implementation.

## Research references

- `research/current-behavior-and-plan.md`: verified current behavior; original proposed priority is superseded.

## Bounded integration review

- `src/workers/synthetic_market.rs:639-665`: current scan starts with strategies and excludes a pair with no strategy. Default selection needs a pair-first candidate scan, not a frontend workaround.
- `src/workers/synthetic_market.rs:693-702` and `src/modules/market/infrastructure/adapters/ingestion.rs:580-624`: existing leases and archive fences are strategy-scoped. Pair-scoped arbitration must cover both producers and their downstream acceptance.
- `src/modules/admin/infrastructure/market.rs:1077-1087`: a permanent ordinary strategy would conflict with the overlapping-active-strategy guard. The default configuration must be distinct from scheduled strategies.
- `src/modules/market/synthetic.rs:202-215,255-267,700-711`: existing randomness includes seed/version/time and assumes a finite interval; persist a stable rolling-window model or dedicated incremental generator instead of rebuilding a path from the current time on each tick.
- `src/modules/market/synthetic.rs:313-338,734-744`: current generation does not automatically continue from run.current_price; preserve decimal precision and define explicit first-price and takeover-price contracts.
- `src/workers/synthetic_market.rs:471-495`: same-minute replacement and old-version close suppression require an explicit minute-owner rule. Prefer scheduled handoff at aligned minute boundaries; emergency all-market pause still acts immediately.
- `src/workers/synthetic_market.rs:323-380,520-604` and `src/modules/market/synthetic_realtime.rs:121-123`: reuse existing ticker/1m/activity ingestion and higher-period aggregation rather than independently randomizing each chart period.

## Implementation acceptance

- [x] Default parameters and deterministic generation tested, including restart/minute identity.
- [x] Versioned pair configuration, preview, pause-all, permissions and audited optimistic updates.
- [x] Pair-level single writer and source transitions integrated with current runtime.
- [x] Shared ticker/candle/simulated depth/trades and higher-period aggregation, no automatic backfill.
- [x] Admin configuration and source display with focused UI tests.
- [x] Consumer/source/provenance review, isolated integration validation and release handoff recorded.

Evidence: `research/implementation-verification.md`. Local implementation is in
review for commit/release handoff; production activation remains out of scope.


## Implemented slice and explicit delivery boundaries

The approved baseline is implemented locally. Defaults are code-level parameter
values with explicit pair-level overrides; the existing-pair migration does not
activate anything or create an initial price. There is no separate global
settings editor in this slice. The Admin panel shows a read-time runtime
snapshot, not a live health dashboard or next-switch predictor.

Manual activation retains the saved start/target contract and now explicitly
warns of discontinuity, requires a reason-confirmation, and asks the operator to
compare current price and preview. It does not add automatic current-price
anchoring or a computed live price-gap preview. These are explicit UI extensions,
not hidden changes to the authored strategy. Platform symbols must not overlap
external provider routes; the pair lock coordinates the two synthetic sources.

Already-persisted adjacent pending closures may be completed by a new lock
owner during the current minute. No process restart derives new past candles
or scans gaps. All-market pause drains generation, not existing orders or funds.

See `docs/superpowers/DEFAULT_MARKET_HANDOFF.md` for the precise operator and
rollout contract. Online activation, deployment and financial live validation
remain outside this turn. The worktree contains older uncommitted slices which
are deliberately preserved; no archive auto-commit or Git push is performed.
