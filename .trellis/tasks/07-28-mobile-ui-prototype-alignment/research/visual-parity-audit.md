# 真实手机端与 Sites 原型视觉一致性审计

## 结论

真实 Vue 客户端已经具备完整路由、真实接口、认证、主题、PWA 和 Tauri 能力，但视觉层仍是通用移动后台语法；Sites 原型则已经形成 HIPPO 的交易终端语法。重构必须复用原型的层级与交互形态，不能复制模拟数据，也不能只靠 `base.css` 覆盖全部页面。

## 同屏差异

### 真实订单页

- Header 使用居中单行标题，缺少原型中的场景标签、副标题和左对齐信息层级。
- 未登录状态是一个居中大卡片，下方形成大面积空白，页面不像交易工作台。
- 标签、数据区和动作区缺少硬边界分区，真实订单数据出现后仍偏通用列表。
- 页面宽度上限为 620px，桌面预览显得过宽；原型手机工作区约 430 至 448px。

### 原型订单页

- 二级 Header 左侧返回按钮，右侧为 `交易管理 / 订单中心 / 委托、持仓与历史` 三层信息。
- 三栏标签等宽、边界明确，活动态用底部信号线而不是大圆角胶囊。
- 订单条目是紧凑横向信息面，主信息、辅助信息和动作形成清晰列。
- 页面使用低圆角、硬边线、高密度、等宽数字和冷中性色。

## 共享视觉系统

### 应复用

- `mobile/sites-prototype/app/globals.css` 中 3000 行后的最终覆盖层是当前原型的主要视觉合同。
- 手机画布宽度控制在 448px 以内；真实应用在大屏仅作为居中手机工作区，手机宽度下占满。
- 浅色使用明亮冷灰绿背景、白/浅灰表面、近黑文字、绿色信号色和珊瑚色风险/强调。
- 深色使用近黑背景、分层黑灰表面、明亮绿色信号色，不能简单反转浅色。
- 页面以 1px 边界和 full-width band 分区，不使用卡片套卡片。
- 圆角 0 至 8px；关键圆形仅用于图标按钮、币种标识和秒合约主入口。
- 正文使用系统无衬线；金额、价格、场景编号可使用等宽字体栈。
- 动效使用短距离平移、淡入和边线变化；支持 reduced motion。

### 真实端需调整

- `mobile/src/styles/base.css`
  - `--app-max-width` 从 620px 收敛到 448px。
  - 将 `--radius: 10px` 收敛到不超过 8px。
  - 引入场景标签、数据字体、信号色、紧凑分区和共享表单容器。
  - `.page-content` 以 16px 为常规边距，320px 下 12px。
  - `.surface` 不默认附加强阴影；边界优先。
  - 组合输入使用容器 `:focus-within`，内部 input 不画独立边框。
- `mobile/src/App.vue`
  - 路由动画期间旧页面不可覆盖 sticky Header 与底部导航。
  - 保持 PWA 状态和认证过期逻辑。
- `mobile/src/components/PageHeader.vue`
  - 支持 `eyebrow`、`subtitle`、左对齐标题组和可选紧凑模式。
  - 返回按钮与操作按钮保持 44x44，Header 不透明且层级最高。
- `mobile/src/components/AppBottomNav.vue`
  - 七栏顺序保持：首页、行情、现货、秒合约、合约、资产、我的。
  - 中间秒合约抬升，导航外形与原型一致。
  - 320px 下标签不重叠，图标不偏心。
- `LoginRequiredState.vue`、`PwaStatus.vue`
  - 改成 full-width 状态区，不形成大圆角孤岛。
  - 不改变认证跳转、安装或离线逻辑。

## 页面文件分组

### Slice A：基础与一级页面

所有权：

- `mobile/src/styles/base.css`
- `mobile/src/App.vue`
- `mobile/src/components/AppBottomNav.vue`
- `mobile/src/components/PageHeader.vue`
- `mobile/src/components/LoginRequiredState.vue`
- `mobile/src/components/PwaStatus.vue`
- `mobile/src/components/AssetMark.vue`
- `mobile/src/views/HomeView.vue`
- `mobile/src/views/MarketsView.vue`
- `mobile/src/views/AssetsView.vue`
- `mobile/src/views/ProfileView.vue`
- `mobile/src/views/ProductHubView.vue`

要求：

- 首页对齐品牌 Header、搜索、深色资产信号区、买币/充币、圆形快捷入口、市场表格和资讯条。
- 行情页使用信号色 intro、搜索、筛选轨和高密度价格表。
- 资产页使用全宽资产 Hero、资金操作矩阵和账户/资产列表。
- 我的页使用身份 Hero、等级、三项统计和设置矩阵。
- 产品中心按主业务和扩展业务分层，不把现货、合约、秒合约合并。

### Slice B：交易域

所有权：

