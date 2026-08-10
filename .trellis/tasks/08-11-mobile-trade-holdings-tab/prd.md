# 手机现货交易将持有资产归入持仓栏目

## Goal

修正手机端 `/trade/:symbol` 现货账户区的信息归属：当前展示的现货钱包持有资产必须位于激活的“持仓”栏目下，“委托”只作为进入当前委托页面的入口，不再呈现为持有资产列表的激活栏目。

## What I Already Know

- `mobile/src/views/TradeView.vue` 的 `spot-account-workspace` 当前将“委托”按钮硬编码为激活态，但下方实际渲染的是 `fetchWalletAccounts()` 返回的 `spotVisibleBalances`。
- 同一区域还显示“全部撤单”，但页面没有加载当前委托数据；真正的当前委托、历史委托和撤单能力均在 `/orders` 的 `OrdersView`。
- 第二个按钮当前标为“仓位和资产”并跳转到 `/orders?tab=positions`，该目标是合约仓位，不代表当前现货钱包持有资产。
- 账户区原型总高度由 48px 栏目行、34px 上下文行和至少 198px 状态/列表构成，应保持现有几何稳定。

## Requirements

- 现货账户区将“持仓”设为当前激活栏目，并使用已有 `orders.positions` 中英文文案。
- 当前持有资产列表、加载、错误、空态及资产操作均结构化归属到持仓面板。
- “委托”不再显示激活态；点击后继续进入 `/orders?tab=spot` 查看真实当前委托。
- 持仓栏目在当前 `/trade` 页面不跳转到合约仓位页，也不调用 `openOrders('positions')`。
- 历史按钮继续进入 `/orders?tab=history`。
- 将错误的“全部撤单”上下文操作替换为持仓语义：保留“只看当前交易对”提示，并通过“查看全部”进入资产页。
- 钱包筛选仍仅显示当前交易对的 base/quote 且总余额大于零，不修改钱包 API、余额计算或下单逻辑。
- 保持现有 281px 账户区几何、44px 触控、双主题、320–448px 窄屏和 Lucide 图标合同。

## Acceptance Criteria

- [x] `/trade` 中持仓栏目具有可见激活态和可访问当前态，持有资产位于其关联面板。
- [x] 委托按钮无激活态并进入 `/orders?tab=spot`。
- [x] 现货持仓按钮不进入 `/orders?tab=positions`。
- [x] 现货账户区不再显示“全部撤单”，持仓上下文行可进入全部资产页。
- [x] 历史订单入口和现有钱包筛选/状态分支保持不变。
- [x] 聚焦测试、Mobile 全量测试、type-check、PWA/Tauri build 通过。

## Definition of Done

- `TradeView` 现货账户区语义、结构和样式完成。
- 回归测试能证明持仓/委托归属且不通过整体快照替换掩盖无关变化。
- Mobile 可执行规范和 `docs/superpowers/PROGRESS.md` 更新。

## Technical Approach

保留独立 Orders 页面作为委托事实源，在 `TradeView` 只展示当前交易对的钱包持仓摘要。将账户区第二项改为当前持仓栏目，第一项继续作为显式导航；把原订单过滤行改为持仓上下文行，并用带 `aria-labelledby` 的持仓面板包裹钱包所有状态。现有 Pencil parity digest 仅对本次账户区差异做定向归一化，再继续校验旧 digest。

## Decision (ADR-lite)

**Decision**: `/trade` 不复制订单列表或撤单状态；委托入口跳转到权威 `/orders`，现货页账户区专注持仓摘要。

**Consequences**: 持有资产不会再被误标为委托；撤单操作只出现在真实加载委托数据的页面；现货页面保持轻量且不新增后端请求。

## Out of Scope

- 不在 `/trade` 内新增完整委托列表、撤单接口或订单轮询。
- 不修改 `/orders` 的现货/杠杆/历史/持仓结构。
- 不修改合约交易分支、钱包 API、余额金额或下单逻辑。
- 不调整盘口、K 线、订单类型弹窗或其他交易表单样式。

## Technical Notes

- 主要实现：`mobile/src/views/TradeView.vue`。
- 聚焦测试：`mobile/tests/spot-trading-ui-optimization.test.ts`、`mobile/tests/pencil-trading-product-selected-parity.test.ts`。
- 现有权威委托页：`mobile/src/views/OrdersView.vue`。
