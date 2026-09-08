# Market availability: verified behavior and proposed fallback

> Superseded proposal notice: the user subsequently clarified that platform pairs need an ongoing DEFAULT SYNTHETIC GENERATOR, not external-feed fallback. Section 1 remains a verified code audit; sections 3-7 are the earlier proposal, not the current delivery plan. See `../prd.md` for the corrected requirements.

Planning only, inspected 2026-09-07. No production code or online state was changed. All proposed behavior below is distinct from the implementation currently present in the repository.

## 1. Verified current behavior

### Generation and storage

- `src/workers/synthetic_market.rs:639-665`: eligibility requires an active pair with strategy/internal source, active strategy, runnable live run, matching version, and `start <= now < end`. Missing configuration/run/version, pause, future start, and expiration stop new synthetic data. Expiration filters work; it does not automatically change the stored strategy status.
- `src/modules/market/infrastructure/cache.rs:420-437,493-530` and `src/modules/market/infrastructure/cache/synthetic.rs:32-36,81-98`: retained ticker, depth, and synthetic trades have no automatic TTL cleanup. Stopping generation does not clear them.
- `src/modules/market/application.rs:76-161` and `src/modules/market/infrastructure/persistence.rs:217-244`: public ticker/depth serve retained snapshots without age checks. Missing ticker/depth return NotFound; missing simulated trades/history return empty collections. Existing history is preserved, not regenerated.
- `src/modules/market/service.rs:60-68`: outdated higher-period forming candles are not merged into current history.
- `src/main.rs:59-92` and `src/workers/market_feed.rs:961-978`: external providers use separate explicit subscription/environment configuration, not automatic strategy fallback. Same-symbol external subscription is not categorically excluded by market type; future source arbitration must prevent competing authoritative writers.

### Financial consumers

- `src/modules/spot/infrastructure/market_prices.rs:18-54`: positive Redis ticker, at most 60 seconds old; missing returns None while stale returns an error. This helper does not itself inspect strategy lifecycle or reject future observations.
- `src/modules/spot/application/order_creation.rs:221-288`: missing price rejects market execution, whereas limit/stop-limit can remain untriggered. Stale-price errors propagate; do not claim every missing-price order type is already blocked identically.
- `src/modules/margin/infrastructure/market_data.rs:105-127`: Margin uses positive, non-future, at-most-60-second-old Redis prices for opening/closing and risk decisions; that price gate is not a strategy-status gate. `src/workers/margin_liquidation.rs:313-338,535-585,1063-1068`: liquidation rechecks quote age after locking; missing data postpones work, and cross-margin skips the entire shared account if any position lacks a valid mark. `src/modules/margin/application/lifecycle.rs:421-451`: cancellation of unfilled orders remains independent of price availability.
- `src/modules/seconds_contract/application.rs:71-106,387-408` and `src/modules/seconds_contract/infrastructure.rs:252-278,709-730`: Seconds entry additionally checks current runnable internal/strategy capability (including lease/version), or supported enabled external feed coverage. Entry ticker age is at most 60 seconds but currently lacks a future-time rejection. The capability check covers now, not the whole future contract settlement window.
- `src/modules/seconds_contract/infrastructure.rs:1069-1105`: existing orders use the first historical tick in `[expires_at, expires_at + 5 seconds)`, not the latest display snapshot. Same-time candidates follow provider/version/ID precedence; any backup-source rollout must preserve a reviewed provenance rule.
- `src/workers/seconds_contract_settlement.rs:35-69,301-309,340-379`: missing settlement evidence retries with a 60-second defer; default review eligibility begins 300 seconds after the 5-second window closes (actual transition depends on scanner timing). `src/modules/seconds_contract/infrastructure.rs:1017-1058`: manual review does not change wallets or automatically refund.
- `src/modules/seconds_contract/service.rs:315-324`: equal entry and settlement prices currently produce a loss for either direction. Repeatedly re-stamping a retained flat price as newly observed ticks would change financial outcomes; display fallback must not do that.
- A stopped producer and a stale cached quote are separate states. A residual quote can remain within its age limit just after pause/expiration. A pause also does not guarantee already-archived, in-flight downstream side effects have drained.

