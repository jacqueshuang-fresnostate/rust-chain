# 手机资金账单收支方向色最终复核

## Findings（已修复）

- 文件：`mobile/tests/wallet-ledger-classification.test.ts`
  - 问题：初始回归主要用跨越大段 SFC 的正则匹配源文本，没有真正执行页面 `directionTone()`，也没有编译 scoped CSS 证明默认 ink 与动态类的最终级联；深色主题继承的 sell token 也未被完整锁定。
  - 修复：用 SFC 解析结果定位总额的真实开始标签；从生产 script AST 提取并执行 `directionTone()`，以与 `changeType` 语义相反的正、负、零 `entry.amount` 锁定 `is-buy` / `is-sell` / `is-ink`；编译真实 scoped CSS，逐条核对默认规则、三个语义规则的顺序与 token，以及明暗主题的有效值。
- 文件：`.trellis/spec/mobile/backend-integration.md`
  - 问题：Mobile Transaction Records 合同只记录了通用正负色板，没有明确总额三态映射、权威字段、级联优先级和必需测试。
  - 修复：在现有合同内最小补充 `entry.amount -> directionTone(entry) -> is-buy/is-sell/is-ink -> buy/sell/ink token` 链路、明暗有效色值、`change_type` 排除和编译级联回归要求。

## Findings（未修复）

- 无。

## 生产路径复核

- `.ledger-row__total.numeric` 直接绑定 `directionTone(entry)`；`directionTone()` 仅将 `walletLedgerDirectionForAmount(entry.amount)` 的 `credit` / `debit` / 空方向映射为 `is-buy` / `is-sell` / `is-ink`。
- `.ledger-row__total` 的 ink 默认色和三个语义类编译后具有相同优先级，语义类在后且默认规则无 `!important`，所以非零总额不会被 ink 覆盖。
- 浅色 buy/sell/ink 分别为 `#0DBE7B` / `#FF5878` / `#111714`；深色覆盖 buy/ink 为 `#45EFAE` / `#F3F7F5`，sell 正确继承 `#FF5878`。
- 相对 `HEAD` 的生产文件只有总额标签一行增加动态 class，未变更金额文本、正负号、精度、title/ARIA、接口、筛选、分页、布局、手续费、余额或其他数字颜色。

## Verification

- 聚焦测试：`node --test --experimental-strip-types tests/wallet-ledger-classification.test.ts` 通过，17/17。
- Lint：`npm --prefix mobile run lint --if-present` 返回成功；当前 `mobile/package.json` 未配置 lint script。
- TypeCheck：`npm --prefix mobile run type-check` 与 `npm --prefix mobile run type-check:tests` 通过。
- Governance：`npm --prefix mobile run check:governance` 通过，源码尺寸与关键测试质量门禁均通过。
- Mobile release gate：`npm --prefix mobile run release:gate` 通过；全量测试 652/652，PWA/Tauri 各编译 2136 modules，PWA 预缓存 152 项，两类产物检查、Bundle 与治理门禁均通过。
- Diff：`git diff --check` 通过。
- Trellis：`python3 ./.trellis/scripts/task.py validate .trellis/tasks/09-04-mobile-wallet-ledger-income-expense-tone` 通过。
- 未执行 commit/push；未编辑 `mobile/pencil` 中的既有脏改动。
