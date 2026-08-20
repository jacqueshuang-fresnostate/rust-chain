# Research: Cross-margin hedged liquidation-price contract

- Query: Trace current cross-margin risk and estimated-liquidation-price data from backend domain/query DTOs to the mobile adapter/display; explain the current null/account-level copy; derive an account-safe threshold for shared-wallet long/short portfolios; identify the smallest compatible API/UI change and minimum tests.
- Scope: mixed (local code/specs plus official exchange references)
- Date: 2026-08-20

## Findings

### Conclusion

The current `null -> “账户级风控 / Account-level risk”` result is intentional, not a serialization failure: the backend only solves an isolated-position price, and mobile explicitly discards any price for `cross`. However, the current per-position risk endpoint also reports **single-leg** `equity`, `maintenance_margin`, `margin_ratio`, and `should_liquidate` for a cross position, while the worker liquidates by the whole `(user_id, margin_asset)` account. A cross account has one exact liquidation condition—account equity `<=` aggregate maintenance margin—but no universal scalar liquidation price unless a mark-price path is stated.

The smallest compatible repair is to keep existing isolated fields unchanged and add an optional `cross_account_risk` object to the existing position-risk response. It should carry the live account threshold and an explicitly conditional reference-pair price. Mobile should render that server value only when stable; exact/near-zero net delta must render account coverage and “no stable single price”, never infinity, zero, or a single-leg fallback.

### Files found

| File | Description |
| --- | --- |
| `src/modules/margin/domain.rs` | Pure isolated display metrics and aggregate cross-account equity/maintenance calculation. |
| `src/modules/margin/application/queries.rs` | Builds `/margin/positions/{id}/risk`; currently loads and values only the requested position. |
| `src/modules/margin/infrastructure/position_queries.rs` | SQL read models for one risk position and persisted `margin_cross_accounts` snapshots. |
| `src/modules/margin/presentation.rs` | HTTP DTOs for wallet cross accounts and position risk. |
| `src/modules/margin/routes.rs` | Existing authenticated wallet and position-risk routes. |
| `src/modules/margin/infrastructure/market_data.rs` | Reads one positive, at-most-60-second-old shared ticker `last_price`. |
| `src/workers/margin_liquidation.rs` | Authoritative isolated/cross trigger and cross-account settlement workflow. |
| `src/workers/margin_interest.rs` | Accrues per-position interest and updates the persisted account interest snapshot. |
| `migrations/0086_cross_margin_accounts.sql` | Stores last account equity, PnL, interest, maintenance, ratio, risk time, and version. |
| `mobile/src/api/trading.ts` | Maps both `cross_accounts[]` and per-position risk DTOs into camel-case numbers. |
| `mobile/src/core/marginRiskMetrics.ts` | Forces every cross-position estimated liquidation price to `null`. |
| `mobile/src/views/TradeView.vue` | Polls per-position risk, drops wallet `crossAccounts`, and displays localized account-level copy. |
| `mobile/tests/margin-risk-metrics.test.ts` | Explicitly locks in null cross prices and absence of cross-account consumption. |
| `tests/unit_src/src_modules_margin_domain_tests.rs` | Covers aggregate arithmetic/equality trigger but not hedged price geometry. |
| `tests/margin_liquidation_worker.rs` | Covers one two-leg cross liquidation and single account settlement. |
| `tests/margin_routes.rs` | Covers isolated risk DTO; no live cross-risk route case was found. |
| `.trellis/tasks/archive/2026-08/08-20-mobile-margin-risk-metrics-display/prd.md` | Previous requirement deliberately chose account-level copy for cross mode. |

### Current end-to-end flow and root cause

