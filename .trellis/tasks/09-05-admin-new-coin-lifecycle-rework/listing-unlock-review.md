# Listing events and unified unlock — implementation review

## Delivered behavior

- Preserve `listed_at` as the existing planned date; add server-owned
  `actual_listed_at` for actual stage/create events. Configuration writes retain
  actual time, and lifecycle commands reject client-supplied listing dates.
- Migration 0123 uses recorded event evidence only; unknown legacy actual dates
  remain null. No existing lock timestamp, fee snapshot or amount is rewritten.
- New prelisting immediate-on-listing allocations pin a project gate and merge
  by project/user/asset. Past/future plans and later rule edits cannot release
  them. Actual listing permits maturity, still subject to valid fee evidence.
  Fixed/relative locks and legacy null-gate locks retain independent maturity.
- Manual and automatic release use one transaction, shared eligibility SQL,
  asset/wallet/unlock lock order, precision validation, replay and paired ledger
  writes. Forged paid flags, nonzero not_required flags, missing wallet evidence
  and missing/wrong platform legs do not qualify. Worker errors never fabricate
  a success; broadcasts follow committed releases only.
- Admin overview, configuration and project lists distinguish planned/actual
  time. Lock read projection returns effective nullable maturity so the table,
  detail and export never present the stored source timestamp as a due release.
- Manual partial allocation still freezes 25, settles 10 for quantity 4, refunds
  15 and gates the 4 coins. Post-listing purchases (not grants) use their separate
  existing entry and immediately make immediate-on-listing coins available even
  if the configured plan is still in the future.

## Verification evidence

All database suites used the explicitly created disposable local database
`codex_newcoin_unlock_20260906_0015`, not the repository runtime environment.

- Admin new-coin suite: 16/16; user new-coin suite: 11/11.
- Unlock scanner/manual suite: 10/10 including genuine positive/zero fee,
  malformed evidence, starvation prevention, concurrent manual/worker release,
  preserved other wallet buckets, write-failure rollback, precision rejection
  and release of inactive assets after precision is restored.
- Exact migration rehearsal: 1/1 in another auto-cleaned isolated schema,
  proving event-only backfill, microseconds and unchanged historical locks.
- Rust library: 332/332; architecture: 11/11; final format, all-target/all-feature
  check and Clippy `-D warnings` passed. Clippy caught one complex test tuple,
  replaced with a named snapshot alias; final listing regression reran 1/1.
- Web: typecheck/lint; full suite 67 files / 478 tests; production policy 15;
  coverage gate 23; production build and bundle budget passed. The first highly
  concurrent full run timed out in three unrelated UI tests; rerunning the
  unchanged suite with `--maxWorkers=2` passed without extending test timeouts.
- Ego browser, real loopback API: at 1728px observed planned .250-ms input,
  no editable actual timestamp, and timestamp-free reason confirmation/cancel.
  At 1280px verified a waiting-for-listing lock, contained table scroll, detail
  null maturity, and distinct future-plan/current-actual project overview.
  Observed document overflow was zero. No browser mutation was submitted.
  Focused evidence is in `/tmp/newcoin-unlock-shots`; the prior page slice's
  broader visual matrix is not claimed as rerun here.

## Rollout contract (not executed)

1. Back up and inventory historical projects/locks and invalid legacy paid
   evidence; do not invent ledger entries, re-charge fees or relock old assets.
2. Stop old unlock scanners and temporarily close new-coin allocation, purchase
   and release traffic. Apply immutable migrations 0121/0122/0123 in sequence.
3. Switch API, scanner and Admin together, then restore traffic. **Do not run an
   old release API/scanner against newly gated locks:** old versions ignore the
   gate and would interpret the source timestamp as maturity.
4. Reconcile supply, frozen obligations, real refunds and paired release ledgers.
   Any rollback must retain listing-gate support; do not return new gated records
   to an old scanner or erase gates/timestamps to make rollback appear healthy.

## Scope and follow-up

No live deployment, real project/fund mutation or commit/push was performed.
The existing Mobile layout deletion and all earlier task changes are preserved.
No new Admin release action, user cancellation, project pause, historical fee
repair command, refund aggregation or receipt-table refund join was added.
The latest public DTO shapes are unchanged by this slice; the prior Mobile/PC
consumer release gates remain their previous slice's evidence, not a fresh run.
Disposable browser task space 7 is closed; both loopback services have stopped,
the temporary example/session files were removed, and the task database was
dropped (verified remaining schema count 0).

Implementation and applicable final gates are complete; task archival and
commit-coupled journal recording remain deferred while task code is uncommitted.
