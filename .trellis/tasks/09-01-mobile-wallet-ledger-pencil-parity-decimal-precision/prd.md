# 手机交易记录 kcP5D/A85if 复刻与账本精度合同

## 用户更正（2026-09-02）

此前以 `y6Y7TW/m25xr0` 为视觉真值的“资金账单”目标已被用户明确更正并废止。旧目标中的 60px Header、三个 28px 胶囊筛选和 56px 紧凑流水行不再构成本任务验收依据，也不得保留其已完成勾选作为新验收证据。

本轮视觉与文案唯一权威是 Pencil `mobile/pencil/hippo-mobile-uiux.pen` 的 `kcP5D`（浅色）和 `A85if`（深色）。此前已完成的真实账本分页、服务端筛选、鉴权、DecimalText、`precision_scale`、错误状态和请求代际隔离继续有效，本轮只更正 Mobile 页面、入口文案、i18n、聚焦测试和任务文档，不改 Admin、PC、后端资金逻辑或 `OrdersView`。

## Goal

将手机端 `/assets/ledger` 从“资金账单”更正为“交易记录”（英文 `Transaction Records`），按 `kcP5D/A85if` 1:1 重构页面可见结构，同时只展示真实 API/资产元数据并保留现有账本行为合同。

## Requirements

### Header 与安全区

- 页面本身不绘制 Pencil 的 28px 系统状态栏；继续由现有 `.page` 安全区处理顶部 inset。
- Header 内容区固定 58px，左右 padding 16px，轨道为 `26px minmax(0, 1fr) 26px`。
- 左侧使用 Lucide `chevron-left`，视觉框 26×26，实际点击目标至少 44×44；不得修改其他页面共享 `PageHeader` 的默认返回图标。
- 标题居中，中文“交易记录”、英文 `Transaction Records`，字号 22px、字重 700；右侧保留 26×26 空占位。
- 返回行为继续通过 `goBackOr(router, route.meta.backFallback || { name: 'assets' })`。

### 四栏导航与筛选

- Header 下方四栏固定高 52px：历史仓位、交易账单（active）、当前策略、历史策略；文本 13px，active 下划线 3px、`#18D38D`。
- 四栏必须指向现有真实页面，不创建死链接：历史仓位 → `orders?tab=positions`，交易账单 → `wallet-ledger`，当前策略 → `orders?tab=margin`，历史策略 → `orders?tab=history`。
- 不改 `OrdersView`；仅使用其现有 `positions | margin | history` 查询参数合同。
- 筛选条固定高 58px、左右 padding 16px、gap 24px；可见项为“币种”下拉、“交易类型”下拉和右侧 Lucide `list-filter` 24px。
- 复用现有资产、收支方向、日期三个可访问 Sheet；所有触发器点击目标至少 44px，仍保留遮罩、Escape、焦点陷阱和关闭后焦点恢复。

### 连续交易记录行

- 列表从筛选条后连续开始；每行固定 166px，无卡片、无日期分组，仅有 bottom 1px 分隔线。
- 行 padding 为 `12px 18px`，纵向 gap 9px；四行网格分别为 30px、22px、22px、19px，对应内容 y=12/51/82/113。
- 第一行：真实资产 logo/既有 `AssetMark` 元数据回退，30px；symbol 20/650；右侧带符号总额 18/500。
- 第二行：左侧 15/600 使用现有本地化 `entryLabel(entry)` 展示真实交易事项；数量标签 13/500 muted、数量 15/500。当前账本 API 没有交易对字段，不得拼出演示交易对，也不得让全部真实记录永久显示 `--`。
- 第三行：真实账户类型 14/600；中间 15/650 必须按 amount 符号展示现有本地化“收入/支出”，零值显示 `--`，不得再次重复交易事项或把它伪装成买入/卖出 side；成交元信息与手续费 12/500。
- 后端 fee 是非负手续费合同；可见费用按扣除语义展示，非零值在 DecimalText 前加 `-`，零保持 `0`，不得经过浮点数转换。
- 第四行：本地时间 13/400；余额标签 13/500、余额值 14/500。
- 金额、手续费、余额继续走 DecimalText 与资产 `precision_scale`，不得经过 `Number`、`parseFloat` 或 `toFixed`；长值在 320px 下截断可见文本但保留精确 title/可访问描述。
- 任何 API 缺失字段使用语义化 `--`，不复制 Pencil 演示资产、金额、时间或交易状态。

### 主题与响应式

- 浅色：背景 `#FFFFFF`、主字 `#111714`、tab muted `#7B8680`、行 muted `#8A948F`、tab 底线 `#EEF1EF`、行底线 `#EDF1EF`、active `#18D38D`、买入/正向 `#0DBE7B`、卖出/负向 `#FF5878`。
- 深色：背景 `#000000`、主字 `#F3F7F5`、tab/行 muted `#8F9B94`、tab 底线 `#18231D`、行底线 `#17221C`、active `#18D38D`、买入/正向 `#45EFAE`、卖出/负向 `#FF5878`。
- 320px、390px、448px 均不得出现页面级横向溢出；使用 Lucide-only，不新增 emoji 或手写 SVG。