1. **Actual liquidation worker is account-scoped.** It selects distinct `(user_id, margin_asset)` accounts, expands every filled/open cross position, reads wallet `available`, sums position margin/PnL/interest/maintenance, and liquidates all positions together (`src/workers/margin_liquidation.rs:329-364`, `src/workers/margin_liquidation.rs:689-766`).
2. **The HTTP risk query is position-scoped.** It loads only the requested row, fetches only that symbol ticker, invokes the single-position worker formula, and maps those values directly to the response (`src/modules/margin/application/queries.rs:158-192`, `src/modules/margin/application/queries.rs:194-218`; SQL at `src/modules/margin/infrastructure/position_queries.rs:52-73`).
3. **The domain deliberately nulls cross price.** `estimated_liquidation_price` is calculated only when `margin_mode == "isolated"`; every cross input reaches the `None` branch (`src/modules/margin/domain.rs:174-188`). Distance is derived from that optional price and is therefore also null (`src/modules/margin/domain.rs:189-196`).
4. **The transport contract says the same.** The DTO documents the field as isolated-only (`src/modules/margin/presentation.rs:413-421`), and the backend spec requires cross null (`.trellis/spec/backend/margin-trading-actions.md:45-50`, `.trellis/spec/backend/margin-trading-actions.md:99-100`).
5. **Mobile enforces null a second time.** `resolveMarginPositionRiskMetrics` returns `{ estimatedLiquidationPrice: null, liquidationRiskScope: 'account' }` before considering the server field (`mobile/src/core/marginRiskMetrics.ts:71-105`). `TradeView` converts that scope into localized copy (`mobile/src/views/TradeView.vue:909-915`).
6. **Account data exists but is not the display source.** `/margin/wallets` maps `cross_accounts[]` (`mobile/src/api/trading.ts:264-290`), but `TradeView` retains only `wallets` and `positions` (`mobile/src/views/TradeView.vue:533-550`). The test currently forbids `marginCrossAccounts`/`MarginCrossAccount` use (`mobile/tests/margin-risk-metrics.test.ts:156-176`).
7. **Persisted cross-account rows are unsuitable as a live price source.** They are last-worker snapshots (`src/modules/margin/infrastructure/position_queries.rs:138-160`), the HTTP projection omits existing `last_risk_at` and `version` columns (`migrations/0086_cross_margin_accounts.sql:7-15`), and a newly created account starts with zero snapshots until the worker evaluates it (`src/modules/margin/infrastructure/positions.rs:389-406`).

Consequently, the null/copy is intentional legacy behavior, but the cross branch of the position-risk response is internally misleading: per-leg PnL/quantity/return are valid, while its `equity`, `maintenance_margin`, `margin_ratio`, and `should_liquidate` are not the values used for cross liquidation.

### Authoritative current-engine accounting

For every filled/open cross position `i` in one `(user_id, margin_asset)` account, define:

```text
s_i = +1 for long, -1 for short
E_i = positive entry price
P_i = current server risk mark
N_i = stored notional_amount
q_i = N_i / E_i                         # base quantity
C_i = stored margin_amount
I_i = accrued interest_amount
m_i = current product maintenance_margin_rate
U_i(P_i) = s_i * q_i * (P_i - E_i)     # unrealized PnL
MM_i = N_i * m_i                        # current engine: static entry notional

W = current margin_wallet_accounts.available
C = sum(C_i)
I = sum(I_i)
U = sum(U_i)
MM = sum(MM_i)
Equity = W + C + U - I
Buffer = Equity - MM
Coverage = Equity / MM                  # null when MM == 0
Liquidate iff positions are non-empty and Buffer <= 0
```

This is exactly the current worker/domain model: single-leg PnL is `N * price_delta / entry`, maintenance is `N * rate` (`src/workers/margin_liquidation.rs:204-242`), and account equity is wallet available plus occupied margins plus aggregate PnL less interest (`src/modules/margin/domain.rs:242-283`). `Coverage` is `equity / maintenance`, so safe values are above `1`; do not silently invert it into another venue's MMR convention.

Important accounting consequences:

- Long and short PnL offset through signed quantity, but current maintenance margin remains the **gross sum of both legs**; the local engine has no hedge-maintenance offset.
- Interest reduces equity one-for-one. At fixed marks, the remaining additional-interest capacity is exactly `max(Buffer, 0)`.
- Pending limits are excluded as positions, but their already-debited collateral reduces `W`; isolated allocations and outbound transfers can likewise move the cross threshold without a mark move.
- The worker uses current product maintenance rates, so an admin rate change moves the threshold immediately.

### Executable account-level reference-pair price

A scalar price requires a scenario. The least ambiguous position-card scenario is:

> Change the requested pair's one shared mark; hold every other pair mark, wallet balance, positions, rates, and accrued interest fixed.

All same-pair legs must use one mark. For reference pair `g`:

```text
P0 = current shared mark for pair g
D_g = sum(s_i * q_i) for all account positions whose pair_id == g
Gross_g = sum(q_i) for those positions
rho_g = abs(D_g) / Gross_g
P_star = P0 - Buffer / D_g
```

`D_g` is the account-equity change per one quote-currency unit of that pair's mark. `P_star` solves `Equity(P_star) == MM` under the current static-maintenance engine. It must be computed with decimal arithmetic on the backend, never with JavaScript `number`.

Recommended executable result rules:

