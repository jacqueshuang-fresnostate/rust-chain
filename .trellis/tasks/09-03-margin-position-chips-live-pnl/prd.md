# 修复杠杆持仓芯片布局与收益实时刷新

## Goal

修复手机端“交易记录 → 当前仓位和资产”中仓位标签文字的视觉偏差，并让当前仓位的收益额、收益率和标记价格随着共享实时行情更新，不再停留在首次进入页面时取得的风险快照。

## What I already know

- 问题页面由 `OrdersView.vue` 和 `MarginPositionRecord.vue` 组成。
- `margin-position-record__chips` 当前使用无独立类名的 `span`，字重为 `400`；Pencil 当前仓位画板要求标签内边距 `4px 7px`、圆角 `6px`、字号 `13px`、字重 `650`。
- `OrdersView` 已通过 `market.startLiveUpdates('transaction-records-assets')` 持有共享 ticker 租约，但仓位卡收益额仍直接读取只在页面加载时请求一次的 `records.risks`。
- 后端单仓未实现盈亏公式为：`notional * directional(mark - entry) / entry`；收益率为 `unrealized_pnl / margin_amount`，计算精度为 18 位小数。
- 后端、API、数据库和下单/平仓合同无需改变；本任务只修复 Mobile 展示投影。

## Assumptions

- 共享 ticker 的 `lastPriceText` 与 `observedAt` 是本页面实时展示价格来源。
- 只有不早于服务端风险快照的合法正数 ticker 才能覆盖快照；否则继续显示最后一份权威风险快照。
- 维持保证金率和预估强平价仍以服务端风险快照为准，本任务不在客户端重建完整清算风险。

## Requirements

- 为仓位标签文字使用独立元素类名，显式约束 inline-flex、居中、单行、内容宽度和 Pencil 字重，避免宽泛 `span` 选择器或继承样式造成偏差。
- 基于 `DecimalText` 精确重建实时单仓未实现盈亏和收益率，禁止经过 IEEE-754 再参与金融计算。
- ticker 每次有效更新后，当前仓位卡的收益额、收益率和标记价格必须响应式更新。
- ticker 缺失、非法、非正数或早于风险快照时保留服务端快照；不得显示设计稿样例值。
- 不增加每帧 REST 请求，不改变现有共享 WebSocket 生命周期和租约释放逻辑。

## Acceptance Criteria

- [x] `.margin-position-record__chip` 精确使用 `4px 7px`、`6px` 圆角、`13px` 字号、`650` 字重，并显式垂直/水平居中且不换行。
- [x] 多、空、全仓/逐仓、倍数三个标签均保持内容自适应宽度，320–448px 不产生横向溢出。
- [x] 做多与做空仓位在 ticker 价格变化后得到方向正确的实时收益额和收益率。
- [x] 标记价格与同一 ticker 观测值同步更新，收益额与收益率由同一价格计算。
- [x] 旧 ticker、缺失精确价格、零/负价格和无入场价时回退到服务端风险快照。
- [x] Mobile 聚焦测试、类型检查、源码尺寸/测试质量门禁和 `git diff --check` 通过。

## Definition of Done

- Tests added/updated for exact decimal long/short PnL, freshness fallback and chip CSS contract.
- Mobile type checks and focused tests pass.
- `docs/superpowers/PROGRESS.md` records implementation and verification.
- Relevant Mobile backend-integration spec documents the live display projection boundary.

## Out of Scope

- 修改 Rust 风险计算、数据库精度或行情提供商。
- 改变强平价、维持保证金率或全仓账户清算算法。
- 重构 TradeView 的既有五秒账户对账生命周期。
- 提交独立的 `mobile/pencil/hippo-mobile-uiux.pen` 自动保存修改。

## Technical Notes

- Research: `research/current-position-live-pnl-audit.md`。
- Relevant specs: `.trellis/spec/mobile/backend-integration.md`、`.trellis/spec/mobile/pwa-and-shell.md`、`.trellis/spec/guides/code-reuse-thinking-guide.md`。
