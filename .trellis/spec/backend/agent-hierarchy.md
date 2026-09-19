# Agent Hierarchy Contracts

## Scenario: Three-Level Agent Organization And Subtree Scope

### 1. Scope / Trigger

- Trigger: an admin creates an agent, assigns users, changes agent status, or an agent-authenticated route reads team data.
- Platform administrators are a virtual super-agent level 0 and keep the existing Admin RBAC identity.
- Stored agents support exactly three levels: general agent, level-2 agent, and level-3 agent.

### 2. Signatures

- DB fields: `agents.parent_agent_id`, `agents.root_agent_id`, `agents.level`, `agents.path`.
- Admin create: `POST /admin/api/v1/agents`, optional `parent_agent_id` and compatibility assertion `level`.
- Admin subtree users: `GET /admin/api/v1/agents/{id}/users`.
- Agent scope routes: `GET /agent/api/v1/{users,sub-agents,team-tree,dashboard,convert/stats,commissions}`.
- Materialized path segment: `/agent:{id}`.
- Referral fields: `user_referrals.direct_inviter_type`, `direct_inviter_id`, `root_agent_id`, `depth`, `path`.
- Team-user response fields: `owner_agent_id`, compatibility field `root_agent_id`, `direct_inviter_type`, and `direct_inviter_id`.

### 3. Contracts

- Level 1 has no parent; level 2 must have a level-1 parent; level 3 must have a level-2 parent.
- `parent_agent_id`, `root_agent_id`, `level`, and `path` are server-derived. A client-provided `level` is only an assertion.
- Existing agents migrate to independent level-1 roots.
- Authenticated agent scope is its own path, never a client-provided path or just the top-level root ID.
- A candidate belongs to the scope only when its path equals the scope path or starts with `scope_path + '/'`.
- `user_referrals.root_agent_id` continues to identify the directly owning agent. Parent visibility is resolved by joining that owner to `agents.path`.
- Agent organization and user referral are two simultaneous dimensions:
  - `owner_agent_id` identifies the directly owning agent/company.
  - `direct_inviter_type/direct_inviter_id` identifies the concrete agent or user that introduced the user.
- An agent-invited user stores that agent as both owner and direct inviter. A user-invited descendant keeps the inviting user as direct inviter but inherits the inviter's owner agent, depth, and referral path.
- The historical `root_agent_id` API field remains an alias for the directly owning agent. Do not reinterpret it as `agents.root_agent_id`; new consumers should prefer `owner_agent_id`.
- Parent agents can see descendant-owned users; children cannot see parents, siblings, or unrelated trees.
- Online support is the deliberate exception to subtree visibility: an agent
  can list/open only conversations whose `assigned_agent_id` equals that
  authenticated agent's exact ID. Parent reporting access never grants access
  to a child's customer conversation. See
  [Agent-Routed Online Support Contracts](./online-support.md).
- Agent commission routes expose only records owned by the current agent and must not leak descendant payout records.
- Commission rules and tiered payout behavior follow the multi-business commission scenario below.
- Login, refresh, agent routes, agent invite-code registration, and user invite-code registration/binding require the owning agent and every ancestor to be active.

### 4. Validation & Error Matrix

- Parent missing -> `NOT_FOUND`.
- Parent or any ancestor inactive -> `CONFLICT` for admin creation/assignment; agent authentication is rejected.
- Parent level 3 -> `VALIDATION_ERROR`, because a fourth level is forbidden.
- Client `level` differs from server-derived level -> `VALIDATION_ERROR`.
- Uninitialized/malformed parent root or path -> `CONFLICT`.
- Agent attempts to read a parent, sibling, or unrelated user -> the record is absent from the scoped response.
- Unsupported commission product type or a commission record without a payout asset -> `VALIDATION_ERROR`/`NOT_FOUND` before an admin payout ledger is written.
- User inviter is inactive -> user-owned invite code registration/binding returns `VALIDATION_ERROR`.
- Owning agent or any ancestor is inactive -> both agent-owned and user-owned invite codes return `VALIDATION_ERROR`; no user/referral row or usage-count increment is committed.

### 5. Good/Base/Bad Cases

- Good: level 1 sees users owned by levels 1, 2, and 3 in its subtree.
- Good: level-3 agent invites user A, A invites user B; B has `owner_agent_id = level_3_agent_id`, `direct_inviter_type = user`, and `direct_inviter_id = A`.
- Base: level 3 sees only users directly owned by itself because it has no descendants.
- Bad: filtering by only `root_agent_id` lets a child see siblings; always filter through the current agent path.
- Bad: disabling level 2 while level 3 stays locally active must still block the level-3 session.
- Bad: replacing B's owner with A's ID loses company attribution because user IDs and agent IDs belong to different relationship dimensions.