```text
if Buffer <= 0:
    price = null; status = "already_liquidatable"
else if D_g == 0:
    price = null; status = "net_delta_zero"
else if Gross_g <= 0:
    price = null; status = "invalid_exposure"
else if rho_g <= CROSS_NET_DELTA_EPSILON:
    price = null; status = "net_delta_near_zero"
else:
    exact = P0 - Buffer / D_g
    if exact <= 0:
        price = null; status = "no_positive_boundary"
    else if D_g > 0 and exact >= P0:
        price = null; status = "wrong_adverse_direction"
    else if D_g < 0 and exact <= P0:
        price = null; status = "wrong_adverse_direction"
    else:
        # Conservative display rounding to the pair tick:
        price = ceil_to_tick(exact) if D_g > 0 else floor_to_tick(exact)
        status = "estimated"
        distance_rate = abs(price - P0) / P0
```

`CROSS_NET_DELTA_EPSILON` is a **display-stability policy**, not a liquidation trigger. A concrete first contract can use decimal `0.000001` (one part per million of same-pair gross quantity), must name and test that constant, and must not apply it inside the actual `Buffer <= 0` trigger. Near-zero estimates are unstable because:

```text
d(P_star) / d(Buffer) = -1 / D_g
```

Thus a one-unit wallet/interest/MM change shifts the estimate by `1 / abs(D_g)` price units. Returning an enormous number for a nearly hedged book is less truthful than returning `net_delta_near_zero` plus live account coverage.

#### Zero-net-delta boundary

For equal same-symbol long/short quantity, `D_g == 0`. Even with different entry prices, aggregate PnL is constant with respect to that shared mark. Under the current static-MM model:

- `Buffer > 0`: no finite liquidation price exists along that pair-price axis.
- `Buffer == 0`: the account is already on the liquidation boundary at every positive pair price.
- `Buffer < 0`: the account is already liquidatable, independent of that pair price.
- Interest can still liquidate an exactly hedged account. If current buffer is `H > 0` and aggregate incremental interest is `J`, equality occurs at `J = H`; with a stable aggregate hourly accrual `R > 0`, the rough time is `H / R` hours, subject to the worker's discrete accrual schedule and future rate changes.

Do not label `net_delta_zero` as “safe”: another pair, wallet transfer, new order, interest, or maintenance-rate change can still consume the account buffer. Cross-asset dollar neutrality is also not a perfect hedge because relative/basis moves remain.

#### Uniform multi-pair shock (optional account stress metric)

If every mark is explicitly assumed to move by the same multiplier `x`, define quote-valued signed exposure:

```text
Delta_value = sum(s_i * q_i * P_i0)
x_star = 1 - Buffer / Delta_value
P_i_star = x_star * P_i0
```

This is a scenario shock ratio, not a universal liquidation price. `Delta_value == 0` means no boundary under that common-shock path, not no basis risk. It should be exposed as a separately named stress metric if ever used.

#### Do not mix in mark-based maintenance without changing the worker

If maintenance were later changed to `MM_i(P) = m_i * q_i * P`, the selected-pair buffer slope would become `D_g - A_g`, where `A_g = sum(m_i * q_i)`, and the formula would be:

```text
P_star_mark_MM = P0 - Buffer / (D_g - A_g)
```

That formula is **not** compatible with today's worker, which uses stored entry notional. Risk tiers, maintenance deductions, and close/liquidation fees would make the function piecewise and require boundary search using the exact worker evaluator.

### Mark-price assumptions and accounting hazards

1. The code calls the risk input a mark price, but it is the shared cache's `last_price` (`src/modules/margin/infrastructure/market_data.rs:17-32`, `src/workers/margin_liquidation.rs:166-171`). The default Bitget feed subscribes to `instType: "SPOT"` and its spot ticker (`src/modules/market/infrastructure/adapters/provider.rs:253-269`); the REST fallback is also `/api/v2/spot/market/tickers` (`src/modules/market/infrastructure/adapters/provider.rs:163-173`). This is not an independent derivatives fair/mark-price feed.
2. Any new estimate must use the same source as the worker until both are migrated together; otherwise UI and actual liquidation will disagree.
3. The worker currently fetches by **position id** and inserts each result separately (`src/workers/margin_liquidation.rs:339-347`). Two opposite positions on the same symbol can therefore observe different Redis updates, fabricating PnL in a perfect hedge. The prerequisite accounting fix is to read each unique pair/symbol once and reuse that exact price for every same-pair leg.
4. Different symbols are not a truly simultaneous snapshot. The API should return `marks_observed_at_min`, `marks_observed_at_max`, and reject or mark unavailable a snapshot exceeding an explicit skew limit. The current worker discards ticker observation times after freshness validation (`src/workers/margin_liquidation.rs:538-555`).
5. The current 60-second freshness gate and positive-price check must remain. A missing/stale mark for any account leg must make the complete account estimate unavailable; silently omitting one leg overstates safety.

