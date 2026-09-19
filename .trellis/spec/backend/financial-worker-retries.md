# 资金后台任务重试

## 1. 范围

仅理财到期赎回、贷款逾期回收、代理佣金自动结算；调度不改变资金规则。

## 2. 接口与存储

迁移 `0129_financial_worker_retries.sql` 创建 `financial_worker_retries`，主键 `(task_kind, item_id)`。
`workers::financial_retry::claim(pool, kind, item_id, now)` 返回可选租约，`finish(pool, lease, now, outcome)` 条件回写。
任务类型为 `earn`、`loan`、`commission`；状态包含 `running`、`waiting_balance`、`waiting_source`、`failed`。

## 3. 合同

- 候选先过滤尚未到期的重试，再按 `last_attempt_at`、业务到期时间/ID 排序；未尝试项优先，不让旧失败占满固定扫描窗口。
- 租约领取在短事务中提交，五分钟到期；多个实例只能有一个成功领取当前租约。
- 等待余额/来源一分钟重试；失败从一分钟指数退避至一小时，永不使用进程内永久黑名单。
- 回写必须匹配随机租约 token，旧进程不能覆盖新领取者状态。完成后删除自己的调度记录。
- 资金终态及流水仍由既有业务事务保证幂等；租约过期不代表原业务事务已结束，禁止用租约代替行锁/终态检查。
- 业务提交后调度回写失败可安全再扫；异常时不删除业务记录、不自动退款、不修改金额口径。

## 4. 错误矩阵

余额不足：`waiting_balance`，保留 overdue；来源尚未终态：`waiting_source`；业务拒绝/瞬态错误：`failed` 有界重试；调度库不可用：整轮失败关闭，下轮恢复。持久化只存稳定分类，不存原始错误或凭证。

## 5. 正反例

正确：超过 500 个坏理财单，下一轮仍可赎回其后的健康单。基础：首次尝试无调度行，按原业务顺序扫描。错误：先 LIMIT 再用内存失败集合排除，队尾永久饥饿。

## 6. 必要测试

实库验证超过 500/1000 扫描窗口推进、故障修复后自动恢复、重启读取同一退避、并发租约和迟到回写保护，以及资金只入账一次。

## 7. 错误与正确

错误：佣金异常加入永久 `HashSet`，重启才能恢复。
正确：事务提交调度结果及下次尝试时间，候选查询过滤退避并公平排序。

## 运维

部署必须先运行新增迁移，再启动新版 API/worker。可以只读查询该表的任务、次数、分类和下次尝试时间定位积压。成功记录会清理，历史资金与审计以业务表为准。

## Admin Exception Workbench (E05)

- `GET /admin/api/v1/governance/financial-retries` requires
  `governance.financial.read`. It accepts only `task_kind` (`earn`, `loan`,
  `commission`, `seconds`), `outcome` (`ready`, `running`, `waiting_balance`,
  `waiting_source`, `failed`, `manual_review`), `limit` (1–100, default 50), and `offset`
  (0–100000). Unknown values/fields fail closed.
- Rows, filtered `total`, and outcome `counts` use one explicit repeatable-read,
  consistent-snapshot, read-only transaction. Counts are not page counts.
  GET never claims a lease, calls a worker, repairs an order or writes an audit.
- `checked_at`, `last_attempt_at`, and `next_attempt_at` use Unix milliseconds;
  source Decimal amounts remain strings. The source amount is principal for
  earn/loan, original commission amount for commissions, and stake for seconds, not an inferred
  payable/refund amount. Do not aggregate different assets.
- Safe one-to-one joins include both task kind and item ID. Missing sources
  remain visible with null evidence. Commission source IDs remain opaque
  strings, including quote IDs; never cast them into guessed business tables.
- `lease_status` is `active`, `expired`, or `none`, evaluated at snapshot time.
  Either a stored token or `running` with a future deadline is active.
  The lease token itself is never exposed to the browser or audit log.
- `POST /admin/api/v1/governance/financial-retries/{kind}/{id}/requeue`
  requires an authenticated Admin and `governance.financial.operate`, a positive
  decimal u64 ID, a trimmed 1–500-character `reason`, and `expected_version`
  from the displayed row. The permission match is exact and method-aware;
  malformed paths, extra segments and unsupported methods stay unmapped.
- `version` is a SHA-256 digest of the complete stored schedule, including
  microsecond timestamps and lease identity. Under the schedule row lock,
  compare the expected version before writing. A stale page, replayed command,
  concurrent operator or intervening worker claim/finish returns 409 without
  moving the new schedule. The worker table and claim/finish contract remain unchanged.
