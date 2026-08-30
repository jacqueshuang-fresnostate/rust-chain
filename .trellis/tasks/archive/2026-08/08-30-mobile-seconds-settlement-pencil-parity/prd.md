# 手机端秒合约结算弹窗对齐 Pencil 选稿

## Goal

依据 Pencil 当前选中的 `07e / 秒合约 · 盈亏结算弹窗 · 浅色主题` (`tFcTH`) 与深色主题 (`FBdqS`)，1:1 重构生产 `/seconds` 的结算结果弹窗，同时保留后端权威结果、FIFO 多结算队列、历史订单跳转和可访问性。

## What I already know

- Pencil 明暗选稿均为 390×920，弹窗为 `x=16 / y=176 / width=358 / height=541`。
- 选稿为带全屏遮罩的模态弹窗，不是当前页面顶部的非模态毛玻璃通知岛。
- 弹窗顺序为：已结算状态/关闭、盈亏结果、买入与结算价对比、交易对/方向/周期、自动结算说明、查看历史订单。
- 生产已有订单结果追踪器和 FIFO 队列，数据必须继续只来自 `/seconds-contracts/orders` 的终态快照。
- 买入价与结算价必须使用订单的 `entryPrice` / `settlementPrice`，不得使用实时行情补值。

## Requirements

- Teleport 结算层使用全屏遮罩、居中上移 13.5px 的 358px 卡片，390×920 基线精确落在 `16,176,358,541`。
- 明暗主题颜色通过全局选中页主题变量映射，不在组件中建立第二份主题状态。
- 顶部显示 Lucide `CircleCheckBig` 已结算标签和 34px 可视关闭面；关闭按钮保留至少 44px 实际触控区。
- 结果区显示 Lucide `BadgeDollarSign`、“本单结算盈利/亏损”、带符号净盈亏和以本金为分母的实际收益率。
- 价格对比区显示权威买入价和结算价；缺失字段显示 `--`。
- 订单摘要显示格式化交易对、方向和周期；结算说明显示本金、结算资产与自动结算语义。
- 可见操作仅保留 52px “查看历史订单”主按钮；关闭用于继续交易/展示队列下一笔。
- 弹窗使用 `role=dialog`、`aria-modal=true`、初始焦点、Tab 循环、Escape/遮罩关闭、body 滚动锁定与焦点归还。
- 多笔同时结算仍按到期时间和订单 ID 维持 FIFO，队列数量仅作读屏提示，不破坏 Pencil 可见布局。
- 320–448px 不横向溢出；短屏可纵向滚动；`prefers-reduced-motion` 禁用入场位移和缩放。

## Acceptance Criteria

- [x] 390×920 中遮罩、卡片位置与 541px 内容节奏对齐 Pencil。
- [x] 浅色与深色主题的卡片、边框、阴影、文字、语义色与 Pencil 节点一致。
- [x] 盈利与亏损均正确显示净额、收益率、方向和订单价格。
- [x] 结算价缺失时不使用最新行情或 K 线价格填充。
- [x] 关闭、Escape、遮罩和历史订单跳转行为正确，多笔结算不丢失。
- [x] i18n、键盘焦点、安全区、320/390/448px 和 reduced-motion 回归通过。
- [x] Mobile 类型检查、相关测试和全量测试通过。

## Definition of Done

- 代码、i18n、测试和 Mobile 规范同步。
- 使用 Ego Browser 在实际 Vue 页面中校验 390×920 明暗截图与关键几何。
- 不覆盖或回滚工作树中无关的未提交改动。

## Decision (ADR-lite)

**Context**：旧实现是页头下方的非模态毛玻璃卡，与当前 Pencil 全屏遮罩、中央结算弹窗的交互语义与视觉均不一致。

**Decision**：以 Pencil 节点为唯一可见合同，将结果层升级为可访问模态弹窗；保留现有后端追踪/FIFO 数据流，不更改订单判定逻辑。

**Consequences**：弹窗显示期间将锁定背景滚动与焦点；第二笔及以后的结果通过关闭按 FIFO 继续展示。

## Out of Scope

- 不修改后端结算规则、资金入账、worker 调度或订单 API。
- 不修改 Pencil 画板本身。
- 不重构秒合约下单确认弹窗和主页其他区域。

## Technical Notes

- Pencil 取证：`research/pencil-settlement-dialog.md`。
- 主实现：`mobile/src/views/SecondsView.vue`。
- Teleport 主题边界：`mobile/src/styles/pencil-selected-pages.css`。
- 模态焦点与滚动锁：`mobile/src/core/modalDialog.ts`。
- 回归：`mobile/tests/seconds-live-multi-orders.test.ts`、`mobile/tests/award-ui-trading-workspaces.test.ts`。