### Smallest compatible API/UI change

Keep the route and existing isolated contract:

```text
GET /api/v1/margin/positions/{id}/risk
```

Add only an optional object (absent for isolated and for older servers):

```json
{
  "risk": {
    "...existing_fields": "unchanged",
    "liquidation_risk_scope": "cross_account",
    "cross_account_risk": {
      "margin_asset": 1,
      "reference_pair_id": 10,
      "price_assumption": "reference_pair_only_other_marks_static",
      "equity": "...",
      "maintenance_margin": "...",
      "liquidation_buffer": "...",
      "margin_ratio": "... or null",
      "unrealized_pnl": "...",
      "interest_amount": "...",
      "should_liquidate": false,
      "net_quantity": "...",
      "gross_quantity": "...",
      "estimate_status": "estimated|already_liquidatable|net_delta_zero|net_delta_near_zero|no_positive_boundary|mark_unavailable",
      "conditional_liquidation_price": "... or null",
      "conditional_liquidation_distance_rate": "... or null",
      "marks_observed_at_min": 0,
      "marks_observed_at_max": 0
    }
  }
}
```

Compatibility rules:

- Preserve existing top-level `estimated_liquidation_price` and `liquidation_distance_rate` as isolated-only fields; do not silently change their meaning.
- Preserve top-level per-position `unrealized_pnl`, quantity, return, and maintenance rate.
- Mobile must treat `cross_account_risk` as authoritative for account equity/maintenance/trigger and must not calculate cross risk locally.
- When `estimate_status == "estimated"`, dynamically label the row as localized “账户预估强平价” / “Est. account liquidation” and expose the “other marks unchanged” assumption in help text.
- For zero/near-zero/no-positive-boundary states, show account coverage or buffer plus localized “无稳定单一价格”; do not show `--` alone, infinity, zero, or the old single-leg formula.
- If the object is absent (old backend) retain the existing localized account-level fallback, so rollout order is backward compatible.
- No migration or new route is required. A future batch/account endpoint can remove duplicate account valuation if polling cost becomes material.

Affected production files for a later implement agent:

- Backend pure calculation: `src/modules/margin/domain.rs`
- Backend account-position/wallet query: `src/modules/margin/infrastructure/position_queries.rs` and façade export in `src/modules/margin/infrastructure.rs`
- Backend unique-mark read preserving timestamps: `src/modules/margin/infrastructure/market_data.rs`
- Backend use-case mapping: `src/modules/margin/application/queries.rs`
- Backend additive DTO: `src/modules/margin/presentation.rs`
- Worker same-symbol mark deduplication/shared calculation: `src/workers/margin_liquidation.rs`
- Mobile DTO adapter: `mobile/src/api/trading.ts`
- Mobile risk projection: `mobile/src/core/marginRiskMetrics.ts`
- Mobile display: `mobile/src/views/TradeView.vue`
- Mobile localization: `mobile/src/i18n/messages/zh-CN.ts`, `mobile/src/i18n/messages/en.ts`

### Minimum required tests

1. **Domain formula:** partial same-pair hedge with `D_g > 0` and `D_g < 0`; assert the solved price reaches `Equity == MM`, one adverse tick is liquidatable, and one favorable tick is safe.
2. **Zero/near-zero:** exact equal long/short quantity returns `net_delta_zero`; equality buffer is already liquidatable; the named epsilon boundary returns `net_delta_near_zero`; no result is infinity/zero.
3. **Interest and gross maintenance:** equal hedge remains price-invariant while gross maintenance includes both legs; increasing interest by exactly `Buffer` reaches liquidation equality.
4. **Worker coherence:** same-symbol opposite legs use one identical cached mark, settle the account once, and cannot fabricate PnL from two sequential ticks.
5. **Route/account scope:** cross response includes every filled/open same-user/same-margin-asset leg, excludes pending/isolated/closed/other-user/other-asset rows, and fails the complete account estimate when any required mark is stale or absent.
6. **Compatibility:** isolated risk JSON and formula remain byte-shape compatible; old cross top-level fields remain present; unauthorized/other-user reads remain rejected; all decimals remain strings.
7. **Mobile adapter/UI:** new object maps strictly; old response falls back safely; stable estimate displays with account label/assumption; zero/near-zero displays account coverage and no single price; cross never invokes isolated local fallback.

