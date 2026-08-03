# 2026-08-04 Pencil 当前选中页面生产差异

## 设计源与审计方式

- 活动设计文件：`mobile/pencil/hippo-mobile-uiux.pen`。
- 读取方式：Pencil MCP `get_app_state`、`execute`、`export_nodes`、`export_html`；未直接读取 `.pen` 文件。
- 当前一次选中 84 个 390×920 顶层画板，覆盖明暗主题、访客/会员、现货/杠杆订单、秒合约默认/进行中与邀请访客/会员状态。
- Pencil 画板包含 28px 系统状态栏；生产 Web 内容视口为 390×892，因此应用内容坐标统一为设计坐标 `y - 28px`，原生 Android 状态栏补齐该 28px。
- 设计联系表与精确 HTML：
  - `research/pencil-selected-20260804/home-news.jpg`
  - `research/pencil-selected-20260804/trading.jpg`
  - `research/pencil-selected-20260804/products.jpg`
  - `research/pencil-selected-20260804/wallet.jpg`
  - `research/pencil-selected-20260804/account.jpg`
  - `research/pencil-selected-20260804/wallet-light.html`
  - `research/pencil-selected-20260804/account-light.html`
  - `research/pencil-selected-20260804/trading-products-light.html`
- 2026-08-04 生产基线联系表：`research/pencil-selected-20260804/runtime-baseline-light.jpg`。

## 已完成且需要保持的画板映射

- 首页：`FwNBM`、`W1cWyh`、`miHnt`、`CvipW`。
- 行情：`KB7ag`、`hlr9Y`；行情详情：`ftTny`、`VoZfE`。
- 现货：`yzOPc`、`bo8k5`。
- 资产：`CUK3y`、`i6YDBr`；我的：`dUqOS`、`duJTW`、`S23rM`、`S0Bj8`。
- 订单：`kcP5D`、`A85if`、`n6oGO`、`t2GTW4`。
- 登录/注册：`u99Fpg`、`WNbsc`、`MCuqb`、`RGYGj`。
- 资讯/详情：`VGPW0`、`b6EGF`、`Q50Rgr`、`ASvmq`。
- 闪兑/币种选择：`x9T4CL`、`eXdnN`、`sf288`、`xvVss`。
- 理财、借贷、新币与详情：`zIzOm`、`tCHZ9`、`kIOBX`、`yrsRy`、`oOJ0q`、`ZTtvY`、`nFwYy`、`B6Qh9J`。

这些页面只做回归检查，不重新改写真实 API、WebSocket、状态机或已通过的坐标合同。

## 本轮必须补齐的选中画板

### 交易和产品

| 页面 | Light | Dark | 生产路由/文件 |
| --- | --- | --- | --- |
| 合约 | `by3G9` | `pKHeU` | `/trade/:symbol?mode=contract` / `TradeView.vue` |
| 秒合约默认 | `VL8er` | `g9agt` | `/seconds` / `SecondsView.vue` |
| 秒合约进行中 | `Lpt6q` | `WxeB8` | 同上，真实订单状态驱动 |
| 产品中心 | `Z0B0N6` | `zMsKE` | `/products` / `ProductHubView.vue` |
| 预测市场 | `pU7Kz` | `IcvzQ` | `/products/prediction` / `PredictionView.vue` |

### 消息和钱包

| 页面 | Light | Dark | 生产路由/文件 |
| --- | --- | --- | --- |
| 消息中心 | `FkZ6j` | `bRz9K` | `/messages` / `MessageCenterView.vue` |
| 充币资产 | `fNXT7` | `n5jiPN` | `/assets/deposit` / `DepositAssetView.vue` |
| 充币网络 | `y4ifR` | `qKfsZ` | `/assets/deposit/:asset/networks` / `DepositNetworkView.vue` |
| 充币地址 | `w5htG` | `TCN5A` | `/assets/deposit/:asset/:network` / `DepositDetailView.vue` |
| 提币资产 | `NGBmq` | `h0WWYC` | `/assets/withdraw` / `WithdrawAssetView.vue` |
| 提币表单 | `Qa9dW` | `o8Wsh` | `/assets/withdraw/:asset` / `WithdrawView.vue` |
| 资金账单 | `y6Y7TW` | `m25xr0` | `/assets/ledger` / `WalletLedgerView.vue` |
| 提币记录 | `DxqMB` | `G3HecO` | `/assets/withdrawals` / `WithdrawalRecordsView.vue` |
| 快捷充值 | `CyRqi` | `cM0eg` | `/assets/quick-recharge` / `QuickRechargeView.vue` |

