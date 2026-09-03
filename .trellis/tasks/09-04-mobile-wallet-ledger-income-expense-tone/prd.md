# 手机资金账单收支颜色语义

## Goal

让手机端交易记录中的资金账单总额以方向色直接表达收支：收入为绿色，支出为红色，零金额保持中性色，同时保留现有精确 DecimalText、正负号、明暗主题和布局合同。

## What I already know

- `WalletLedgerView.vue` 已通过 `walletLedgerDirectionForAmount(entry.amount)` 将正数映射为 `credit`、负数映射为 `debit`、零映射为空方向。
- 页面已有 `directionTone()`，将 `credit/debit/zero` 分别映射为 `is-buy/is-sell/is-ink`。
- 现有语义变量已经定义浅色和深色主题的正向、负向颜色；目前只有执行方向和手续费使用，`.ledger-row__total.numeric` 尚未绑定方向类。

## Requirements

1. `.ledger-row__total.numeric` 根据权威 `entry.amount` 的符号绑定现有方向色。
2. 正数收入使用 `is-buy` 绿色，负数支出使用 `is-sell` 红色，零金额使用 `is-ink` 中性色。
3. 复用现有 `directionTone()` 与语义变量，不新增硬编码颜色，不按 `change_type` 推断方向。
4. 不改变金额格式、符号、精度、标题、ARIA、账单筛选、请求或卡片布局。

## Acceptance Criteria

- [x] 总额元素绑定 `directionTone(entry)`。
- [x] 回归测试锁定真实模板绑定，执行生产 `directionTone()` 的收入/支出/零值映射，并编译 scoped CSS 核对级联和明暗 token。
- [x] Mobile Transaction Records 可执行规范已同步总额三态颜色与级联约束。
- [x] 聚焦测试、Mobile 完整 `release:gate`、`git diff --check` 和 Trellis validate 通过。
- [x] 无关脏改动保持原样。

## Definition of Done

- 实现与聚焦回归测试完成。
- 相关 Mobile 可执行规范和项目进度记录同步。
- 未经用户明确要求，不提交或推送。

## Out of Scope

- 不修改后端账本方向和金额。
- 不修改交易记录卡片布局、筛选、分页或文案。
- 不修改手续费、余额或其他数字的颜色。

## Technical Notes

- 目标视图：`mobile/src/views/WalletLedgerView.vue`。
- 聚焦测试：`mobile/tests/wallet-ledger-classification.test.ts`。
- 可执行合同：`.trellis/spec/mobile/backend-integration.md` 的 Mobile Transaction Records Read Model。
- 代码调查：[`research/repo-findings.md`](research/repo-findings.md)。

## Final Reviewer Closeout

- 生产改动仅在 `.ledger-row__total.numeric` 增加 `:class="directionTone(entry)"`；`signedAmount()`、`exactAmountTitle()`、行 ARIA、精度、请求、筛选、分页和布局均未变。
- 复核将原本跨大段源码的正则断言改为 SFC 结构定位、生产 `directionTone()` 提取执行与 scoped CSS 真实编译断言；互相矛盾的 `changeType` fixture 证明颜色只由 `entry.amount` 决定。
- 编译后 `.ledger-row__total[data-v-wallet-ledger]` 的默认 ink 规则位于等优先级的 `is-buy` / `is-sell` / `is-ink` 之前，无 `!important`；浅色有效值为 `#0DBE7B` / `#FF5878` / `#111714`，深色为 `#45EFAE` / `#FF5878` / `#F3F7F5`。
- 独立复核细节见 [`review.md`](review.md)。