Existing tests that intentionally encode the old behavior and must be revised, not merely supplemented:

- `tests/unit_src/src_modules_margin_domain_tests.rs:104-131`
- `mobile/tests/margin-risk-metrics.test.ts:102-113`
- `mobile/tests/margin-risk-metrics.test.ts:156-176`
- `.trellis/spec/backend/margin-trading-actions.md:47,99`
- `.trellis/spec/mobile/backend-integration.md:279-290`

### Code patterns

| Pattern | Evidence |
| --- | --- |
| Cross trigger is aggregate `equity <= maintenance` | `src/modules/margin/domain.rs:265-282` |
| Worker maintenance uses static `notional * rate` | `src/workers/margin_liquidation.rs:232-241`, `src/workers/margin_liquidation.rs:746-762` |
| Cross open collateral always comes from margin wallet | `src/modules/margin/infrastructure/settlement.rs:21-71` |
| Wallet account snapshot is persisted by the worker | `src/workers/margin_liquidation.rs:865-901` |
| Wallet query returns persisted, not live, account values | `src/modules/margin/infrastructure/position_queries.rs:138-160` |
| Risk route computes only one position | `src/modules/margin/application/queries.rs:152-219` |
| Mobile polls each filled position every five seconds | `mobile/src/views/TradeView.vue:568-589`, `mobile/src/views/TradeView.vue:1187-1195` |
| Mobile forcibly substitutes account-level copy | `mobile/src/core/marginRiskMetrics.ts:79-84`, `mobile/src/views/TradeView.vue:909-915` |

### External references

- [Bybit — FAQ: Order Execution and Liquidation](https://www.bybit.com/en/help-center/article/FAQ-Order-Execution-and-Liquidation) (accessed 2026-08-20): cross liquidation is determined by account maintenance risk; any displayed cross price is a dynamic reference, and mark price—not last traded price—is the trigger reference.
- [Bybit — Liquidation Price (USDT Contract)](https://www.bybit.com/en/help-center/article?id=000001067) (accessed 2026-08-20): documents net-position treatment for partial hedges and no finite price liquidation for a perfectly hedged same-symbol position under its stated assumptions.
- [Bybit — The New Margin Calculation: Adjustments and Implications](https://www.bybit.com/en/help-center/article/Understanding-the-Adjustment-and-Impact-of-the-New-Margin-Calculation) (updated 2026-06-18): distinguishes entry-notional and mark-notional maintenance formulas; this supports keeping the formula aligned with the actual engine rather than mixing models.
- [OKX — Product Disclosure Statement, OTC Crypto Derivatives](https://www.okx.com/en-us/help/product-disclosure-statement-otc-crypto-derivatives) (accessed 2026-08-20): describes aggregate cross equity, offsetting unrealized PnL, aggregate maintenance, and account-level equality liquidation. It is contextual only; local code remains the source of truth.

### Related specs

- `.trellis/spec/backend/margin-trading-actions.md`
- `.trellis/spec/backend/quality-guidelines.md`
- `.trellis/spec/backend/directory-structure.md`
- `.trellis/spec/backend/wallet-amount-precision.md`
- `.trellis/spec/mobile/backend-integration.md`
- `.trellis/spec/mobile/pwa-and-shell.md`
- `.trellis/spec/guides/cross-layer-thinking-guide.md`

## Caveats / Not Found

- No task `prd.md` exists yet; only `task.json` and seed context files were present.
- No OpenAPI schema for `MarginRiskSnapshot` or `MarginCrossAccountResponse` was found, so the Rust DTO and route tests are the effective HTTP contract.
- No PC/web consumer of these liquidation fields was found; mobile is the only current display path located.
- The repository has no configured cross-price scenario, mark-snapshot skew limit, near-zero epsilon, maximum reference shock, maintenance tiers/deductions, or liquidation/close-fee term. The proposed epsilon is explicitly a display policy and must be recorded in the eventual contract.
- The current risk “mark” is a spot ticker last price. A true derivatives mark/index migration is separate work and must change worker and API together.
- Existing `/margin/wallets.cross_accounts[]` values may be zero before first worker evaluation or stale afterward and expose no risk timestamp/version; they must not be the sole live liquidation-price source.
- Research only: no production files, specs, migrations, or tests were modified or run.
