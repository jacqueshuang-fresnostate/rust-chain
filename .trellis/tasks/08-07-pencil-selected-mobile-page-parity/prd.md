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

## Acceptance Criteria

- [ ] 上述 8 组新增画板都在对应生产页根节点声明 `data-pencil-source`。
- [ ] `/profile/help` 可打开，返回到“我的”，帮助入口不再进入消息中心。
- [ ] 帮助搜索可筛选 FAQ，FAQ 可展开/收起，未配置客服渠道不会导航到伪造地址。
- [ ] 划转、预测和理财弹层形态与画板一致，但仍调用现有真实 API。
- [ ] 订单、资金账单、消息中心的 loading/error/empty/data 分支均可辨识，布局无横向溢出。
- [ ] 移动端相关源码合同测试、全量测试、TypeScript 类型检查和 PWA 构建通过。

## Out of scope

- 不修改后端 API 或管理后台。
- 不改动已完成的行情 WebSocket、K 线渲染器、现货/合约/秒合约业务分层。
- 不修改当前未保存到 `.pen` 文件的实验性 `41–44` 运营状态脚本。
- 不在这一轮重新设计 Pencil 画板本身。

## Definition of Done

- 生产页面、路由、双语文案与回归测试完成。
- 通过 `npm --prefix mobile run type-check`、`npm --prefix mobile test`、`npm --prefix mobile run build:pwa` 和 `git diff --check`。
- 使用 Ego 浏览器检查关键路由的 390px 明暗主题、空态与弹层。
- 更新 `docs/superpowers/PROGRESS.md`。
