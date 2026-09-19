# E08 Prospective Seconds Principal Refund

## Authorized Boundary

Implementation verified and frozen on 2026-09-19. Policy is disabled/null by default, never activated
by migration. Only newly opened orders may snapshot an enabled policy. Existing
orders without a snapshot remain ineligible permanently. No threshold is chosen.

## Work Checklist

- [x] Immutable policy revisions, new-order snapshots and refund receipts (0139).
- [x] Audited policy configuration, evidence-checked principal refund, source locks.
- [x] Admin configuration/action and all client terminal labels.
- [x] Real database race/replay/rollback/balance tests and frontend checks.
- [x] Delivery evidence and remaining limitations.

## Coordination

Ohm owns 0138 and capacity admission. Refund policy storage/action is independent.
Opening needs one snapshot hook after insertion while holding the product lock.
The available tool registry did not expose multi_agent_v1.send_input; main relays
the hook contract. No app-thread coordination after that instruction.

## Delivered Contract

- Dedicated product policy GET/PATCH and order principal-refund GET/POST.
  Exact read/write/settle permission mappings, authenticated actor, mandatory
  normalized reason, strict payloads and both OpenAPI aliases are wired.
- Product lock serializes policy CAS and NEW-order snapshot insertion.
  Legacy replays never gain snapshots; later policy disable/product closure
  cannot revoke the original snapshot. Policy history prevents physical
  product deletion; operators can disable products instead.
- Eligibility uses the original unresolved `missing_settlement_snapshot`
  exception, original event window, database clock and original wait snapshot.
  No current ticker/client price, fabricated outcome or default duration.
- Refund explicitly uses repeatable-read isolation and locks source order,
  indexed history window, ascending commission IDs, then original wallet.
  Empty history-range locks serialize late archival. All seconds commission
  payout/batch/worker/reversal entry points use source-first locking and
  lock-time source revalidation; pre-read hints occur before the transaction.
- Exact original debit evidence and current precision are required. Only
  original available principal is credited; frozen/locked remain unchanged.
  Corrupt evidence, negative wallet buckets, paid/reversed commissions or ANY
  original payout evidence refuse. Pending commissions are rejected atomically;
  no source clawback, negative debt or retroactive commission basis is invented.
- Receipt, wallet, ledger, terminal `refunded`, commission rejection, balanced
  platform journal and Admin audit share one transaction. Result/price stay
  null. Journal key `seconds_contract:{id}:refund` has pending liability
  +principal and user wallet liability -principal.
- Generated SHA-256 digest is indexed; replay verifies raw key, authenticated
  actor and trimmed reason. Cross-order reuse/uniqueness collision conflicts.
  The database does not equate digest equality with request equality.
- Application event wrapper publishes `seconds_contract.order.refunded` only
  after a NEW commit to the original user's private channel. Payload contains
  exact `refund_amount`, terminal status and null result/price, not a payout
  or fabricated win. Replays and failures emit nothing.
- Admin/Agent/PC/Mobile map `refunded`; PC avoids false result popups and renders
  absent price/PnL as unavailable. Mobile keeps neutral terminal/PnL semantics.
  Wallet principal-refund classification has explicit Chinese/English labels.

## Changed Paths

- `migrations/0139_seconds_principal_refund.sql` (immutable after local application).
- `src/modules/seconds_contract/{application,infrastructure,presentation,routes}/refund.rs`.
- Seconds parent `application.rs`, `infrastructure.rs`, `presentation.rs`,
  `routes.rs`, `service.rs`: declarations, audited deletion guard, event wrapper;
  Ohm owns the NEW-order snapshot hook in `application.rs`.
- `src/modules/admin/infrastructure/agents/source_lock.rs`,
  `src/modules/admin/infrastructure/agents.rs`,
  `src/modules/admin/application.rs`,
  `src/modules/admin/application/agents.rs`,
  `src/modules/admin/application/agents/reversal.rs`: source-first coordination.
- `src/modules/admin/infrastructure/agents/reversal.rs`: requested integer
  comparison lint repair only during refund finish.
- `src/modules/admin/service/access_control.rs`, `web/src/admin/access.tsx`:
  narrow exact refund-policy/context/command permission mappings.
- `src/openapi/seconds_refund.rs`, declaration/merge in `src/openapi.rs`.
- `src/modules/agent/{application,infrastructure,service}.rs`:
  terminal filter and lifecycle documentation.
- `web/src/admin/resources/actions/secondsRefund.tsx`,
  `secondsRefund.test.tsx`, hooks in `secondsContract.tsx`.
- `web/src/api/agent.ts`, `agent.test.ts`,
  `web/src/agent/UserPortfolioPage.tsx`, `web/src/shared/adminEnumLabels.ts`.