### 6. Tests Required

- Migration test: a fresh database applies the hierarchy migration and historical agents become level-1 roots.
- Domain test: derive levels 1-3 and reject level 4 or a mismatched requested level.
- Admin route test: create all three levels, reject a fourth, and assert parent/root/path values.
- Admin subtree test: root, level 2, and level 3 each return exactly the expected descendant users.
- Agent route test: root/child/grandchild visibility excludes sibling and unrelated users.
- Auth test: suspending a parent blocks child login, refresh, routes, and invite-code registration.
- Referral test: agent -> user A -> user B preserves B's owner agent, records A as B's direct inviter, increments depth/path, and exposes both dimensions in agent/admin responses.
- Referral-status test: suspending an owning agent ancestor blocks both registration-time user invite codes and post-registration referral binding without partial writes.
- Support isolation test: a parent can still see a child-owned user in team
  reporting but gets not-found when opening that child's directly assigned
  support conversation.

### 7. Wrong vs Correct

#### Wrong

```sql
WHERE user_referrals.root_agent_id = :logged_in_agent_id
```

This preserves the old one-level model and hides descendant teams.

#### Correct

```sql
JOIN agents owner_agents ON owner_agents.id = user_referrals.root_agent_id
WHERE owner_agents.path = :scope_path
   OR owner_agents.path LIKE CONCAT(:scope_path, '/%')
```

The server-derived path enforces the current agent subtree without accepting a client scope. This query is for reporting/business subtree routes only; support routes must use exact `assigned_agent_id` equality as documented in the online-support contract.

#### Wrong

```rust
// 把“谁邀请的”误当成“归属哪个公司”，会让用户邀请后脱离原代理公司。
owner_agent_id = direct_inviter_id;
```

#### Correct

```rust
// 用户邀请只改变直属邀请人，公司归属继续继承邀请人的直属代理。
owner_agent_id = inviter.owner_agent_id;
direct_inviter_type = "user";
direct_inviter_id = inviter.user_id;
```

## Scenario: Multi-Business Tiered Differential Commission

### 1. Scope / Trigger

- Trigger: a referred user completes a convertible settlement, opens a prediction order, completes a spot fill, opens a margin position, or opens a seconds-contract order.
- Applies to commission rule management, business transaction writes, agent/admin commission responses, and later admin payout settlement.

### 2. Signatures

- Supported rule values: `convert`, `prediction`, `spot`, `margin`, and `seconds_contract`.
- Rule storage: `agent_commission_rules(agent_id, product_type, commission_rate, status)`.
- Record storage: `agent_commission_records(agent_id, user_id, source_type, source_id, source_amount, payout_asset_id, commission_rate, commission_amount, status)`.
- Record idempotency key: `(agent_id, source_type, source_id)`.
- Shared write entry: `insert_agent_business_commission_in_tx(tx, AgentBusinessCommissionWrite)`.
- API record field `commission_rate` is the actual differential rate allocated to that record, while a rule's `commission_rate` is a cumulative rate.

### 3. Contracts

- Load the latest active rule for each active ancestor, ordered from the directly owning agent toward the level-1 root.
- A rule rate is cumulative, not independently additive. For owner/root rates `5% / 8% / 10%`, actual record rates are `5% / 3% / 2%`.
- Missing rules are skipped. A higher ancestor receives only the positive difference between its cumulative rate and the highest cumulative rate already allocated below it.
- An inverted or repeated cumulative rate creates no negative/zero record and never reduces an amount already allocated.
- Calculate each cumulative amount with `truncate_amount_to_asset_precision(source_amount * cumulative_rate, assets.precision_scale)`, then subtract the previously allocated cumulative amount. This keeps the sum equal to the highest quantized cumulative payout.
- Insert commission records in the same MySQL transaction as the source business funds/order mutation. A source transaction rollback must also remove every level's commission record.
- Invalid prediction refunds must reject still-pending `prediction_order` commission rows in the same settlement transaction. Already-settled commission is not clawed back here.
- Business basis and payout asset mapping:
  - `convert_order`: source amount and source asset.
  - `prediction_order`: stake amount and stake asset.
  - `spot_trade_buy`: quote amount and quote asset.
  - `spot_trade_sell`: filled base quantity and base asset.
  - `margin_position`: opening margin amount and margin asset.
  - `seconds_contract_order`: stake amount and stake asset.
