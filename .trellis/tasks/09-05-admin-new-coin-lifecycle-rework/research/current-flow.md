# New-coin lifecycle audit and proposed implementation slices

Date: 2026-09-05. Status: pre-implementation audit plus the final-decision addendum below.
The original findings and candidate designs describe the earlier source state;
they are not the current manual-settlement implementation contract.

## Scope and evidence limits

This audit covers Admin creation/configuration, lifecycle transitions, public subscriptions,
Admin distributions, post-listing purchases, and manual/worker unlocks. Findings below are
from local source inspection, not execution against production or a test database. No live
project, balance, order, lock, deployment, or historical migration was changed.

The earlier preheat-to-subscription shortcut and BrowserRouter navigation fix remain in
the working tree. They address discoverability, not the financial lifecycle described here.
The user's unrelated Mobile project-list style change must remain intact.

## Confirmed current behavior

| Area | Source anchor | Behavior / mismatch |
| --- | --- | --- |
| Lifecycle graph | `src/modules/new_coin/domain.rs:22`, `:34` | Only `preheat -> subscription -> distribution -> listed` is valid. Same-state, reverse, and skipped transitions fail. |
| Creation | `src/modules/admin/service/new_coin.rs`, `validate_create_new_coin_project` | Any of the four valid stages can be an initial stage, including historical imports. There is no draft state. |
| Admin transition | `src/modules/admin/application/new_coin.rs:359` | Locks the project, checks active status and the forward graph, then writes the state, event, and audit in a transaction. There is no outstanding-order/distribution-completion readiness check. Listing accepts a timestamp or uses now. |
| User subscription | `src/modules/new_coin/infrastructure.rs:408-550` | Reserves supply, debits quote available balance, credits or locks the full base quantity, sets allocated quantity to requested quantity, and finalizes supply in one transaction. `pending` is transient within that transaction, not an awaiting-allocation phase. |
| Linked Admin distribution | `src/modules/admin/application/new_coin.rs:582`; `src/modules/admin/infrastructure/new_coin.rs:667` | Optional subscription linkage adds to allocated quantity and rejects amounts above requested quantity. Current successful subscriptions are already fully allocated, so they have no positive remaining entitlement for this action. |
| Unlinked Admin distribution | Same distribution use case | Without a subscription ID, this is a separate zero-cost grant consuming project supply. Combining it with order fulfillment in one form obscures the distinction. |
| Distribution retries | `src/modules/admin/application/new_coin.rs`, `distribute_admin_new_coin` | An existing idempotency key returns conflict rather than the original successful result. Financial actions need clear unknown-result/retry handling. |
| Unlock calculation | `src/modules/new_coin/domain.rs:216`, `apply_unlock_rule` | `immediate_on_listing` uses a timestamp, not lifecycle state. A source time at/after that timestamp can produce available coins while the project is still in an earlier lifecycle stage. Fixed/relative rules use absolute/source-relative maturity. |
| Unlock edits | `src/modules/admin/application/new_coin.rs:407` | Changes project rules only, not existing lock snapshots. Immediate-on-listing edits update `listed_at`; advancing to listed can overwrite that same field. Historical lock times remain independent. |
| Actual release | `src/modules/new_coin/infrastructure/unlock.rs:363`; `src/workers/unlock_scanner.rs:225`, `:286` | Both release by position maturity without checking the project's listed lifecycle. Thus a planned timestamp is not an actual-listing gate. |
| Fee release eligibility | Same release files | The manual release query verifies paid timestamps, ledger evidence, and matching platform financial journals; the worker query and locked check rely on fee status. Eligibility differs between entry points. |
| Direct purchase after listing | `src/modules/admin/application/new_coin.rs:525` | The endpoint exists and requires active/listed, including for disabling. Enabling validates the bound pair and activates it. Disabling only disables direct purchase, not the pair. The Admin UI has no configuration form for this endpoint. |
| Admin read/edit endpoints | `src/modules/admin/routes/new_coin_convert.rs:17-92` | List/create, dedicated rule writes, and order lists exist. No project detail/update/status/lifecycle-event read route exists. Project list only supports pagination. |

### Important interpretation boundaries

- The linked-distribution quantity guard prevents over-allocating a subscription. This audit
  does **not** claim that the current linked action pays the same entitlement twice.
- Config edits preserving existing lock snapshots can be intentional. The defect is that the
  UI does not explain the boundary and `listed_at` combines schedule and actual-event meaning.
