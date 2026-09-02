# 统一后台与手机端金融数字显示精度

## Goal

修复后台与手机端把 `DECIMAL(38,18)` 或计算中间值直接展示给用户的问题。类似
`1,134.331253942506787192 USDT` 的值不是整数溢出，而是存储/计算精度泄漏到展示层；
本任务只收敛可见格式，不降低 API、计算、下单、结算、账本和数据库中的十进制精度。

## What I Already Know

- 后端金额以 `BigDecimal`/十进制字符串传输，部分收益报表固定序列化 18 位，这是计算和审计合同，不应改成浮点数。
- Mobile `todayReturnPresentation.ts`、首页收益历史以及若干资金页面显式使用
  `maximumFractionDigits: 18`；交易/秒合约展示绑定器的默认值也是 18。
- Admin `formatAdminNumber` 的声明格式是 `0,0.00[0000]`，但当前实现会保留服务端全部小数位，导致通用表格与详情抽屉可显示 18 位。
- 钱包账单已经返回权威 `precision_scale`，但该字段是业务/存储精度，不等同于面向用户的最佳显示位数。
- 用户此前明确要求主要完善 Admin 与移动端，PC 不在本轮范围。

## Requirements

1. 所有格式化继续直接处理十进制字符串/`DecimalText`，禁止通过 JavaScript `Number` 承接金融值。
2. 新增共享“显示精度”策略，明确区分存储精度、业务输入精度和用户可见精度：
   - USDT、USDC、USD 及常见法币金额最多显示 2 位；
   - 其他资产数量最多显示 8 位，若接口提供更低的合法精度则服从更低值；
   - 未知资产/通用 Admin 金融字段默认保留 2 位、最多 6 位；
   - 百分比默认最多 2 位，利率等明确业务字段可以显式请求最多 4 位；
   - 行情价格继续服从交易对 `price_precision` 或既有价格专用格式，不套用余额规则。
3. 超出显示位数时采用确定性的十进制四舍五入，禁止科学计数法；负零必须归一为零。
4. 非零但小于最小可见单位的值显示为阈值文案（如 `<0.00000001` / `>-0.00000001`），不得误显示为真实零。
5. Mobile 收敛今日收益、收益历史摘要、钱包/账单、提现、借贷、理财、新币、快速充值、交易和秒合约中会泄漏 18 位小数的可见金额。
6. Admin 的通用金额、详情字段和资源表格按统一默认格式显示；显式资产精度仍可用于更低上限，但不得把 18 位存储精度原样铺到表格。CSV/API 原值保持不变。
7. 输入框、请求载荷、后端响应、账本记录和结算计算保持原有精确字符串，不因展示格式发生截断或覆盖。

## Acceptance Criteria

- [x] `1,134.331253942506787192 USDT` 在 Mobile 今日收益/资产摘要中显示为 `1,134.33 USDT`。
- [x] Admin 通用值 `70000.123456789012345678` 最多显示为六位小数，USDT 金额最多两位。
- [x] BTC 等非稳定币数量最多八位，并支持由更低 `precision_scale` 收紧。
- [x] 极小非零值不会显示为 `0`，而以明确阈值形式呈现。
- [x] 至少覆盖正数、负数、负零、进位、极大整数、18 位输入、科学计数输入（Admin 兼容边界）和非法输入测试。
- [x] Mobile 与 Admin 全量类型检查、测试、构建/预算门禁通过；`git diff --check` 通过。
- [x] 不修改 PC，不把任一金融计算或请求载荷转换成 `Number`。

## Definition of Done

- 新增/更新共享格式化单元测试与受影响页面合同测试。
- Mobile `release:gate` 与 Admin 质量门禁通过。
- 规范和 `docs/superpowers/PROGRESS.md` 记录新的显示精度边界。
- 本轮不自动提交或推送，等待用户明确指令。

## Technical Approach

- 在 Mobile 精确十进制层新增资产金额展示 helper，内部复用 `DecimalText` 解析并实现字符串级四舍五入、分组和阈值文案。
- 在 Admin `shared/decimal.ts`/`numberFormat.ts` 将默认无限保留改为显式最大显示位数，并让 `AmountText` 根据资产符号选择显示上限；原始导出与 API DTO 不变。
- 逐项替换仅用于展示的 `maximumFractionDigits: 18`，输入校验中的 `maxScale: 18` 保持不动。
- 对行情价格、汇率计算和提交快照只改最终文本渲染，不改权威原值。

## Decision (ADR-lite)

**Context**：`precision_scale` 描述资产可存储/可提交的小数位，直接用作 UI 展示会把 18 位中间精度暴露给用户。

**Decision**：建立独立的展示上限；金融值在计算和传输层保持原始 Decimal，只有最终渲染文本执行字符串级舍入和阈值表达。

**Consequences**：界面数字稳定易读；审计/导出仍保留原值；极小非零值不会被误报为零。显示文本不得反向用于请求载荷。

## Out of Scope

- 修改数据库 `DECIMAL(38,18)` 列或批量改写历史余额。
- 修改资产业务精度、订单撮合、结算、手续费和收益计算公式。
- PC 前端视觉重构。

## Technical Notes

- 相关规范：`.trellis/spec/backend/wallet-amount-precision.md`、`.trellis/spec/mobile/backend-integration.md`、`.trellis/spec/admin/resource-response-contract.md`。
- 研究文件：`research/display-precision-audit.md`。

## Verification Evidence

- Mobile `release:gate` 全链路通过：类型检查、全量测试、PWA/Tauri 构建、产物、Bundle、源码尺寸与关键测试质量门禁均为绿色。
- Admin `typecheck`、`lint`、全量测试（61 文件 / 437 项）、生产策略、覆盖率、构建与 Bundle 预算全部通过。
- 聚焦回归覆盖今日收益、账本、交易/秒合约、正负舍入、进位、阈值、极大整数、科学计数与 Admin 资产推断；CSV 测试继续断言原始十进制文本。
- `git diff --check` 与 Trellis implement/check 上下文校验通过。