- `mobile/src/core/{secondsOrder,walletLedger}.ts`,
  `mobile/src/i18n/messages/{en,zh-CN}.ts`,
  `mobile/tests/seconds-principal-refund.test.ts`.
- `pc/src/api/{backendAdapters,transaction}.ts`,
  `pc/src/views/SecondOptions.vue`, `pc/src/i18n/index.ts`,
  `pc/tests/seconds-principal-refund.test.ts`.
- `tests/seconds_contract_routes/principal_refund.rs` and child registration.
- `.trellis/spec/backend/seconds-contracts.md` and
  `.trellis/spec/backend/agent-hierarchy.md` own lifecycle contracts;
  original E08 paths remain in `research/e08-delivery.md`.

## Verification

- Dedicated local MySQL 9.3 `hardening_commission_test` on port 13316 and Redis
  on 16386/database 11, proxy environment cleared, escalated test execution.
  `cargo test --test seconds_contract_routes principal_refund -- --nocapture`:
  **1/1 comprehensive scenario passed**, then current compiled binary repeated
  **1/1**. Fixtures exercise real routes, opening transactions and SQLx migrations.
- Assertions: disabled/null defaults; explicit wait and reason; policy CAS;
  prospective snapshot/legacy replay exclusion; disable-after-open; waiting;
  concurrent identical refund, actor/reason/key conflicts and cross-order reuse;
  restart replay; exact available/frozen/locked balances; one audit and balanced
  journal; refund/pay and refund/evidence-settlement races; locked empty history
  versus late archival; missing debit/precision/corrupt provenance refusals;
  paid/payout-evidence anomalies; duplicate journal and injected audit failure
  rollback; strict payload and unauthenticated/user/readonly/write-only denial;
  settle-only POST success; public terminal response and both OpenAPI aliases.
- Same real-route test subscribes to original-user and other private channels:
  one event for the concurrent new commit, none for replay or each injected
  failure; exact principal, null result/price and no payout field.
- Final source-lock regression:
  `cargo test --test admin_routes commission_reversal:: -- --nocapture`
  **1/1**, existing `agent_commission` selection **7/7** using its current binary.
- Final Web focused `secondsRefund`, `commissionReversal`, `ConfirmAction`:
  **3 files, 18/18**, refund file **8/8**. Includes product-disabled original
  snapshot without product reread, refusal to create fresh commands on refunded
  orders, retained policy conflict draft, legacy refusal, durable remount retry,
  malformed receipt and storage fail-closed behavior.
- Web typecheck and focused ESLint for the final refund component/test passed
  after the last frontend test addition.
- Main reported final shared gates: Rust units **463**, architecture **11**,
  documentation **1**, OpenAPI **10**, clippy; Mobile **779** plus full release
  gate/PWA/Tauri; PC **110** plus type/build; Web **820** plus type/lint/build/
  budget. These are main's integration results, not rerun in this agent.
  The added final Web test above was independently run after the Web820 gate.
- Final owned Rust `rustfmt --edition 2024 --config skip_children=true --check`
  and `git diff --check` passed. Shared `src/openapi.rs` and parent route-test
  final ordering/formatting are main-owned.
- Real browser component fixture on the existing local Web server: desktop
  1440px and narrow 390px policy form; narrow refund confirmation/error.
  Modal x=16, width=358, right=374; document width=390, no horizontal overflow;
  failure preserves reason. Screenshots inspected:
  `/tmp/e08-refund-policy-desktop.png`, `/tmp/e08-refund-policy-narrow.png`,
  `/tmp/e08-refund-confirm-narrow.png`, `/tmp/e08-refund-error-narrow.png`.
  TaskSpace17 closed. Mocked API preview, not live authenticated browser E2E.

## Remaining Boundaries

- No runtime policy activated, threshold chosen, old-order backfill, migration
  rewrite, production access, commit, deployment or global tracking/index edit.
- Existing in-memory private hub is best-effort refresh, not durable delivery:
  crash after commit can lose the hint; REST remains authoritative.
- Production MySQL 8.4 is not certified by local MySQL9.3 coverage. Actual
  cryptographic hash collisions and database outage injection were not simulated;
  raw replay comparison and propagated SQL errors are implementation safeguards.
- Product-history deletion refusal is implemented but not separately exercised
  in the focused real-DB scenario. Complete authenticated browser/backend E2E,
  deployment migration sequencing and operational rollout remain integration work.
- Legacy/no-snapshot manual-review orders remain ineligible. Paid-commission
  anomalies require investigation; this policy introduces no automatic clawback.
  Further historical cancellation or post-settlement refund rights need a
  separate explicit policy and source/payment concurrency contract.
