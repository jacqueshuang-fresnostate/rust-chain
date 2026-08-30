# 手机端秒合约交易对选择对齐 Pencil

## Goal

依据 Pencil 当前选中的 `07c / 秒合约 · 交易对选择 · 浅色主题`（`vONcc`）与深色主题（`kLXCs`），将 `/seconds` Header 中的原生透明 `select` 重构为 1:1 的可访问交易对选择弹窗，同时保留真实秒合约产品、实时行情、后台 Logo、当前订单和既有选品业务行为。

## What I already know

- 选稿基线为 390×920；遮罩覆盖全屏，选择面板从 `y=80` 开始，尺寸为 `390×840`。
- 当前生产页面使用 Header 内透明原生 `select`，不符合选稿的全屏遮罩、搜索、行情行和当前选中状态。
- 产品来自 `fetchSecondsProducts()`；价格可复用页面已有的产品级实时 ticker 会话及 `marketStore` 快照。
- Logo 必须使用后台行情资产图片，按 `baseIconUrl -> iconUrl -> AssetMark` 字母回退处理，不生成伪造图片。
- 选中产品必须继续走现有 `selectProduct()`，重置该产品周期/金额并切换 K 线；活动订单按各自交易对保留，不因选择器切换被改写。

## Requirements

- Header 中央改为 44px 可点击对话框触发器，保留当前交易对、秒合约标签和 ChevronDown，并提供 `aria-haspopup`、`aria-expanded`、`aria-controls`。
- 选择器 Teleport 到 `body`，复用 `useModalDialog` 完成初始搜索焦点、Tab 循环、Escape/遮罩关闭、body 滚动锁定及精确焦点归还。
- 390×920 基线：面板 `x=0/y=80/w=390/h=840`，顶部圆角 24，内边距 `18 20 16`，内容间距 14。
- 标题行高度 34；标题 20/700；关闭可见面 34×34，并保留至少 44×44 触控区。
- 搜索框为 350×46、圆角 12、左右内边距 14，使用 Lucide Search；可按交易对、基础币或结算币不区分大小写过滤。
- 产品行高 64、间距 8、圆角 12；显示 30px 后台 Logo、格式化交易对、实时/快照最新价；当前项显示 Pencil 选中面和 Lucide Check。
- 无匹配产品、加载中和产品为空时显示本地化诚实状态，不生成静态 BTC/ETH/HIPPO 行。
- 底部说明使用本地化文案“选择后立即切换行情，当前订单不会受到影响。”。
- 明暗主题精确采用 Pencil 色板，并在 320–448px、安全区、短屏、长列表和 reduced-motion 下保持可用。

## Acceptance Criteria

- [x] 390×920 浅色和深色选择器的遮罩、面板、标题、搜索框、64px 行及说明几何与 Pencil 一致。
- [x] 触发器、搜索、关闭、Escape、遮罩、Tab/Shift+Tab、焦点归还和滚动锁均正常。
- [x] 产品、Logo、价格和选中态均来自真实 API/store；缺失价格显示 `--`。
- [x] 搜索与空态使用 i18n；模板没有固定中英文文案。
- [x] 选择产品后关闭弹窗并切换当前行情/K 线，既有活动订单不被修改。
- [x] 320/390/448px 无横向溢出，长列表可滚动，低动态禁用位移动效。
- [x] 相关测试、Mobile 全量测试、类型检查、PWA/Tauri 构建通过。

## Definition of Done

- 生产模板、状态、主题、i18n、测试与 Mobile 规范同步。
- 使用 Ego Browser 在实际 Vue 页面中校验 390×920 明暗视觉与搜索/选择/键盘交互。
- 不覆盖或回滚工作树中无关未提交改动。

## Decision (ADR-lite)

**Context**：原生透明 `select` 无法承载 Pencil 选稿的搜索、Logo、实时价格、当前项和完整模态语义。

**Decision**：使用页面内产品与行情数据构建 body-Teleported 自定义模态选择器，复用共享 `useModalDialog`；不新增后端接口，不复制独立行情订阅。

**Consequences**：选择器可与 Pencil 1:1 对齐并改善可访问性；需维护搜索过滤、长列表滚动和焦点生命周期，但仍由现有产品与行情源保持数据权威。

## Out of Scope

- 不修改后端秒合约产品、结算、下单或行情协议。
- 不修改 Pencil 画板。
- 不重构秒合约其他表单、确认弹窗、结算弹窗和历史订单页。

## Technical Notes

- Pencil 取证：`research/pencil-seconds-pair-picker.md`。
- 主实现：`mobile/src/views/SecondsView.vue`。
- Teleport 明暗主题：`mobile/src/styles/pencil-selected-pages.css`。
- 共享模态：`mobile/src/core/modalDialog.ts`。
- 现有权威数据：`fetchSecondsProducts()`、页面 `liveTickerSnapshots`、`marketStore.tickerFor()`。