- `mobile/src/components/MobileMarketChart.vue`
- `mobile/src/components/OrderBookPanel.vue`
- `mobile/src/views/TradeView.vue`
- `mobile/src/views/SecondsView.vue`
- `mobile/src/views/MarketDetailView.vue`
- `mobile/src/views/OrdersView.vue`

要求：

- 现货与合约共用真实页面逻辑，但通过 Header 场景、模式标签、保证金信息和动作区明确区分。
- 行情头、实时价格、图表、盘口、订单类型、价格/数量、百分比和主操作形成纵向终端结构。
- 秒合约保留独立入口，周期、方向、金额、产品和历史信息必须在首屏内形成完整仪表盘。
- 订单页增加 `eyebrow + title + subtitle` Header，标签等宽，订单/持仓数据紧凑，批量操作不遮挡列表。
- 图表和盘口在 320px 下不横向溢出；画布必须非空。

### Slice C：二级页面

所有权：

- `mobile/src/views/AccountBindingsView.vue`
- `mobile/src/views/DepositAssetView.vue`
- `mobile/src/views/DepositDetailView.vue`
- `mobile/src/views/DepositNetworkView.vue`
- `mobile/src/views/EarnView.vue`
- `mobile/src/views/ForgotPasswordView.vue`
- `mobile/src/views/KycView.vue`
- `mobile/src/views/LanguageView.vue`
- `mobile/src/views/LoanView.vue`
- `mobile/src/views/LoginTwoFactorView.vue`
- `mobile/src/views/LoginView.vue`
- `mobile/src/views/MessageCenterView.vue`
- `mobile/src/views/NewCoinDetailView.vue`
- `mobile/src/views/NewCoinRecordsView.vue`
- `mobile/src/views/NewCoinsView.vue`
- `mobile/src/views/NewsDetailView.vue`
- `mobile/src/views/NewsView.vue`
- `mobile/src/views/PredictionView.vue`
- `mobile/src/views/QuickRechargeView.vue`
- `mobile/src/views/ReferralsView.vue`
- `mobile/src/views/RegisterView.vue`
- `mobile/src/views/SecurityView.vue`
- `mobile/src/views/SwapView.vue`
- `mobile/src/views/WalletLedgerView.vue`
- `mobile/src/views/WithdrawAssetView.vue`
- `mobile/src/views/WithdrawView.vue`
- `mobile/src/views/WithdrawalRecordsView.vue`

要求：

- 统一使用共享 Header、场景标签、分组标题、低圆角表面、容器聚焦输入和清晰主次按钮。
- 消息中心：等宽分类、未读信号、时间与摘要分层。
- 借贷/理财/新币/预测/闪兑：业务数据区与操作区分离，风险或收益信息不可只放在弱化文本中。
- 安全/KYC/绑定：状态、风险等级和操作入口必须一眼可扫描。
- 认证页：使用品牌身份区、步骤提示、完整字段状态，不使用通用居中卡片。
- 钱包二级页：资产、网络、地址、费用、余额和记录保持统一信息层级。

## 必须保留的真实业务合同

- API 调用、适配器、Pinia store、鉴权刷新、登录重定向、i18n 和路由名不变。
- 未登录、加载、错误、空、成功、禁用和提交中状态继续存在。
- Trade 保留现货/合约 `mode` 路由；Seconds 保留独立根路由。
- Orders 保留现货撤单、杠杆撤销/平仓、批量操作和历史查询。
- Loan 保留申请、撤销、还款和逾期还款。
- Security 保留密码、资金密码和 2FA API。
- MessageCenter 只显示真实公告并保存本地已读 ID。
- PWA 不缓存金融 API；Tauri 不回退设备 loopback。

## 320 / 390 / 448px 验收

### 320px

- 七栏底部导航所有图标居中，标签最多自然换行一次且不重叠。
- 表格价格列和涨跌列不被裁切；必要时缩减列间距，不缩放字体。
- 输入后缀、币种单位和最大按钮不覆盖输入值。
- 二列借贷/产品布局降为单列。

### 390px

- 作为主要设计基准，首页首屏可看到资产区、资金操作和快捷入口起始。
- 交易首屏应包含行情头、图表主体和订单区开头。
- Header、标签和 sticky 动作区不遮挡正文。

### 448px

- 内容不被无意义拉宽；数字列保持紧凑，边线从左到右完整。
- 桌面浏览器居中显示为手机工作区，外部背景不影响生产手机 UI。

## 验证建议

- `npm --prefix mobile run type-check`
- `npm --prefix mobile test`
- `npm --prefix mobile run build:pwa`
- `npm --prefix mobile run build:tauri`
- 浏览器访问首页、行情、现货、合约、秒合约、资产、我的、订单、消息、借贷、安全。
- 每个核心页面在 320/390/448px 截图，检查重叠、溢出、Header 层级、底部导航、输入焦点和对话框。
- 检查浏览器控制台没有 Vue、网络或画布异常。