- `user_referrals.root_agent_id` is the compatibility field for the directly owning agent. Resolve the payout chain through that owner's `agents.path`; do not interpret it as only the level-1 root.
- Admin payout uses the snapshotted `payout_asset_id`; it must not rediscover an asset by joining a business-specific order table.

### 4. Validation & Error Matrix

- Unsupported `product_type` -> `VALIDATION_ERROR` when creating or filtering a rule.
- Rule rate below `0` or above `1` -> `VALIDATION_ERROR`.
- Non-positive source amount, missing referral, or no active ancestor rule -> source business succeeds with no commission record.
- Missing/inactive payout asset -> `NOT_FOUND`; the surrounding source business transaction rolls back.
- Inverted ancestor rate -> skip that tier and continue upward; never create a negative payout.
- Replayed source id -> unique-key no-op for every existing agent record; no duplicate commission is created.
- Commission without a stored payout asset -> `CONFLICT` during admin settlement and no wallet ledger is written.

### 5. Good/Base/Bad Cases

- Good: a level-3 owner has 5%, level 2 has 8%, and level 1 has 10%; records allocate 5%, 3%, and 2% in the business payout asset.
- Good: level 2 has no active rule while level 3 has 5% and level 1 has 10%; records allocate 5% and 5%.
- Base: the user has no agent referral or all rules are disabled; the business transaction succeeds without commission.
- Bad: multiplying source amount by every configured rate independently would pay 23% for a 10% maximum chain.
- Bad: quantizing each differential independently can make the record sum differ from the quantized highest cumulative amount.

### 6. Tests Required

- Domain: `5% / 8% / 10%` yields `5% / 3% / 2%`; missing/inverted tiers never overpay; zero source and invalid rates produce no allocation.
- Database integration: a real three-level convert transaction writes three pending records with the expected agent, source, rate, amount, and payout asset.
- Business integration: convert, prediction, spot buy/sell, margin, and seconds-contract flows each assert their documented source amount and payout asset.
- Idempotency: replaying a source transaction leaves exactly one record per `(agent_id, source_type, source_id)`.
- Admin/API: all five product filters are accepted; commission responses and OpenAPI schemas expose actual `commission_rate`; settlement credits the stored payout asset.

### 7. Wrong vs Correct

#### Wrong

```rust
for rule in ancestor_rules {
    let amount = truncate(source_amount * rule.commission_rate, precision);
    insert_commission(rule.agent_id, rule.commission_rate, amount).await?;
}
```

This adds every cumulative rule and overpays the chain.

#### Correct

```rust
let allocations = allocate_differential_agent_commissions(
    &tiers_from_owner_to_root,
    source_amount,
    payout_asset_precision,
);
for allocation in allocations {
    insert_idempotent_commission_in_source_transaction(allocation).await?;
}
```

The shared domain function converts cumulative rates into positive differences and preserves the quantized maximum payout.

## Scenario: Agent-Linked Invite Code And Read-Only User Portfolio

### 1. Scope / Trigger

- Trigger: an agent creates an invite code, its linked user calls `GET /api/v1/referral/my-code`, or an authenticated agent opens a team user's financial detail.
- Financial detail covers existing spot/margin wallet accounts, margin orders, filled margin positions, spot orders, and seconds-contract orders.

### 2. Signatures

- Shared invite generator: `generate_invite_code()`, six characters from `A-Z0-9`, backed by the system secure random source and rejection sampling rather than biased byte modulo.
- Linked identity: `agents.user_id`; clients never choose this association while reading their effective code.
- Effective agent code: latest `invite_codes` row with `owner_type = 'agent'`, matching `owner_id`, and `status = 'active'`, ordered by `created_at DESC, id DESC`.
- Read-only routes:
  - `GET /agent/api/v1/users/{user_id}/assets`
  - `GET /agent/api/v1/users/{user_id}/margin-positions`
  - `GET /agent/api/v1/users/{user_id}/margin-orders`
  - `GET /agent/api/v1/users/{user_id}/spot-orders`
  - `GET /agent/api/v1/users/{user_id}/seconds-contract-orders`
- Every list response carries a filter-consistent `total`; financial page defaults to 20, caps limit at 100, and caps offset at 100000.

### 3. Contracts