- Valid requeue sets only `next_attempt_at` to now, scheduling `outcome` to
  `ready`, and clears the expired token. It preserves attempt count and last
  attempt. Effective leases are always refused. Revoking the stale token fences
  late worker finish; it does not mean its business transaction has finished.
- Write `financial_retry.requeue` audit with authenticated actor, task kind,
  item ID, reason, and sanitized before/after schedule in the same transaction.
  Audit failure rolls back scheduling. Source orders, wallets, ledger and
  payout/refund rules remain untouched; only original workers may execute the
  business workflows with their existing idempotency and eligibility guards.
- The HTTP success receipt is the same schedule snapshot used by the atomic
  audit `after_json`, read while the transaction still owns the schedule lock
  and returned only after commit. It is not a later unlocked read that might
  describe another worker's lease. The Web adapter requires the same kind/ID,
  a changed version, `ready` outcome, `none` lease, and unchanged attempt count
  and last-attempt timestamp; an inconsistent receipt cannot show success or
  trigger an automatic write retry.
- The Chinese Web page is `/admin/governance/financial-retries`, with strict
  response parsing, filter reset to first page, read/operate separation,
  snapshot lease status, mandatory reason confirmation, conflict retention,
  and no automatic mutation retries. A changed row version invalidates an
  open confirmation rather than silently replacing its expected version.

### Incident Ownership and Seconds Review

- Migration `0131_financial_retry_incidents.sql` is a separate metadata table
  keyed by `(task_kind,item_id)`, not a worker schema change. Owner and deadline
  default to absent; GET never creates incident rows. Version zero in the DTO
  means no metadata exists. Setting or clearing either field increments the
  persisted version; clearing does not delete the row or reset its version.
  SQL projects `CAST(COALESCE(i.version,0) AS UNSIGNED)` explicitly: MySQL
  otherwise widens the unsigned/literal expression to DECIMAL, which must not
  be silently parsed or mistaken for an empty queue.
- List reads union the original worker queue with `seconds_contract_orders`
  where `status='manual_review'`, under the same read-only consistent snapshot.
  Seconds rows expose stored failure code/time and price evidence window, source
  order ID, user, asset and exact stake. `attempt_count`, `last_attempt_at` and
  `next_attempt_at` are null, with no worker lease. No synthetic retry is created.
- `PATCH .../financial-retries/{kind}/{id}/incident` requires
  `governance.financial.operate`, authenticated actor, reason (1–500 trimmed
  characters), `expected_version`, and explicitly supplied nullable
  `owner_admin_id` and `due_at` (Unix milliseconds). Unknown fields fail closed.
  The owner must be an active Admin, never an agent account. Dates must be
  representable in MySQL DATETIME (years 1000–9999); no deadline is invented or
  shifted based on product, amount, current time, or retry count.
- Lock the current source exception first, then its metadata. A vanished worker
  row or seconds order no longer in manual review yields 404. Compare the
  metadata version under lock before replacement. Append
  `financial_retry.incident_update` actor/reason/kind/before/after audit in the
  same transaction. Any failure rolls back metadata. Active leases do not block
  metadata edits, but every schedule/lease field remains exactly unchanged.
- Seconds rows cannot use the requeue route. Only worker kinds map to requeue
  permission. Metadata assignment never decides win/loss, settles an order,
  refunds an unknown withdrawal, calls a worker or modifies funds/business state.
- Web displays unassigned/unset/overdue/within-deadline using server `checked_at`.
  A metadata editor pins its initial version; a 409 retains the draft and reason.
  Deadline input starts blank when null. `/admin/seconds-contract/orders/:orderId`
  is an exact-order read-only deep link reusing the existing Admin detail GET,
  guarded by `seconds.orders.read`; the workbench shows it only with that scope.
  The deep-link page offers no settlement/refund controls.
- Metadata is retained when a worker finishes so audit/version history is not
  lost; completed rows are not presented as current exceptions. This workbench
  is not an archival incident search or a notification/escalation scheduler.

### Required E05 Regressions

Exact method/path permission parity, unauthenticated/non-Admin denial, invalid
filters/page/reason/version/extra fields, Decimal/source-ID preservation,
filtered totals and pagination, unchanged repeated GET, active/expired leases,
stale-command and request-replay conflicts, two concurrent requeues, stale
worker finish fencing, atomic audit rollback, exact HTTP/audit snapshot equality,
rejected inconsistent Web mutation receipts, permission-gated seconds deep links
with exact response ID checks, and unchanged source money/state,
wallet balances and ledger. Opt-in tests use
`FINANCIAL_RETRIES_TEST_DATABASE_URL` only, require a local host, create their
own disposable schema and do not inherit `DATABASE_URL`.
