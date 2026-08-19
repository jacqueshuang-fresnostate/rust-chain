# 优化手机端杠杆下单确认弹窗

## Goal

将 `/trade/:symbol?mode=contract` 的下单确认层从通用旧式弹窗升级为与当前 Pencil 杠杆页面及倍数、保证金模式弹层一致的专属底部确认面板，让用户在提交杠杆订单前清楚核对方向、交易对、保证金模式、杠杆、参考价格、保证金和预估名义价值，同时保持真实接口与既有下单语义不变。

## What I already know

* 当前 Pencil 选中稿为杠杆主页面 `by3G9/pKHeU`，以及倍数、保证金模式、交易对的明暗弹层 `f0L8yf/R8t0p`、`aNuw6/PKAcD`、`Crw8v/YuKtQ`。
* 选中稿的弹层语言是：底部上浮、22px 顶部圆角、顶部抓手、圆形关闭按钮、薄荷绿主操作、暖橙风险提示、明暗主题各自的中性层级。
* `TradeView.vue` 目前让现货和杠杆共用 `.confirmation-sheet`，只展示价格和金额，未展示保证金模式、杠杆、方向、保证金与风险说明，也未沿用杠杆弹层的视觉系统。
* 现有确认流程已具备遮罩关闭、Escape、Tab 循环、打开时聚焦取消、背景滚动锁和焦点恢复，必须保留。
* 杠杆下单仍调用 `placeMarginOrder`，本任务不修改后端接口、订单参数或计算口径。

## Assumptions

* 只重构杠杆确认弹窗；现货确认弹窗保持现有结构与行为，避免扩大范围。
* 市价单展示的是实时参考价格，并明确最终成交价格以撮合结果为准，不伪装为锁价报价。
* 弹窗中的所有交易值均来自当前表单、行情、产品和用户设置，不使用演示数据。

## Requirements

* 杠杆确认层通过 `Teleport` 挂到 `body`，在 320–448px 宽度下不受页面容器、Sticky Header 或路由层叠上下文影响。
* 使用专属杠杆确认面板，视觉与 Pencil 杠杆弹层一致，并支持浅色、深色、安全区和减少动态效果。
* 顶部明确显示“确认杠杆订单”、交易对图标、交易对、永续/市价语义以及做多/做空方向状态。
* 明细至少展示保证金模式、杠杆倍数、参考价格、投入保证金、预估名义价值和预估开仓数量。
* 显示市价成交与杠杆风险说明；接口失败时错误必须出现在仍打开的确认面板内。
* 保留提交防重、忙碌态不可关闭、遮罩/关闭按钮/Escape 取消、Tab 焦点循环、焦点恢复和背景滚动锁。
* 使用 Lucide 图标，不使用 Emoji；交互目标至少 44px。
* 中英文文案必须对称并通过 i18n 资源读取。

## Acceptance Criteria

* [x] 杠杆模式打开的是专属 `.contract-order-confirm` 底部面板，现货模式仍使用现有确认内容。
* [x] 面板展示 API/表单派生的方向、Logo、交易对、模式、杠杆、参考价、保证金、名义价值和预估数量。
* [x] 浅色、深色以及 320×720、390×844、448×900 均无横向溢出或底部按钮遮挡。
* [x] 遮罩、关闭按钮、Escape、Tab 循环、滚动锁、提交忙碌态和焦点恢复合同通过回归测试。
* [x] 提交失败消息在弹窗内部可见，重新提交仍使用同一真实接口。
* [x] Mobile type-check、聚焦测试、全量测试、PWA/Tauri 构建和 `git diff --check` 通过。

## Definition of Done

* Tests added/updated for structure, data truthfulness, accessibility, theme, safe area and narrow screens.
* Mobile type-check, full tests and both production build modes pass.
* Browser visual verification covers light/dark and compact phone widths.
* Progress and relevant mobile spec are updated.

## Out of Scope

* 不修改杠杆撮合、风控、强平、钱包或后端订单接口。
* 不改变现货下单确认弹窗的产品信息架构。
* 不新增或修改 Pencil 画板；生产实现复用当前已批准弹层语言。

## Technical Notes

* 主要入口：`mobile/src/views/TradeView.vue`。
* 可复用视觉与交互基线：`mobile/src/components/ContractTradeSheets.vue`、`mobile/src/core/modalDialog.ts`。
* 生产 UI 规范：`.trellis/spec/mobile/index.md`、`.trellis/spec/mobile/backend-integration.md`、`.trellis/spec/mobile/pwa-and-shell.md`。
* 设计与现状审计见 `research/pencil-contract-confirm-audit.md`。