- Newly generated user-owned and agent-owned codes share the same format and the global unique index on `invite_codes.code`. MySQL duplicate-key collisions retry with a fresh code for a bounded number of attempts.
- A regular user continues to receive its earliest valid user-owned code. A user referenced by an active `agents.user_id` instead receives that agent's latest active agent-owned code.
- If an active linked agent has no active code, `/referral/my-code` locks the agent row, creates one, and returns it. Portal creation locks the same row, so concurrent first reads and portal writes use one lock order.
- Portal status mutation locks the owned invite-code row before the agent coordination row, matching registration's `invite code -> agent` order. It then updates and rereads in one transaction; same-status requests are idempotent, and a completed disable cannot interleave inside an effective-code read.
- Creating a newer active portal code changes only the linked user's displayed effective code. Existing codes, including historical long-format agent codes, remain valid for registration/binding until separately disabled or exhausted.
- Agent financial authorization comes exclusively from the authenticated `AgentAccessScope.path`. No path, query, or request-body agent ID can enlarge it.
- Parent agents may read users owned by descendant agents. Parent, sibling, unrelated-root, unassigned, and nonexistent users are indistinguishable as `NOT_FOUND` to an out-of-scope caller.
- Membership pre-checks are not persistent authorization. Each wallet/position/order row query and its COUNT query must independently join `user_referrals -> agents` and repeat the materialized-path predicate.
- Asset rows retain one account per source ledger and expose `account_type = spot | margin`; balances from the two ledgers are never silently merged. `logo_url` remains present even when null, and stored `precision_scale` outside `0..=18` fails closed instead of reaching amount formatting.
- Missing seconds-contract status means no status predicate and therefore includes `opened`, `settled`, `manual_review`, and `refunded`. Margin and seconds status filters must also apply to COUNT. Principal refunds remain a separate terminal status with no invented win/loss.
- Margin orders and positions share `margin_positions`; no separate order ledger is invented. `margin-orders` returns `{ orders, total }` using the position row DTO. `status=pending` selects stored `opened AND entry_price IS NULL`; `status=opened` selects `opened AND entry_price IS NOT NULL`. Missing status includes all records; other filters are `closed | canceled | liquidated`. Response status remains the stored enum, not `pending`.
- `margin-positions` always requires `entry_price IS NOT NULL`, including its COUNT, so an unfilled/canceled limit order is never labelled a position. Its existing status parameters remain accepted for compatibility.
- Spot orders return `{ orders, total }`, with `side=buy|sell`, `order_type=market|limit|stop_limit`, nullable Decimal `price/trigger_price`, Decimal `quantity/filled_quantity`, and millisecond `created_at/updated_at`. Optional status is `pending | open | partially_filled | filled | cancelled | rejected`; missing status includes ongoing and historical orders.
- In the seconds-contract portal table, the backend value `opened` is labelled `进行中`; the generic margin-position label `持仓中` must not be reused for that context.
- Opened seconds orders show a display-only countdown using `max(0, ceil((expires_at - Date.now()) / 1000))`, updated once per second and resynchronized on visibility/focus. Format is `mm:ss` (or `hh:mm:ss`); zero reads `待结算`, never a client-invented settled state. Settled/manual-review rows show `-`. Countdown expiry makes no API call or financial write; timer/listeners are cleaned on row removal, tab/page/user changes and unmount.
- The portfolio has five lazy-loaded, independently paged tabs. Pending margin orders display `待成交` from status plus entry-price nullability; the orders table displays `created_at` as placement time, not a fabricated fill time. Refresh invalidates that tab's cached pages; user or agent session generation changes reset all financial caches and pending requests. Late responses cannot replace newer filters.
- Financial routes execute SELECT-only snapshots. They must not lazily create wallets, load market prices, accrue interest, settle contracts, close/liquidate positions, mutate balances, or write ledgers/events.
- Every amount, price, rate, leverage, and PnL crosses JSON as Decimal text; timestamps use Unix milliseconds.

### 4. Validation & Error Matrix

- Invalid financial status -> `VALIDATION_ERROR` before opening a database pool/query path.
- Missing/inactive authenticated agent chain -> `UNAUTHORIZED`.
- Target outside the token-derived subtree or absent -> `NOT_FOUND` with the same response shape.
- Invite-code random-source failure or bounded collision exhaustion -> internal error; no partial invite row is committed.
- Unknown query parameters such as `agent_id` have no scope effect.

### 5. Good/Base/Bad Cases

