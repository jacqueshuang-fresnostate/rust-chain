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

The server-derived path enforces the current agent subtree without accepting a client scope.

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