### 既有生产行为

- 保留真实账本 offset 分页、资产/方向/日期服务端筛选、登录态、首次加载、空态、首屏错误、已有数据追加错误、重试与加载更多。
- 保留 session generation、filter generation、请求单飞、陈旧响应丢弃和原 offset 重试合同。
- 资产 logo 只来自 `fetchWalletAccounts()` 返回的后台 URL/已有资产元数据；请求结果必须经过 token 与 generation 校验后才能写入。
- 本轮不改账本 API、后端金额/余额/手续费、数据库或资金结算行为。

## Acceptance Criteria

- [x] `/assets/ledger` 源码声明 `kcP5D A85if`，不再把 `y6Y7TW/m25xr0` 当作该页合同。
- [x] 专用 Header 达到 58px、16px、26px chevron、22/700 标题和 44px 返回点击目标，且返回仍走 `goBackOr`。
- [x] 52px 四栏导航与 58px 筛选条按权威几何实现，四个入口全部映射到现有有效路由。
- [x] 连续流水行达到 166px、`12px 18px`、9px gap、四层固定高度和仅 bottom 1px 分隔线。
- [x] 入口、路由标题和页面状态文案均改为“交易记录”/`Transaction Records`，中英文键对称。
- [x] 页面消费真实账本与资产 logo 元数据；第二行显示真实本地化交易事项，第三行显示 amount 对应的收入/支出，非零 fee 以负号呈现，未写入 Pencil 演示数据。
- [x] 既有分页、三筛选 Sheet、鉴权、DecimalText、错误/空/加载和 session/filter generation 隔离继续由聚焦测试覆盖。
- [x] Mobile 聚焦测试 5 文件 44/44 与应用/测试类型检查通过，并以行为测试覆盖资产目录乱序、token/generation、退出和卸载隔离。
- [x] 主会话完成 320/390/448px 浅色/深色浏览器视觉复核，确认无横向溢出及安全区重复；最终 390px 复核同时确认默认日期为“全部日期”、Sheet 滚动锁和焦点恢复。
- [x] 主会话完成最终 `npm --prefix mobile run release:gate`：617/617、PWA/Tauri 双构建、制品与全部质量预算通过。

## Definition of Done

- Mobile 页面、入口文案、i18n 和聚焦源码/行为合同完成。
- 当前 PRD/research 明确记录用户更正，不继续使用旧画板的错误验收。
- `docs/superpowers/PROGRESS.md` 记录每个交付切片及验证结果。
- 最终浏览器视觉复核和发布门禁已由主会话收口。

## Decision (ADR-lite)

**Context**：旧 `y6Y7TW/m25xr0` 画板描述的是另一套紧凑资金流水页面，和用户指定的 `kcP5D/A85if` 交易记录页在标题、Header、顶栏、筛选和行高上均不一致；账本 API 不提供交易对或交易 side，但有真实变动类型、amount 方向和非负 fee。

**Decision**：废止旧视觉验收，以 `kcP5D/A85if` 为唯一视觉真值；在本页实现专用 Header，不改变共享 `PageHeader`；四栏复用现有 Orders 查询参数；第二行用本地化变动类型作为真实交易事项，第三行用 amount 符号映射收入/支出，非零 fee 转成 DecimalText 扣除展示。

**Consequences**：页面与新 Pencil 合同一致且不会影响其他 Header/Orders 页面；生产数据不会伪造交易对或买卖方向，也不会让第二行永久空白。已有账本精度与并发安全合同保持不变。

## Out of Scope

- 重构 `mobile/src/views/OrdersView.vue` 或修改它现有紧凑 64px 布局。
- 修改 Admin、PC、后端钱包、账本 SQL、资金结算或数据库结构。
- 为追求演示图一致而伪造交易对、买卖 side、时间、金额、手续费或余额。
- 修改其他页面共享 `PageHeader` 的默认 ArrowLeft 22px 行为。
- 在本轮提交或推送代码。

## Technical Notes

- Pencil：`mobile/pencil/hippo-mobile-uiux.pen`，浅色 `kcP5D`、深色 `A85if`。
- Mobile：`mobile/src/views/WalletLedgerView.vue`、`mobile/src/styles/pencil-selected-pages.css`、`mobile/src/i18n/messages/{zh-CN,en}.ts`。
- 聚焦合同：`mobile/tests/wallet-ledger-classification.test.ts`、`mobile/tests/pencil-wallet-flow-parity.test.ts`、`mobile/tests/pencil-selected-page-parity-20260807.test.ts`，并回归钱包二级页与路由可访问性测试。
- 已完成但本轮未改的精度链路：`mobile/src/{api/wallet,core/walletLedger}.ts` 与后端钱包 `precision_scale` 合同。
