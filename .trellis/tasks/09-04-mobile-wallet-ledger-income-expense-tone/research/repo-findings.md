# Repository Findings

- `WalletLedgerView.vue` 的 `signedAmount()` 保留权威正负号与 DecimalText 显示；颜色不应进入该格式化函数。
- `walletLedgerDirectionForAmount()` 已由账本分类测试覆盖：正值为 `credit`、负值为 `debit`、零值无方向。
- `directionTone()` 已将 `credit -> is-buy`、`debit -> is-sell`、零 -> `is-ink`，并被同一卡片的执行方向复用。
- `.is-buy`、`.is-sell`、`.is-ink` 分别引用 `--wallet-record-buy`、`--wallet-record-sell`、`--wallet-record-ink`；现有主题合同规定正向绿色、负向红色。
- 最小实现是在 `.ledger-row__total.numeric` 上绑定 `:class="directionTone(entry)"`，并在现有 source-contract 测试中增加结构与语义色断言。