### Mobile

- `mobile/src/api/market.ts:71-90` and `mobile/src/stores/market.ts:27-32`: individual ticker failures can omit pairs from a partially successful market list; total refresh failure can retain the old list.
- `mobile/src/components/MobileMarketChart.vue:81-87`: initial missing history has an empty state; no candles are fabricated.
- `mobile/src/core/marketLifecycle.ts:45-46,102-109,159-167`: the 65-second staleness check is connection-wide, not per pair. Other symbols updating does not prove this pair is current.
- `mobile/src/views/MarketDetailView.vue:117-139,156-165,568-576` and `mobile/src/views/TradeView.vue:369-390`: a received detail frame can keep the page flagged live; visible business outage/source/quote-age status is missing.
- `mobile/src/views/TradeView.vue:278-280,1332-1342,1398-1465`: positive cached price is not enough to prove tradability. Spot final submission rechecks its existing predicate; contract submission needs the same availability/freshness recheck. Backend validation remains authoritative.
- `mobile/src/api/marketDetailStream.ts:198-205,249-284`: a connected/pong-responsive socket is not proof of fresh business data; reconnect currently resubscribes without full REST reconciliation.

## 2. Comparable public designs

Inspected official documentation, not an assertion that the repository already implements these contracts:

- [Coinbase Exchange product modes](https://docs.cdp.coinbase.com/api-reference/exchange-api/rest-api/products/get-all-known-trading-pairs): product metadata distinguishes cancel-only, limit-only, and trading-disabled modes. This supports separating listed/displayable from permitted order actions.
- [Kraken WebSocket status](https://docs.kraken.com/exchange/api-reference/spot-websocket-v2/status): engine status is sent on connection and changes; cancel-only preserves cancellation while stopping new orders and matching. This supports explicit status events rather than treating an open socket as market health.

Inference for this repository: use a backend-owned availability state and action capability contract. Neither document defines suitable numerical outage thresholds or settlement rules for this application; those remain our product-specific design decisions.

## 3. Feasible approaches

1. **Availability protection first (recommended MVP)**: expose missing/stale/paused/ended status, preserve historical display, restrict price-dependent actions, alert administrators. Smallest operational change; it does not create a price where none exists.
2. **Verified alternate source after MVP**: add explicit equivalent-instrument source mappings and coordinated failover. Better continuity where a real eligible source exists, but requires source fencing, price-discontinuity checks, and settlement provenance rules. A similar ticker name alone is not a mapping.
3. **Separate simulation-only default**: an explicitly labeled demonstration feed may run independently of finite scenarios, but must have separate streams/storage and no actual order, liquidation, or settlement side effects. This is not a production executable-price fallback.

## 4. Proposed state and data contract

Keep listing state, source health, operator intent, and trading capabilities separate:

- Availability: `unconfigured`, `scheduled`, `live`, `stale`, `paused`, `ended`, `recovering`.
- Reason/source metadata: exact reason code, source kind/identity, source generation/version, quote observation time, server evaluation time, and last successful ingest time. Never rewrite an old observation time to make it appear fresh.
- Per-channel freshness: ticker, depth, and trades have separate ages. Silence in trade activity alone is not an outage when there genuinely were no trades; evaluate feed health and snapshot age separately. Candle period is not ticker freshness.
- Action capability: server-calculated permissions for new orders, triggered execution, cancellation, position-close pricing, and settlement, with business-specific reasons. The display price can exist while executable price is absent.
- Explicit administrative pause overrides automatic failover; absent/expired configuration can use only a preconfigured, eligible fallback policy. Do not silently re-enable a paused scenario or repeat a finite one-hour window.
- Known scheduled end must be considered before accepting new fixed-duration contracts; ensure supported price coverage through the settlement window, not merely a valid entry price right now.

Time-based states must age even when no frames arrive. A shared evaluation clock, backward/future timestamp bounds, source provenance, and threshold version are required; avoid a status flag that stays live forever in Redis.

## 5. Proposed behavior matrix

| Condition | Display | Execution / recovery |
|---|---|---|
| Valid authoritative source | Current price and source | Existing product rules plus backend capabilities |
| Eligible primary fails, validated backup available | Source-change status; preserve actual observed values | Single writer/source generation; bounded transition checks, then recover |
| No source and never had data | Keep listed pair, show waiting-for-market and `--` | No new price-dependent risk; cancellation/read access remain |
| Old data retained after outage/end | Keep history and last observed price with time and stop-update notice | Last price is display-only; no synthetic ticks to trigger fills or decide settlement |
| Operator pause | Explicit paused reason | No automatic restart or failover against operator intent |
| Recovering source | Recovery notice and authoritative snapshot refresh | Resume only after source/age/sequence checks; avoid rapid toggling |
| Existing seconds order has missing settlement evidence | Pending evidence/manual-review status | Preserve original expiry and eligible historical window; no invented close price or silent refund rule change |

For order books, label or hide stale executable depth. Preserve historical trades with original timestamps; do not generate fake activity to make the page appear active. Missing candle intervals stay gaps rather than fabricated flat candles.

Position closing and liquidation require an eligible valuation source; blindly freezing them forever is not an incident policy. Raise an operational incident with affected positions and retain existing contractual/manual-review procedures. Define that runbook before any automated failover rollout.

## 6. Admin and mobile presentation

- Admin pair detail: configured source, current effective source, availability/reason, last valid update, time until start/end, fallback policy, and affected products. Distinguish saved strategy status from actual runtime health.
- Add planned-end warnings and overlap/coverage checks. Next-strategy scheduling remains explicit; this plan does not enable or reschedule the user's online strategy.
- Audit every policy/source/mode transition, operator pause/resume, and rejected transition. Deduplicate alerts; do not create a background automation in this conversation.
- Mobile/PC retain listed instruments even without ticker, expose concise Chinese state labels, stop indicating stale data as live, preserve real history, and disable incompatible actions with a reason.
- Reconcile the current pair's snapshots on reconnect/resume with bounded retry and response ownership. Check availability at button, confirmation preview, and final submit; the backend independently rechecks.

## 7. Staged delivery and tests (not implemented)

1. **Read model and observability**: source-backed availability evaluator, per-pair API/status event, Admin/Mobile/PC labels, exact timestamps. Keep old API fields additive.
2. **Execution protections**: shared quote provenance/freshness gate plus product-specific rules, lifecycle/end-coverage checks, cancellation preservation, idempotent existing-order replays, and an incident/manual-review runbook.
3. **Optional backup source**: explicit mapping and eligibility, fenced ownership, non-overwriting event timestamps, controlled recovery, audit and alerts. Stage behind disabled-by-default configuration and test off production first.

Tests: never configured; missing version/run; future start; exact end boundary; operator pause; age threshold boundaries and future clocks; partial channel outage; another symbol still live; malformed cache/dependency failures; concurrent old/new source writers; duplicate and out-of-order frames; big source price gap; reconnect during confirmation; cancellation and replay during outage; fixed contract crossing planned end; missing settlement snapshot reaching review; multi-timeframe history and no fabricated candles. Validate decimal precision and funds/idempotency invariants with isolated infrastructure before changing execution rules.

Deployment/rollback: additive schema/DTO first, migrate every price consumer before enabling policy, keep failover disabled until audited. Rollback must not reactivate stale-price execution or mislabel retained data. No history or settled-order rewrites.
