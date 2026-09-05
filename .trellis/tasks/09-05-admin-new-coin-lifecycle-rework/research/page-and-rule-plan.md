# Admin new-coin page and rule plan

Date: 2026-09-05. Status: approved; core page/command slice implemented and validated; see `../project-center-review.md`.
The local evidence below describes the pre-implementation state.

## Local evidence

- `web/src/admin/navigation.tsx`: seven sibling entries currently mix projects,
  lifecycle actions, subscriptions, distributions, purchases, locks and unlocks.
- `web/src/admin/actions/NewCoinActions.tsx`: separate project selectors and local
  form defaults for lifecycle, grants, unlock and fee settings; no project-center
  authoritative configuration hydration. The new manual settlement card sits in
  this shared action page as an incremental financial slice.
- `web/src/admin/resources/actions/newCoins.tsx`: project rows navigate to an
  actions page with `project_id`; preheat has a guarded start-subscription action.
- `web/src/admin/routes.tsx`: resource routes can remain compatibility entrypoints
  while project-scoped views and work queues reuse existing resource contracts.
- `web/src/admin/access.tsx`: projects, subscriptions, distributions, purchases,
  locks and unlocks have separate scopes. Grouping pages must not widen access.

## Approaches considered

1. Rename existing menus only: smallest change but leaves repeated project
   selectors, default-filled configuration and mixed operations unresolved.
2. Put everything inside a project page: strong project context, but operators
   must open projects repeatedly to handle a cross-project pending queue.
3. Project center plus cross-project work queues (recommended): one authoritative
   configuration context and dedicated operational lists. Reuse the same readers
   and commands with optional project scoping rather than duplicate business logic.

## Proposed sidebar

Parent label: 新币管理.

| Entry | Responsibility | Writes / boundaries |
| --- | --- | --- |
| 项目管理 | Project list, creation and project center | Basic/configuration editing, guarded lifecycle commands |
| 申购与配售 | Pending and settled subscriptions, project/user/status filters | Final per-order allocation and refund only in the permitted stage |
| 派发与退款记录 | Completed receipts, actual delivered/refunded values and linked ledger | Read-only results; distinguish subscription fulfillment and free grants |
| 锁仓与解禁 | Positions and release records in separate tabs | Eligibility-checked release where an existing authorized API supports it |
| 上市后购买 | Post-listing purchase orders and linked outcomes | Purchase configuration belongs in the project center; not subscription fulfillment |

The lifecycle-action menu disappears as a primary entry. Unlock fee rules are
project settings, not a separate business lifecycle. Audit lives in project
history and the existing shared audit module, not another duplicate top-level page.
Free grants remain an explicitly labeled advanced project action with permission
and reason checks; they never masquerade as settlement of a subscription.

## Project center

- Header: authoritative identity, lifecycle, operational status and allowed next
  command. Show the reason a command is unavailable and actionable readiness gaps.
- Summary: issued/reserved/allocated/remaining supply, pending manual orders,
  outstanding frozen funds and delivered/refunded totals from backend aggregation.
  Never infer project totals by summing only the currently loaded list page.
- Tabs: overview; issuance/settings; subscription/settlement; delivery/refund;
  lock/unlock; post-listing purchase; events/audit. Render each with its own scope.
- Forms load current values by project ID; deep links cannot depend on the first
  100 reference options. Loading/error state must not offer default-value writes.
- Save field changes only; confirm affected scope and reason. Successful writes
  refresh the current project, relevant records and precise reference cache.
- Use server command-time checks even if the UI already disabled a control.
  Conflicts reload authoritative state; unknown-result settlement retries retain
  the original key instead of silently creating a new confirmation.

## Stage / operation rules

| Stage | Main command | Financial behavior |
| --- | --- | --- |
| 预热中 (`preheat`) | Complete configuration, then 开始申购 | No subscription funds or automatic delivery |
| 申购中 (`subscription`) | Review accepted orders, then 结束申购 | Accept within remaining quota; freeze only |
| 派发中 (`distribution`) | Confirm each order, then 确认上市 after readiness | Actual allocation and difference refund commit together; zero fully refunds |
| 已上市 (`listed`) | Manage post-listing purchase and inspect unlock eligibility | No return to subscription or repeat settlement of terminal orders |

These codes already exist; do not add draft/completed/cancelled states merely to
match menu labels. Paused operational status, subscription outcome and individual
lock maturity are separate dimensions. Ending subscription never auto-distributes.

## Editing and historical rules

- Protect asset/quote identity, price and accepted-order terms once subscriptions
  exist. Define allowed project field changes explicitly instead of exposing a
  generic whole-object editor; supply counters remain computed, not editable.
- Historical payments, allocations and lock snapshots remain intact. A project
  rule edit must not silently rewrite existing holdings or frozen obligations.
- Planned listing time is not proof that listing actually happened. Separate
  planned time, actual listing event and lock-rule maturity in the follow-up design.
- Manual and automatic release must use the same maturity and fee-evidence checks.
  Whether fixed/relative maturity also requires actual listing remains a business
  decision; no new gate is asserted as implemented by this page plan.
- Operational pause effects, especially refunding outstanding frozen funds, need
  an explicit operation matrix. A menu redesign must not add accidental irreversible
  financial restrictions or silently enable settlement on paused projects.

## Compatibility and rollout order

1. Confirm page boundaries, nomenclature and project action matrix.
2. Add authoritative project read/configuration and readiness contracts with field
   editability; build the project center before removing the old primary entry.
3. Relocate existing manual-settlement controls into the subscription work queue;
   merge lock/release navigation using tabs, reuse current record readers.
4. Finish planned/actual listing and manual/worker unlock consistency separately,
   with migration and historic-snapshot compatibility tests where needed.
5. Verify deep links, pagination beyond 100 projects, per-tab permissions, current
   configuration hydration, conflict/retry behavior, browser layouts and financial
   regressions. Preserve existing routes as redirects/aliases with query context.

This slice does not introduce oversubscription, batch auto-allocation, lotteries,
user cancellations, automatic timed lifecycle advancement, or multi-approval
workflows. Keep extension boundaries, but require separate confirmation for them.

## Implementation boundary — project-center slice

- Implemented five menu categories, exact-ID project center, overview/configuration/
  record-link/extra-grant tabs, whitelisted filtered work queues and old URL redirects.
- Summary currently exposes authoritative supply and order/pending counts, not
  new aggregate frozen/refund totals. Record tabs are permission-scoped links to
  existing work queues rather than seven independently duplicated nested lists.
- Refund amount remains an order-level field on subscriptions; receipts clearly
  distinguish grants and zero refunds but do not invent a joined refund amount.
- Locks/unlocks are read-only existing lists; no new Admin release operation was
  added. Their asset filter is explicitly broader than one project.
- Preheat issuance editing and snapshot concurrency protection are implemented.
  Planned/actual listing, manual/worker fee consistency, operational pause and
  historical lock migration remain follow-up work, not completed by this slice.