- Good: level 1 reads a level-3 user's two wallet-account rows, open margin position, and all seconds orders including `opened`.
- Good: two concurrent first `/referral/my-code` calls for an agent-linked user return the same newly created agent-owned code without duplicate-key leakage.
- Base: a team user has no wallet or order rows; authorized routes return empty arrays and `total = 0` without creating data.
- Bad: checking subtree membership once and then querying financial rows by only `user_id`; a concurrent reassignment can leak the old user's records.
- Bad: defaulting seconds orders to `settled`; active exposure disappears from the agent view.
- Bad: combining spot and margin balances for the same asset without an account-type discriminator.
- Bad: classifying every stored `opened` margin row as a position; unfilled limit orders also use that status.

### 6. Tests Required

- Invite integration: generated portal codes are six uppercase alphanumeric characters; concurrent linked-user first reads converge; a newer portal active code becomes effective immediately; disabling it falls back to the next latest active code; same-status mutation is idempotent; a historical long code still binds.
- Scope integration: root reads direct/descendant users while child requests for parent, sibling, other root, unassigned, and nonexistent users all return not-found, including requests that append an `agent_id` query.
- Financial integration: spot/margin accounts remain distinct, status-filtered totals match rows, default seconds results include `opened`, Decimal/timestamp fields retain their wire types, invalid stored precision fails closed, and before/after wallet and order snapshots are identical.
- Route unit tests: all five endpoints require agent scope, validate statuses, and enforce pagination bounds.
- Margin/spot integration: pending and canceled-unfilled margin records appear in orders but never positions; filled/partial/canceled spot orders retain accurate status totals and scoped paging. Reads leave balances, order states and fill quantities unchanged.
- Countdown fake-clock tests cover sub-second ceiling, hourly formatting, expiry and already-expired rows, terminal states, background clock jumps, cached-tab return, pagination, refreshed settlement and timer cleanup; ticking must not add API requests.
- OpenAPI and web tests: routes/schemas preserve Decimal strings and milliseconds; detail tabs lazy-load, cache successful query keys, isolate filter refreshes, and keep the team-users nav selected.

### 7. Wrong vs Correct

#### Wrong

```sql
SELECT * FROM seconds_contract_orders WHERE user_id = :user_id;
```

The preceding membership check can become stale, so this query leaks after a concurrent ownership move.

#### Correct

```sql
SELECT orders.*
FROM seconds_contract_orders orders
JOIN user_referrals referrals ON referrals.user_id = orders.user_id
JOIN agents owner_agents ON owner_agents.id = referrals.root_agent_id
WHERE orders.user_id = :user_id
  AND (owner_agents.path = :scope_path
       OR owner_agents.path LIKE CONCAT(:scope_path, '/%'));
```

Rows and COUNT repeat the server-derived subtree predicate, and the surrounding use case remains strictly read-only.

## Explicit Commission Reversal And Source Refund

- `POST /admin/api/v1/agent-commissions/:id/reversal` requires an authenticated
  Admin and exact `agents.commissions.settle`; body is strictly
  `{idempotency_key, reason}`. Legacy PATCH status remains independently gated.
- Only settled commissions with a unique positive available payout ledger
  matching original amount/asset may reverse. Debit that ledger's recipient,
  never the current mutable agent owner. Preserve source, source amount, rate,
  commission amount, asset and original payout; append an immutable reversal
  snapshot and advance status to `reversed`.
- Available must cover the exact original amount; frozen/locked are untouched.
  Insufficiency refuses atomically, with no negatives, synthetic debt or
  retroactive commission recalculation.
- Wallet, reversal ledger, receipt, status, authenticated reason audit and
  platform journal commit together. Original payout legs are commission expense
  +amount and user commission wallet liability -amount; reversal is exactly
  opposite. Transaction keys are `agent_commission:{id}:payout|reverse`.
- Actor/key digest uniqueness is an index guard, not proof of replay: compare
  raw key, actor and normalized reason. Exact replay returns the original
  receipt after restart; changed request/collision/cross-record reuse conflicts.
  Migration 0137 repairs UTF-8 text metadata without editing applied 0135.
- Source refunds never imply commission reversals. Seconds refunds require
  opening-time opt-in policy and reject pending source commissions; any paid
  status or payout evidence refuses the source refund instead of clawing back.
- Seconds paths lock the source order before commissions and wallet, with
  lock-time current source eligibility. Hint reads precede transaction begin,
  avoiding old RR snapshots and nested pool acquisition. Other source lifecycle
  contracts are unchanged. Append-only payout evidence has no ledger range lock.
- Test actual MySQL concurrency, restart/raw replay conflict, changing agent
  bindings, insufficient balances, journal/audit rollback, source refund races,
  per-asset zero-sum legs and settle-only frontend/backend authorization.