- Distinct lock acquisition sequences in worker/manual release are a concurrency risk to test,
  not a reproduced deadlock in this audit.
- Earlier progress records report one real-MySQL subscription concurrency test returning
  `[200, 500]` instead of `[200, 400]`; that historical observation was not reproduced here.

## Admin interaction audit

`web/src/admin/actions/NewCoinActions.tsx` currently has four independent forms with four
project selectors: lifecycle, distribution, unlock rule, unlock fee.

1. Lifecycle defaults to `subscription` regardless of the chosen project and offers every
   stage. Users can submit reversals/same-state changes that the backend correctly rejects.
2. Forms do not hydrate the selected project's current configuration. Blank/default rule and
   fee inputs can be mistaken for the saved configuration.
3. Unlock fields are all displayed together; mutually exclusive backend rule shapes are not
   represented as mutually exclusive UI fields.
4. Fee asset options include all active assets although the backend requires the quote asset.
5. Distribution asks for raw subscription and idempotency IDs, without an entitlement preview
   or a clear separation between order fulfillment and additional grants.
6. Successful writes do not refresh the authoritative selected-project configuration or
   invalidate project reference options.
7. `web/src/admin/referenceOptions.tsx:149` fetches only the first 100 projects.
   `web/src/admin/sharedOptionQuery.ts:28-43` retains options for five minutes. A selected
   project's status/config must not depend on being present in that cached page.
8. `web/src/admin/actions/NewCoinActions.test.tsx:102` submits a distribution-stage fixture
   to subscription and treats a mocked success as valid. Replace this with realistic stage
   fixtures and tests of disabled/allowed transitions and conflict recovery.

The project center should retain the earlier route correction in
`web/src/admin/resources/actions/newCoins.tsx`, not restore hash navigation.

## Blocking product decision: subscription settlement model

The user replied “好的” after the staged-allocation recommendation on 2026-09-05.
Option A's settlement direction is now confirmed. Its allocation quantity policy and
cancellation/refund boundaries remain open; option B below is retained as decision history.

### A. Staged allocation (settlement direction confirmed)

- Subscription: reserve eligibility/supply according to the confirmed allocation policy and
  freeze the quoted amount; do not credit new coins yet.
- Close subscription: stop accepting new orders and settle a deterministic allocation.
- Settlement: consume the accepted payment and refund/release the unallocated remainder,
  with explicit money/supply conservation and idempotent order outcomes.
- Distribution: deliver only settled but not yet delivered entitlement, then create the
  configured lock snapshot. Allocation and delivery are distinct recorded states even if
  the project keeps one compatible `distribution` stage.
- Listing: advance only when confirmed readiness conditions are met; record actual listing
  separately from planned time and direct-purchase enablement.
- Follow-up financial decisions: allocation policy (e.g. full first-come orders, proportional,
  or operator-approved quantities), cancellation/refund boundaries, and unsold supply policy.

This changes the financial contract. Existing fully settled subscriptions must remain
settled. Introduce an explicit model/version discriminator and a new migration if needed;
never infer unsettled status from an old label or replay payment/allocation.

### B. Immediate settlement (compatible with current user write semantics)

- Subscription remains a direct purchase at authoritative issue price, with immediate
  debit and delivery/lock in the same transaction.
- The end of subscription does not create another allocation/payment obligation.
- Separate Admin grants from subscription fulfillment; remove misleading linked-fulfillment
  actions for already settled orders while preserving valid historical entitlements.
- Lifecycle labels, records, and instructions must say what actually happened, rather than
  imply a future lottery/allocation step.

Both options still require project-centric editing, clear time semantics, guarded transitions,
audits, authoritative refresh, and unified unlock eligibility.

## Follow-up: confirmed staged direction and quantity-policy research

The original design in `docs/superpowers/specs/blockchain-exchange/03-new-coin-lifecycle.md`
sections 2-3 describes payment reservation during subscription, allocation after closing,
refunds for unallocated amounts, and delivery during distribution. It does not specify an
allocation algorithm. Its older purchase pricing and fee descriptions are not authoritative
over the newer executable contracts; do not restore them as part of this rework.

The current `src/modules/new_coin/infrastructure.rs:1033` reservation helper accepts the
whole requested quantity only when `remaining_supply >= quantity`. There is no current
oversubscription pool or lottery/proportional allocator. This makes the following a separate
product decision from the already approved change in settlement timing:

| Quantity policy | Behavior | Repository impact |
| --- | --- | --- |
| Full allocation within available quota (recommended for the first release) | Accept the entire request if quota remains, otherwise reject without freezing funds. Accepted requests reserve quota; closing triggers full settlement and then delivery. Admission follows serialized transaction order, not a guaranteed client wall-clock order. | Preserves the existing no-oversubscription admission rule while separating payment and delivery. No partial allocation in the normal successful flow. |
| Proportional allocation after oversubscription | Accept demand above supply, freeze payments, then allocate a proportional quantity and refund the rest. | Requires a separate demand counter, frozen allocation population/cutoff, deterministic decimal rounding and residual distribution. Demand must not violate the existing supply conservation check. |
| Operator-confirmed quantities | Admin reviews and confirms quantities per order; residual frozen payments are returned. | Requires allocation limits, preview/confirmation, audit and replay protection. User consent to staged settlement alone does not authorize arbitrary quantity adjustments. |

### Existing wallet patterns and proposed common accounting constraints

- `src/modules/wallet/domain.rs:63` applies available/frozen/locked deltas atomically in
  memory and rejects negative buckets. Wallet service `freeze`, `unfreeze`, and `settle`
  illustrate domain changes but do not themselves guarantee one SQL transaction across
  order, wallet, ledger, and supply. Do not compose separate repository commits for this flow.
- `src/modules/spot/infrastructure/wallet_accounts.rs:61`, `:122`, `:319` illustrates
  transaction-bound freeze/refund with paired available/frozen ledger legs. Reuse the
  pattern, not a private Spot-specific order API or a broad unrelated refactor.
- Each new staged order needs its own remaining frozen obligation. Wallet aggregate frozen
  funds can include Spot and other orders, so a settlement must validate both the order's
  obligation and the wallet bucket; checking the aggregate alone is insufficient.
- Proposed monetary invariant: initial frozen amount = consumed payment + returned amount
  + remaining order freeze, with all terms nonnegative. A terminal settled/refunded order
  has zero remaining freeze. Use the immutable issue-price/quote-asset snapshot and existing
  decimal precision calculation; never compute the final debit from a changed project price.
- Proposed quantity invariant: `0 <= delivered_quantity <= allocated_quantity <=
  requested_quantity`. Legacy `allocated_quantity` already represents delivered holdings;
  preserve its historical meaning using a versioned order model rather than creating a new
  delivery obligation from old rows.
- Quota reservation across transactions must remain represented until delivery or release.
  Keep `total_supply = remaining_supply + reserved_supply + allocated_supply`; the existing
  project `allocated_supply` means coins actually credited, not merely an announced award.
- Settlement atomically records allocation, consumes accepted funds, releases any difference,
  and updates the order's obligation. Delivery atomically consumes the settled undelivered
  entitlement, creates the coin ledger/lock/source record, and finalizes the supply reserve.
  Replay at either step must not repeat money or supply movements.
- Normal full-allocation orders have no refund difference; refunds still need a clearly
  defined cancellation/rejection path. Do not silently add user cancellation, project
  cancellation after delivery, or clawback of distributed coins before those rules are agreed.

These are design constraints and source-pattern findings, not implemented/tested behavior.

## Proposed common lifecycle / operations matrix

This matrix is a design proposal, not new behavior already shipped.

| Dimension / stage | Operator operations | Rules to enforce on the backend |
| --- | --- | --- |
| Preheat | Inspect/edit configuration, review readiness, start subscription | Validate active assets, quote/price/supply, and coherent unlock/fee rules before advancing. Do not present arbitrary state selection. |
| Subscription | Inspect orders and totals, end subscription | Protect accepted-order financial terms. New orders must serialize with closing the stage. Settlement semantics depend on A/B above. |
| Distribution | Inspect entitlement/grant records, deliver eligible quantities, review readiness for listing | Separate order fulfillment from discretionary grants. Bound quantities, supply conservation, retry reconciliation, and audit are mandatory. |
| Listed | Inspect actual listing time, bound pair, direct-purchase settings and records | Keep issue-price direct purchase distinct from spot trading. Show pair activation side effects explicitly; do not silently alter unrelated trading. |
| Operational enablement | Explicit pause/resume with reason and impact preview | Separate from lifecycle and irreversible financial settlement. Define which actions pause affects; a pause must not silently confiscate due funds or rewrite entitlements. |
| Per-user lock/unlock | Inspect maturity, fee evidence, paid/released records | Not a project stage. Manual and worker eligibility must share the same authoritative checks. Config changes must state their effect on existing snapshots. |