### 认证和账户

| 页面 | Light | Dark | 生产路由/文件 |
| --- | --- | --- | --- |
| 二次验证 | `qmNDA` | `kp9wV` | `/login/two-factor` / `LoginTwoFactorView.vue` |
| 找回密码 | `mgAF7` | `HrPy2` | `/forgot-password` / `ForgotPasswordView.vue` |
| 安全中心 | `WZ42z` | `sDl6T` | `/profile/security` / `SecurityView.vue` |
| KYC | `Raoes` | `wJT9Y` | `/profile/kyc` / `KycView.vue` |
| 账户绑定 | `x84Cbv` | `Z0ging` | `/profile/bindings` / `AccountBindingsView.vue` |
| 邀请访客 | `c80gd` | `Bmt4u` | `/profile/referrals` / `ReferralsView.vue` |
| 邀请会员 | `e4bPj` | `Qy31s` | 同上，真实登录态驱动 |
| 语言 | `kwFEy` | `yPf6O` | `/profile/language` / `LanguageView.vue` |

## 精确共享几何

- 所有补齐的普通二级页在 Web 内容坐标中从 `y=0` 开始使用 60px Header：左右 40px 控件、18px/750 居中标题、横向 20px、纵向 10px；应用 `PageHeader :pencil="true"`。
- 普通页面 Body 从 `y=60` 开始，横向 padding 20px，上 padding 6–8px；相关项 gap 10–14px。
- 资产/网络选择搜索框为 350×44、22px 胶囊；资产行 350×60；币种圆标 36×36。
- 钱包表单字段、账户表单和主操作均使用完整容器聚焦环；内部 input 不允许第二层蓝色/矩形边框。
- 浅色背景为 `#FFFFFF` 或画板指定的 `#F7F9F8`；深色背景为 `#000000`。正文、分隔线、薄荷/珊瑚语义色继续使用现有 Pencil token，不恢复旧网格舞台或大型 Editorial 卡片。
- 列表页不使用大 Hero；64px 行、44px 圆形图标板、18px chevron 与 11–13px 辅助文案按导出 HTML实现。
- 未登录、加载、错误、空态必须保持同一几何骨架，不复制画板演示余额、订单、邀请码、产品或认证结果。

## 生产基线确认的主要差异

- 钱包、KYC、绑定、邀请等页面仍在使用旧 `PageShell`、大眉题、大登录卡和淡网格背景，与选中画板结构不同。
- Product Hub 当前为五张大卡，选中稿为两条 64px 连续产品行和一条 48px说明入口。
- Message Center 当前为大摘要卡；选中稿为 60px Header、34px 分类栏、64px 连续消息行，并包含五入口 Dock。
- Contract、Seconds 当前沿用旧工作台层级与材质，未映射选中合约和秒合约默认/进行中状态。
- 认证二级页、Security 和 KYC 的字段高度、标题位置、状态行、主按钮和空白节奏均未逐坐标对齐。

## 跳转与返回合同

1. 根 Dock 固定为：首页、行情、现货交易、资产、我的；使用 `replace`，不堆积根页面历史。
2. Message Center 由首页 Header 进入，并按选中稿显示 Dock；它不是带 Root Header 的根页面。
3. 合约和秒合约不显示 Dock；返回到真实来源，深链兜底首页。现货继续显示 Dock。
4. Product Hub 返回首页；预测/理财/借贷/新币返回 Product Hub。Product Hub 的新闻入口进入 `/news`，产品说明进入 `/news?category=product`，不得跳到无关业务页。
5. 充币：资产 → 网络 → 地址；地址深链返回当前资产网络页。提币：资产 → 表单。账单、提币记录和快捷充值返回资产页。
6. KYC、安全、账户绑定、邀请和语言返回我的。受保护页面登录按钮携带当前完整 `route.fullPath`。
7. 登录 → 注册/找回密码/二次验证属于同一认证流程，使用 `replace` 并完整保留清洗后的 `redirect`；完成后只回到该目标，不把中间认证步骤留在历史栈。
8. Profile 访客的右上设置进入公开语言设置，不再跳到需要登录的安全中心；会员右上设置进入安全中心。Profile 的注册入口保留 `redirect=/profile`。
9. 所有 PageHeader 返回优先使用可用历史；直开深链时使用上表业务父级兜底，动态充币详情不得错误跳过网络选择页。

