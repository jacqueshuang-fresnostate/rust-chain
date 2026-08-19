# 依据 Pencil 当前选中稿完成手机端页面

## Goal

以 `mobile/pencil/hippo-mobile-uiux.pen` 中当前选中的 102 张业务画板为视觉基线，继续补齐已设计但尚未完整落地到 `mobile/src/` 的页面和状态，同时保留真实 API、路由、认证与资金安全行为。

## Current Pencil baseline

- Pencil 文档现有 103 个顶层节点，当前选中 102 个业务画板（不含 Design System）。
- `00–37` 的主要生产页面已有对应实现；本轮重点是最近新增且已保存的画板：
  - `A9It6g` / `h4gfd`：新币记录。
  - `v6phV` / `TuWXq`：资金划转底部弹层。
  - `UouET` / `FM5tp`：帮助与客服。
  - `e5Qs1` / `hxe8l`：订单空态。
  - `Bcug6` / `IVMAO`：资金账单空态。
  - `t7j6n` / `eSMHf`：消息中心空态。
  - `CzpTv` / `ZvGMv`：预测下单确认态。
  - `nqP6W` / `aXxul`：理财申购确认态。

### 2026-08-18 current contract selection

Pencil 当前选区已经切换为 8 张杠杆合约画板，本次用户请求只交付这一切片：

- `by3G9` / `pKHeU`：`06 / Contract` 杠杆交易主页面明暗主题。
- `f0L8yf` / `R8t0p`：杠杆倍数底部弹层明暗主题。
- `aNuw6` / `PKAcD`：保证金模式底部弹层明暗主题。
- `Crw8v` / `YuKtQ`：合约交易对底部弹层明暗主题。

## Requirements

### 1. Route and navigation parity

- 新增 `/profile/help` 帮助与客服路由，定义 `depth`、`showBottomNav: false` 和回退到 `/profile` 的 `backFallback`。
- `ProfileView` 中的“帮助与客服”必须进入新帮助页，不再错跳到消息中心。
- 订单空态中的“去交易”使用持久化现货交易对进入现货路由，不合并现货、合约和秒合约。

### 2. Selected visual states

- 新币记录：声明新 Pencil 来源，对齐 44px 标签轨、72px 记录行、36px Lucide 图标盘和连续分隔线；继续消费真实认购、分发、申购、解锁接口。
- 资金划转：将现有通用确认层改为画板的底部 Sheet，包含拖拽条、来源/目标账户、交换按钮、币种、数量、真实可用余额和 50px 主动作。
- 订单空态：使用 56px 空态图标盘、标题/说明和底部主动作；不影响已有现货/杠杆真实订单列表。
- 资金账单与消息中心空态：按画板使用独立的 56px 图标盘和双行说明，保留筛选、已读、重试和真实数据行。
- 预测下单确认：弹层/确认态的市场信息、YES/NO 赔率、投入数量、潜在回报、结算说明和风险文案必须使用真实报价与配置。
- 理财申购确认：弹层/确认态对齐产品标识、申购数量、参考收益、起息/赎回规则与 50px 主动作；不伪造收益、限额或余额。
- 帮助与客服：实现画板的 Header、Hero、44px 搜索、64px FAQ/联系行、Lucide 图标和明暗主题；未配置的外部客服渠道必须显式显示为未配置，不伪造在线状态。

### 3. Runtime and accessibility contracts

- 固定文案全部通过 `vue-i18n`，中英文资源对称。
- 图标统一使用 Lucide，不使用 emoji 或内联 SVG。
- 所有交互目标至少 44×44px；支持 320–448px、safe area、深色主题、键盘焦点和 reduced motion。
- 弹层必须保留 `role="dialog"`、`aria-modal`、Escape 关闭、Tab 焦点闭环、背景滚动锁和焦点恢复。
- 缺少 API 数据时只显示 `--`、骨架、空态或明确错误，不引入 Pencil 演示金额、资产、收益、赔率和客服承诺。

### 4. Selected margin trading parity

- 杠杆交易 Header 必须按画板显示后台行情返回的资产图标、交易对、永续标签、实时涨跌幅和自选状态；点击交易对在当前页打开底部选择器，不再跳转到通用行情页。
- 主交易区保留真实订单簿、余额、仓位和市价开仓能力；后端仅支持市价委托时不得按画板伪造限价委托。
- 杠杆倍数弹层只显示当前产品 `leverageLevels`，滑轨、快捷倍数和确认按钮均写入 `/margin/settings/:product_id/leverage`，成功后再更新页面状态。
- 保证金模式弹层只显示当前产品 `marginModes`，确认后写入 `/margin/settings/:product_id/mode`；文案必须明确设置只作用于后续开仓，不伪造存量仓位迁移行为。
- 进入或切换交易对时读取 `/margin/settings/:product_id`，已保存设置必须覆盖产品默认值；404 表示用户尚未设置，应安全回落到产品配置。
- 交易对弹层只渲染 `/margin/products` 与实时行情 Store 的交集，图标、价格、涨跌幅和可交易状态全部使用真实数据，并支持搜索、自选/全部/主流筛选。

## Acceptance Criteria

- [ ] 上述 8 组新增画板都在对应生产页根节点声明 `data-pencil-source`。
- [ ] `/profile/help` 可打开，返回到“我的”，帮助入口不再进入消息中心。
- [ ] 帮助搜索可筛选 FAQ，FAQ 可展开/收起，未配置客服渠道不会导航到伪造地址。
- [ ] 划转、预测和理财弹层形态与画板一致，但仍调用现有真实 API。
- [ ] 订单、资金账单、消息中心的 loading/error/empty/data 分支均可辨识，布局无横向溢出。
- [x] 移动端相关源码合同测试、全量测试、TypeScript 类型检查和 PWA 构建通过。
- [x] 杠杆主页面及三个弹层声明当前 8 个 Pencil 来源，并在 390px 明暗主题下对齐选中画板。
- [x] 杠杆倍数、保证金模式和交易对切换均使用真实产品/行情/用户设置接口，无演示数据与虚构能力。
- [x] 三个杠杆弹层具备可访问对话框语义、焦点闭环、Escape/遮罩关闭、滚动锁、safe area 与焦点恢复。
- [x] 390×920 主页面锁定 61px Header、431px 双栏交易区、425px 表单、372px 六档盘口和 37px 仓位标签轨；不显示画板中的虚构资金费率、余额或委托。
- [x] 500px 杠杆、446px 保证金模式和 620px 交易对弹层按画板原始内容轨道排布；320×760 下长风险文案自适应增高且不裁切主操作。

## Out of scope

- 不修改后端 API 或管理后台。
- 不改动已完成的行情 WebSocket、K 线渲染器、现货/合约/秒合约业务分层。
- 不修改当前未保存到 `.pen` 文件的实验性 `41–44` 运营状态脚本。
- 不在这一轮重新设计 Pencil 画板本身。

## Definition of Done

- 生产页面、路由、双语文案与回归测试完成。
- 通过 `npm --prefix mobile run type-check`、`npm --prefix mobile test`、`npm --prefix mobile run build:pwa`、`npm --prefix mobile run build:tauri` 和 `git diff --check`。
- 使用 Ego 浏览器检查关键路由的 390px 明暗主题、空态与弹层。
- 更新 `docs/superpowers/PROGRESS.md`。