No extra draft/closed/cancelled lifecycle codes are approved. Keep the four-stage public
contract until the financial decision proves new stages necessary; model sub-operations
separately rather than making every user's unlock status a project-wide stage.

## Proposed implementation slices

1. **Contract and regression baseline**: settle A/B and its dependent financial rules;
   define state/operation matrix, planned vs actual time, immutable order snapshots,
   idempotency behavior, lock ordering, and migration/rollback boundaries.
2. **Project center read/configuration path**: authoritative per-project detail and history,
   searchable/filterable list, one selected project, hydrated forms and field editability;
   show supply/orders/locks/purchase configuration and read-only reasons.
3. **Lifecycle and financial writes**: backend capability/readiness calculation plus
   transaction-time revalidation, guarded commands instead of free state dropdowns,
   correct A/B settlement/delivery, clear grant and direct-purchase actions.
4. **Unlock consistency and consumers**: actual-listing semantics where applicable, shared
   fee proof/eligibility, historical snapshot compatibility, worker/manual parity; align
   Mobile and PC DTO parsing, records, action availability and translations only as needed.
5. **Verification and rollout preparation**: real-MySQL financial/concurrency assertions,
   Admin interaction/regression gates, affected consumer tests, migration rehearsal and
   documentation. Deployment and live state changes remain outside this task's execution.

## Compatibility surfaces / verification checklist

- Preserve migrations `0006_new_coin_lifecycle.sql`,
  `0014_new_coin_post_listing_purchase_config.sql`, and
  `0111_new_coin_authoritative_issuance.sql`; add new migrations rather than editing history.
- Preserve authoritative asset/quote IDs, DecimalText values, supply counters, idempotency
  fingerprints, wallet ledger and platform journal accounting.
- Public list/detail and Mobile: `.trellis/spec/backend/new-coin-mobile-contract.md`,
  `mobile/src/core/newCoinModel.ts`, `mobile/src/core/newCoinPresentation.ts`.
- PC consumer: `pc/src/api/activity.ts` (projects, subscriptions, distributions, purchases,
  and unlock operations). A stage/DTO change is cross-client, not Admin-only.
- Unit tests: full transition matrix, operation eligibility, config editability and unlock
  calculation; preserve `tests/unit_src/src_modules_new_coin_mod_tests.rs` coverage.
- MySQL integration: subscription vs close race, two competing allocations/distributions,
  over-supply attempts, same-key replay/different-payload conflicts, rollback conservation,
  historical order compatibility, planned/actual listing differences, fee-proof parity and
  concurrent worker/manual release exactly once.
- UI tests: deep link outside first page, current-value hydration, rule-switch field cleanup,
  valid next operation only, required reasons, stale state conflict/reload, uncertain write
  retry, authoritative option invalidation and direct-purchase impact confirmation.
- Do not count tests that skip database branches as executed financial assertions.

## Final decision and implemented slice — 2026-09-05

The user explicitly chose manual final allocation after subscription, including
partial allocation and refunding the difference. This supersedes the earlier
full-quota recommendation and candidate two-step settlement/delivery design.

- New subscriptions reserve quota and freeze quote funds; closing subscription
  does not allocate automatically. Existing settled rows remain `legacy_instant`.
- One Admin confirmation per order atomically settles the final quantity,
  delivers/locks coins, returns the difference and releases unused quota. There
  is no committed paid-but-undelivered intermediate state. Quantity zero is a
  full refund; partial allocation is terminal, not a reusable remaining balance.
- Price is the immutable subscription price snapshot. Decimal precision,
  order-specific frozen obligations, stable lock ordering and supply conservation
  are validated. Listing waits for all manual orders to be settled/refunded.
- Same-key retries return the committed receipt even after listing; changed
  input or another final confirmation cannot duplicate financial effects.
- The Admin form separates subscription fulfillment from unrelated free grants.
  Mobile and PC records distinguish frozen, actually paid and refunded amounts.

The active executable contract is
`.trellis/spec/backend/new-coin-manual-distribution.md`. Real MySQL financial,
concurrency and failure-rollback verification is recorded in `../review.md`.
Project-center configuration hydration, planned/actual listing time and
manual/worker unlock consistency remain open follow-up slices. Nothing in this
addendum represents deployment or modification of actual project balances.
