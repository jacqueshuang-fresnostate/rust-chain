# Backend Gap Audit

## Inspected Areas

- `src/modules/spot/{application,infrastructure,presentation,routes,service}.rs`
- `src/modules/margin/{application,infrastructure,presentation,routes,service}.rs`
- `src/modules/seconds_contract/{application,infrastructure,presentation,routes,service}.rs`
- `src/modules/agent/{application,domain,infrastructure,presentation,routes,service}.rs`
- `src/modules/admin/{application,infrastructure,presentation,repository,routes,service}.rs`
- `src/modules/auth/infrastructure.rs`
- Relevant workers, migrations, PC/Mobile API adapters, and route tests.

## Findings

### Spot

- Single-order create/fill/cancel paths already preserve wallet reservation and idempotency.
- Market feed invokes `execute_triggered_spot_limit_orders_with_hub`, so limit and stop-limit orders have a live trigger path.
- Mobile implements “cancel all” as multiple client-side DELETE requests. A server batch endpoint is missing and cannot provide a single authenticated server-side scope/filter contract.
- Market orders previously fell back to the client reference price when Redis had no ticker, allowing a client-controlled execution price.
- Admin fill could not transition a normal `pending` order to a filled state.
- The user trade-list query was not scoped to the authenticated user.

### Margin

- User actions exist for close-all, cancel, cancel-all, transfers, leverage, and margin mode.
- The prior task intentionally models cancelable orders as positions without an entry price; a separate limit-order book is out of scope.
- Existing route tests cover opening/closing/risk/product administration but do not exercise transfers, settings, or bulk actions.
- Settings can be written but there is no read endpoint, so clients cannot reliably restore persisted defaults after a reload.
- Bulk operations call the idempotent single-position use cases, allowing safe retry after a partial external market-data failure.
- `cross` was accepted as a label while risk, PnL, interest, and liquidation remained isolated per position.
- Liquidation credited the spot wallet even when collateral came from the margin wallet.
- New opens without a ticker produced `entry_price = NULL` rows with no later fill worker.
- Transfer requests had no idempotency key or asset-precision validation.
- Bulk queries had a fixed 100-row cap and stopped at the first failure.

### Seconds Contracts

- `cached_entry_price` returns `Ok(None)` when Redis is unavailable.
- `open_order` then inserts an opened order and debits the wallet with `entry_price = NULL`.
- The settlement worker explicitly rejects such an order and reschedules it indefinitely. Opening must fail before the transaction mutates the wallet.
- Stake and payout calculations were not bounded to the stake asset precision, and active products did not guarantee their pair/assets were active.

### Agents

- `agents` has `level` but no `parent_agent_id`, root id, or path.
- All agent-facing queries use `user_referrals.root_agent_id = current_agent_id`, which is a one-level assumption.
- Registration correctly records the invite-code owner agent as the user's direct agent. This can remain unchanged if agent subtree membership is queried from the `agents` hierarchy.
- Authentication only checks the current agent status; inactive ancestors do not block child agents because ancestry is not represented.
- Admin create currently trusts an arbitrary positive level and cannot validate parent/child depth.

## Recommended Design

- Add parent/root/path fields to `agents`; derive levels server-side and cap at three.
- Treat platform admins as a virtual super-agent instead of inserting a synthetic agent row.
- Resolve an authenticated agent scope from its path and apply it consistently to users, stats, commissions, and team tree queries.
- Preserve direct invite ownership and commission calculation behavior; only visibility changes in this task.
- Reject seconds-contract opening without a valid cached entry ticker.
- Add focused APIs/tests around missing server-side batch and settings-read behavior rather than redesigning trading models.
