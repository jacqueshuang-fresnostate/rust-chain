# Research: Pencil 交易记录模块 14 画板与前后端接口审计

- Query: 将此前“独立卡片”合同明确替换为 `/private/tmp/pencil-orders-module.html` 中当前选中的 14 个浅深主题画板；审计 mobile 的 OrdersView、WalletLedgerView、API 适配器、路由，以及 Rust spot/margin/wallet DTO 与查询，给出 1:1 实现所需的字段映射、缺口、最小向后兼容接口方案、修改/验证范围，并明确禁止伪造演示数据。
- Scope: internal
- Date: 2026-09-02

## Findings

### 1. 结论与合同优先级

1. `/private/tmp/pencil-orders-module.html` 是本次审计唯一的当前视觉事实源；文件为正式导出的可读 HTML，大小 413,288 bytes，SHA-256 为 `0036db29b2b0e4ab940de6e34ccb6e8d8bc2396869299753de3ec18e45b37b13`。本次未读取加密 `.pen` 文件。
2. 最新要求覆盖此前“独立卡片”合同：交易记录行必须按导出画板的通栏、分隔线式结构落地，不能继续使用浅灰页面上的圆角白卡、卡间距或 64px 紧凑订单行。旧合同仍残留在 `.trellis/spec/mobile/navigation-and-localization.md:253-269`、`mobile/tests/wallet-ledger-classification.test.ts:746-794` 与 `mobile/tests/pencil-selected-unmapped-pages.test.ts:91-102`，后续应由 `trellis-update-spec`/实现代理同步，不应继续作为验收依据。
3. 范围是完整交易记录模块，而非只改账单页：当前委托、历史委托、历史仓位、关联订单、当前仓位和资产、交易账单，以及画板中出现但后端能力声明不支持的当前策略/历史策略入口。
4. 14 张设计为 7 个状态的浅色/深色对照；画板尾部分类说明也明确写有 “14 designs” 及完整字段范围（`/private/tmp/pencil-orders-module.html:6827-6864`）。
5. 当前任务 PRD 和旧研究虽已记录“14 画板覆盖旧合同”，但部分细节已落后于正式导出：PRD 把导航描述成单一连续 7-tab 条，并把空状态写成 ClipboardList；正式 HTML 实际按页面展示不同的 4-tab 窗口，空状态为 `receipt-text`。另外，当前 Rust/移动端已经具备 margin 时间字段和 executions API，不能再列为缺失。

### 2. 正式导出画板清单与画布坐标

HTML 总画布为 `6500 × 1032`（`/private/tmp/pencil-orders-module.html:27`），每个设备画板均为 `390 × 920`、`top:112px`、裁剪溢出；浅色背景为 `#FFFFFF`，深色背景为 `#000000`。

| # | Pencil 页面 | 主题 | 画布 x | HTML 行 | 页面含义 |
|---|---|---:|---:|---:|---|
| 1 | 08 | 浅色 | 0 | 29-735 | 交易账单，有记录 |
| 2 | 08 | 深色 | 470 | 736-1442 | 交易账单，有记录 |
| 3 | 08b | 浅色 | 940 | 1443-1863 | 历史仓位详情行 |
| 4 | 08b | 深色 | 1410 | 1864-2284 | 历史仓位详情行 |
| 5 | 08c | 浅色 | 1880 | 2285-2471 | 交易账单空状态 |
| 6 | 08c | 深色 | 2350 | 2472-2658 | 交易账单空状态 |
| 7 | 08d | 浅色 | 2820 | 2659-3129 | 关联订单 |
| 8 | 08d | 深色 | 3290 | 3130-3600 | 关联订单 |
| 9 | 08e | 浅色 | 3760 | 3601-4354 | 历史委托 |
| 10 | 08e | 深色 | 4230 | 4355-5108 | 历史委托 |
| 11 | 08f | 浅色 | 4700 | 5109-5614 | 当前委托 |
| 12 | 08f | 深色 | 5170 | 5615-6120 | 当前委托 |
| 13 | 08g | 浅色 | 5640 | 6121-6825 | 当前仓位和资产 |
| 14 | 08g | 深色 | 6110 | 6866-7573 | 当前仓位和资产 |

#### 2.1 公共几何与色彩令牌

- 导出状态栏高 28px，左右 padding 16px；它仅用于 Pencil 设备预览，生产页面必须由原生 safe area/系统状态栏承担，不能再绘制一份假的 9:41、信号和电池。
- 普通页面标题栏高 58px、左右 padding 16px；返回图标 26px，标题“交易记录”22px/粗体，右侧保留 26px 对称占位。关联订单页标题栏为 62px。
- 主标签区域为 52px（51px 内容加 1px 底线），激活指示条高 3px且占满当前四等分标签项宽度；账单/空态横向 padding 10px，委托/仓位页横向 padding 8px。正式 HTML 的浅深画板均使用 `w-full h-[3px]`，不得误实现成 9px 短线。
- 大部分记录行是 390px 视口内的通栏内容，仅以 `#EDF1EF`/`#17221C` 底部分隔；没有卡片背景、圆角和卡间距。
- 浅色：背景/栏 `#FFFFFF`，主字 `#111714`，标签次字 `#7B8680`，行次字 `#8A948F`，标签线 `#EEF1EF`，行线 `#EDF1EF`，激活绿 `#18D38D`，正值 `#0DBE7B`，负值 `#FF5878`。
- 深色：背景 `#000000`，主字 `#F3F7F5`，次字 `#8F9B94`，标签线 `#18231D`，行线 `#17221C`，激活绿 `#18D38D`，正值 `#45EFAE`，负值 `#FF5878`。
- 浅色按钮/芯片背景包括 `#F3F6F4`、`#F1F3F2`、正值 `#DDF8EB`、负值 `#FFE8ED`；深色对应 `#111A15`、`#151E19`、`#103326`、`#32161F`。关联订单的段落分隔带浅色 `#F6F8F7`、深色 `#0D1511`。
- 画板里的当前委托操作按钮视觉高度为 42px、圆角 12px、间距 10px（`/private/tmp/pencil-orders-module.html:5405-5431`）；当前仓位操作按钮为 44px（`/private/tmp/pencil-orders-module.html:6480-6517`）。42px 视觉按钮应通过透明 hit wrapper/伪元素满足规范要求的至少 44px 触控范围，而不改变画板可见几何。
- 导出节点部分使用 `box-sizing:content-box` 与固定 `w-[390px]` 加横向 padding。其 `h-[…]` 同样只表示内容高度：记录的屏幕可见 advance 必须按 `content height + vertical padding + 1px border - 0.5px overlap` 计算。生产实现应保留安全的 `border-box`/自适应横向宽度，只把换算结果用于垂直外框；不能机械复制成可水平滚动的 390px 内容宽加 padding，320/390/448 宽均须保持横向溢出为零。

#### 2.2 各页面纵向几何

| 页面 | 结构与导出高度 |
|---|---|
| 08 交易账单 | 状态栏 28；标题栏 58；主标签 52；筛选栏 58（左右 16，筛选间距 24）；记录导出 content 165.5、生产可见 advance 190，padding `12px 18px`、内容 gap 9 |
| 08b 历史仓位 | 状态栏 28；标题栏 58；主标签 52；筛选栏 58；仓位导出 content 363.5、生产可见 advance 398，padding `10px 18px 24px`、gap 16；底部范围提示左右 18 |
| 08c 空状态 | 状态栏 28；标题栏 58；主标签 52；筛选栏 58；剩余空间居中，元素 gap 12，底部补偿约 150 |
| 08d 关联订单 | 标题栏 62；汇总区 266.5，padding `18px 18px 20px`、gap 14；分隔带 8；段标题 59.5；订单导出 content 189.5、生产可见 advance 218，padding `14px 18px`、gap 10 |
| 08e 历史委托 | 标题栏 58；主标签 52；订单类型条 44（左右 16、gap 18）；筛选栏 52；现货导出 content 149.5、生产可见 advance 174（`12px 18px`、gap 12）；保证金导出 content 189.5、生产可见 advance 214 |
| 08f 当前委托 | 标题栏 58；主标签 52；订单类型条 44；筛选栏 52；记录导出 content 209.5、生产可见 advance 238（`14px 18px`、gap 12）；尾部提示左右 18、上方 20 |
| 08g 当前仓位和资产 | 标题栏 58；主标签 52；筛选栏 52；仓位导出 content 309.5、生产可见 advance 334（`12px 18px`、gap 12）；资产导出 content 199.5、生产可见 advance 228（`14px 18px`、gap 14） |

#### 2.3 标签窗口不是一条固定可见的 7-tab 导航

正式画板展示的是状态相关的 4-tab 窗口：

- 08/08c：`历史仓位 / 交易账单（激活） / 当前策略 / 历史策略`。
- 08b：`当前委托 / 当前仓位和资产 / 历史仓位（激活） / 交易账单`。
- 08e/08f/08g：`当前委托 / 历史委托 / 当前仓位和资产 / 历史仓位`，分别激活对应项。
- 08d 关联订单没有主标签栏。

内部可以继续维护 7 个稳定 route key，但可见顺序和窗口必须逐状态匹配导出，不能把 7 个标签全部塞进一个可横滑条后声称 1:1。当前策略/历史策略的后端 capability 明确为 false（`src/modules/margin/presentation.rs:205-217`），因此这两个入口不得映射到普通 margin 列表、不得用假记录填充；在产品未补能力前，只能显示明确不可用/空态或按产品确认的禁用交互。

### 3. 页面字段清单

以下只描述字段语义；HTML 中 BTC/USDT、数值、时间、订单号均为视觉样本，不能成为运行时数据。

#### 3.1 交易账单（08）

- 筛选：币种、交易类型、更多。
- 行头：30px 资产图标、资产符号、带正负号的“总变动”。
- 交易上下文：交易对；数量标签/值；账户（如“现货 · 买入/卖出”）；成交数量及币种；手续费；本地时间；账户余额。
- 样本中 BTC 正向行的“总变动”与毛成交数量不同，显示净额已受手续费影响。因此现有账单 `amount` 只能代表净账户变动，不能同时冒充毛成交数量；交易对、买卖方向也不能从资产或金额正负可靠推断。

#### 3.2 历史仓位（08b）

- 筛选：全部交易类型。
- 行头：合约/交易对、进入关联页的 chevron、终态状态、分享按钮。
- 芯片：多/空、逐仓/全仓、杠杆倍数。
- 指标：开仓均价；已实现收益（USDT）；最大持仓数量（BTC）；平仓均价；已实现收益率；已平仓数量（BTC）。
- 时间：开仓时间、平仓时间。
- 操作：关联订单。
- 尾部：历史范围提示。

#### 3.3 交易账单空状态（08c）

- 64×64 圆形底板，浅色 `#F3F6F4`，深色对应深色令牌。
- Lucide `receipt-text` 图标 30px（`/private/tmp/pencil-orders-module.html:2434-2468`），不是当前实现的 FileSearch/ClipboardList。
- 标题：`暂无交易账单`，18px、正常字重。
- 描述：`完成交易后，收益与费用记录会显示在这里`，13px、正常字重。
- 只允许在一次真实请求成功且结果确实为空后显示；加载、失败、未认证必须分别处理。

#### 3.4 关联订单（08d）

- 标题栏：合约、方向芯片、分享。
- 汇总：已实现净收益、已平仓数量、平仓收益、交易手续费、资金费用、开仓时间、平仓时间。
- 订单行：开仓/平仓方向与时间、委托金额、成交数量、成交均价、手续费、可复制订单号。
- 画板某平仓样本中名为“委托金额”的值更像毛收益而非价格×数量，不能据样本反推公式；API/产品必须明确它究竟是成交名义金额、开仓名义切片还是其他业务金额。

#### 3.5 历史委托（08e）

- 二级订单类型：全部、限价/市价、高级限价、止盈止损。
- 筛选：全部交易类型、近 1 年、列表筛选按钮。
- 现货行：交易对、状态、订单类型、买/卖方向、时间、委托数量及动态单位、已成交数量及基础币单位、成交均价。
- 市价买入样本把委托数量显示成 USDT，限价单显示 BTC；这要求接口提供委托资产/金额语义，不能一律把后端 base `quantity` 改标签后展示。
- 保证金行：合约、状态、分享、订单类型、开仓/平仓方向、逐仓/全仓、杠杆、时间、委托数量、成交数量、成交均价；平仓行还显示平仓收益及收益率。

#### 3.6 当前委托（08f）

- 与历史委托相同的主标签、订单类型条和交易类型筛选。
- 行字段：交易对/合约、待处理状态、订单类型、买卖方向，或开仓方向/保证金模式/杠杆、时间、委托价格、委托数量（基础币）、已成交数量。
- 操作：修改、撤单；尾部有说明提示。
- 当前后端有撤单/取消能力，但没有保证原子性和语义完整的 amend endpoint。修改按钮在真实接口完成前应保持 capability 驱动的禁用态，不能只改本地展示值。

#### 3.7 当前仓位和资产（08g）

- 筛选：全部交易类型、资产显示/计价控制、列表筛选。
- 仓位：合约；未实现盈亏金额及收益率；方向、保证金模式、杠杆；持仓数量、保证金、“维持保证金率”；开仓均价、标记价格、预估强平价；止盈止损、平仓、市价平仓。
- Pencil 样本中“维持保证金率”数值更接近当前后端 `margin_ratio`，而不是风险模型参数 `maintenance_margin_rate`。二者不是同一语义，必须经产品/API 命名确认，不能直接换标签。
- BTC 类资产行：图标/符号/chevron；权益、成本价、最新价；余额、未实现盈亏、可用。
- USDT 类资产行：权益、占用、可用；未实现盈亏、余额、冻结。

### 4. 现有移动端实现审计

#### 4.1 OrdersView

- `mobile/src/views/OrdersView.vue:36-60` 只有 `spot|margin` 市场切换和 `current|history|positions` 状态切换，不具备 7 个规范 route key，也无法表达独立的历史仓位、关联订单与交易账单。
- `mobile/src/views/OrdersView.vue:239-300` 当前现货/历史现货各拉 30 条；历史 margin 并行拉 closed/liquidated/canceled 三次后在客户端拼接。它没有全局分页、统一排序、订单类型/方向/时间筛选；每个子集 limit 30 也不能保证得到真正的“最近 30 条历史委托”。
- `mobile/src/views/OrdersView.vue:101-105` 只对 spot 以 `createdAt` 排序；组合 margin 历史缺少服务端全局稳定排序。
- `mobile/src/views/OrdersView.vue:192-230` 把精确字符串转成 `Number` 格式化，并用 `notionalAmount / entryPrice` 客户端反推持仓数量。金融字段应由服务端以 DECIMAL string 给出，UI 只做展示格式化。
- `mobile/src/views/OrdersView.vue:504-513` 只在 mounted 时读取一次旧 query，没有监听同组件内 query 切换。
- `mobile/src/views/OrdersView.vue:527-564` 使用通用 PageHeader、无返回按钮和两层 segmented control；`mobile/src/views/OrdersView.vue:795-867` 仍是旧的 45/34/64px 紧凑结构，与正式画板不一致。
- 撤单与平仓行为已接真实 API，应在重构视觉结构时保留其鉴权、确认、错误与刷新语义。

#### 4.2 WalletLedgerView

- `mobile/src/views/WalletLedgerView.vue:47-124` 已有会话隔离、分页游标、stale response 丢弃、加载更多等可靠行为，重构不能丢失。
- `mobile/src/views/WalletLedgerView.vue:75-100` 把当前策略/历史策略跳到普通 Orders 状态，是 capability 不支持时的错误伪映射，应移除。
- `mobile/src/views/WalletLedgerView.vue:201-207`、`:245-251`、`:405-447` 第二筛选器是收入/支出方向；正式画板要求交易类型。后端已支持 `category`、`change_type`，收入/支出可保留在“更多”中。
- `mobile/src/views/WalletLedgerView.vue:267-347` 用 change type 文案充当交易对、以 amount 正负推断收入/支出，并把 `abs(amount)` 重复为“数量”；它无法提供正式画板的交易对、买卖方向、毛成交量、成交币种等语义。
- `mobile/src/views/WalletLedgerView.vue:467-515` 记录模板仍是旧账单行；`:516-520` 空态使用 FileSearch 24px，文案和几何均不符。
- `mobile/src/views/WalletLedgerView.vue:846-873` 明确采用有 gap、圆角 16、浅色卡片的旧“独立卡片”合同，必须被通栏分隔线布局替代。
- 现有 focus trap、滚动锁、首次/追加请求错误区分、精确 DecimalText 展示和分页行为是应保留的非视觉契约。

#### 4.3 可复用但尚未接入的交易记录组件

- `mobile/src/components/TransactionRecordsLayout.vue:1-242` 已包含接近正式画板的标题和 token，但 `:66-78` 始终循环全部 7 个标签，并以 25%/最小 94px 横滑，不能直接复现每页不同的 4-tab 窗口；`:34-37` 的路由目标可作为兼容路由改造起点。
- `mobile/src/core/transactionRecords.ts:16-24` 已定义 7 个 canonical tab；`:80-155` 也有 exact-decimal 历史聚合尝试。canonical key 可复用，但持久金融指标和 legacy 部分平仓重建不宜由客户端作为权威计算。
- `mobile/src/components/TransactionOrderRecord.vue` 不得把导出的 209.5/149.5/189.5 content height 直接当作 border-box 最小高度；对应生产可见外框必须为 238/174/214px。字段行仍需按现货、保证金、当前/历史四种语义拆分；修改按钮目前为禁用，符合“接口未完成前不伪造修改”的方向。
- `mobile/src/components/TransactionRecordEmptyState.vue` 未被引用，使用 ClipboardList 且文案/垂直布局不符；可改为正式 `ReceiptText` 空态后复用。
- `mobile/src/components/AssetMark.vue:17-25`、`:43-51`、`:78-117` 支持后端 logo 和确定性字母 fallback。字母图标 fallback 不是金融演示数据，可以保留。

### 5. 前端 API 适配器与现有字段映射

#### 5.1 现货订单

| 画板字段 | 当前移动端/后端来源 | 状态 |
|---|---|---|
| 交易对 | `SpotOrder.symbol`；后端 `pair_symbol` | 已有 |
| 买/卖 | `side` | 已有 |
| 限价/市价 | `orderType` | 已有 |
| 状态 | `status` | 已有 |
| 委托价 | `priceText` | 部分：`mobile/src/api/trading.ts:334` 用 `price ?? average_price`，会混淆委托价与成交均价 |
| 成交均价 | `averagePriceText` | 已有 |
| base 委托量 | `quantityText` | 已有，后端 domain 的 market quantity 仍为基础币数量（`src/modules/spot/domain.rs:217-243`） |
| 已成交量 | `filledQuantityText` | 已有 |
| 时间 | `createdAt` | 已有 |
| 可见订单号 | raw `id` | 不合规；规范要求 `order_no` 或稳定非裸主键 token |
| 市价买入 quote 委托金额/资产 | 无 | 缺失；不能把 base quantity 贴成 USDT |
| 手续费、单笔成交上下文 | 列表无；spot trades/ledger 中有部分事实 | 缺失于列表 DTO |
| 分页总数/offset | 响应只有 `orders` | 缺失 |

`mobile/src/api/trading.ts:315-360` 当前映射保留了 exact string；`:367-378` 为多个状态分别请求后在 `:865-868` 去重排序。服务端查询仍应提供真正的多状态全局分页，避免客户端 fan-out。

#### 5.2 保证金仓位/委托

| 画板字段 | 当前移动端/后端来源 | 状态 |
|---|---|---|
| 仓位 ID、pair/product ID | `MarginPosition` | 已有 |
| 方向、逐仓/全仓、杠杆 | `direction/marginMode/leverage` | 已有 |
| entry/exit price | `entryPriceText/exitPriceText` | 已有 |
| notional、margin、interest | exact strings | 已有基础字段 |
| realized PnL | `realizedPnlText` | 已有，但语义是毛价格盈亏，不是画板净收益 |
| 开仓/创建/平仓时间 | `openedAt/createdAt/closedAt` | 已有；见 `mobile/src/api/trading.ts:116-146`、`:762-809` 及 `src/modules/margin/presentation.rs:370-410` |
| close executions | `GET /margin/positions/:id/executions` | 已有且 owner-scoped；适配器在 `mobile/src/api/trading.ts:507-535`、`:812-843` |
| pair symbol、base/quote 单位、precision | 需另拉 products/pairs | 列表未自包含 |
| 持仓数量、最大/已平仓数量 | 客户端以 notional/price 猜 | 缺少服务端权威 read projection |
| 平仓均价、历史收益率 | 可由 executions/legacy row 重建 | 列表未提供，客户端不应成为权威 |
| 交易手续费 | 没有用户侧权威来源 | 缺失；agent commission 不是用户手续费 |
| 资金费用 | `interest_amount`/execution settlement 中有部分事实 | 可构建，但需明确命名和历史规则 |
| 净收益 | 当前无权威完整组成 | 缺失；不得把缺失手续费当 0 |
| 订单号 | raw id | 缺失稳定 display token |
| TP/SL、策略 | capabilities 为 false | 不支持，必须禁用/明确不可用 |

#### 5.3 账单

| 画板字段 | Rust wallet ledger | 当前移动端适配 | 状态 |
|---|---|---|---|
| 资产/precision | `asset_id/symbol/precision` | 仅 symbol/precision | 后端已有，前端 model 丢 asset_id |
| 净总变动 | `amount` | exact `amountText` | 已有 |
| 账户类型 | `account_type` | 已映射 | 已有 |
| change/category | `change_type/category` | 已映射 | 已有 |
| 各 bucket 前后余额 | `balance_type` 与 available/frozen/locked snapshots | 前端 model 丢弃 | 后端已有、前端缺失 |
| 账户余额 | `balance_after` | 已映射 | 已有 |
| fee | legacy 数值 | 已映射 | 现货部分可知；margin 等路径的 0 可能只是 fallback，不等于已知零 |
| ref_type/ref_id | 已有 | 前端 model 丢弃 | 后端已有、前端缺失 |
| 时间 | `created_at` | 已映射 | 已有 |
| 交易对、订单类型、买卖/资产流向 | 无嵌套上下文 | 无 | 缺失 |
| 毛成交数量/币种、成交价 | 无嵌套上下文 | 无 | 缺失 |

`src/modules/wallet/presentation.rs:363-414` 的 DTO 已公开丰富账本事实；`mobile/src/core/walletLedger.ts:48-84` 只接收其中一部分。`src/modules/wallet/infrastructure/accounts_ledger.rs:981-1036` 对若干 spot/convert/withdraw 引用补 fee，但未匹配路径会 `COALESCE` 为 0，因此新 UI 必须同时获得 fee 的可用性状态。

#### 5.4 当前仓位风险与资产

| 画板字段 | 当前来源 | 状态 |
|---|---|---|
| 当前仓位数量、未实现盈亏/率、margin ratio、mark、liquidation | `GET /margin/positions/:id/risk` | 已有 exact 字符串；适配器 `mobile/src/api/trading.ts:562-630`，OrdersView 尚未使用 |
| 当前 margin wallets available/frozen/locked/cross | `GET /margin/wallets` | 已有基础 bucket |
| 资产 precision | mobile 类型支持 `precisionScale`，wallet account DTO/mapper 未提供 | 缺失于该接口；`mobile/src/core/types.ts:83-95`、`mobile/src/api/wallet.ts:287-294,690-707` |
| 资产权益、占用、成本价、最新价、未实现盈亏、观测时间 | `/margin/wallets` 无 | 缺失 |
| 多资产 base exposure | 当前响应主要按 margin asset | 语义/范围待确认 |

### 6. Rust spot DTO 与查询审计

- `src/modules/spot/presentation.rs:23-28` 用户列表只接受 pair/status/limit；`:80-101` 响应有 id、pair symbol、side/type、价格/数量/成交、状态/时间，但没有分页信息、precision、基础/计价币、委托资产与保留金额。
- `src/modules/spot/routes.rs:97-108` 从 JWT 强制注入 owner，安全边界正确，应保留。
- `src/modules/spot/application/queries.rs:28-50` clamp limit 且把 offset 固定为 0。
- `src/modules/spot/infrastructure/read_models.rs:480-493`、`:546-571` 仅实现单 pair/单 status/limit；稳定排序为 created_at desc、id desc（`:21-23`），但没有 count/page envelope。
- `src/modules/spot/domain.rs:217-243` 表明市价买单的 `quantity` 仍是基础资产量；`:277-297` 的买单 reservation 才是 quote 侧金额事实。因此画板的市价买入 USDT 委托金额应来自明确的 reservation/read projection，而不是客户端改单位。
- `src/modules/spot/infrastructure/read_models.rs:161-225` 已持久化 reserved asset/amount、reference price 等订单事实，但用户列表 DTO 未暴露。
- `src/modules/spot/application/trade_settlement.rs:126-153` 的账单引用包含买卖订单 ID，可用于服务端建立账单 `trade_context`，但 self-trade/同用户双侧时必须明确对应哪一资产腿，歧义时返回 unavailable，不能猜。

### 7. Rust margin DTO、查询与历史重建审计

- `src/modules/margin/presentation.rs:27-32` 列表只接受单 status/limit；`:370-410` 当前响应已经包含 exit price、realized PnL、opened/created/closed 时间。
- `src/modules/margin/routes.rs:69-94` 已有 list/detail/executions/risk/close/cancel；没有关联订单聚合、amend、TP/SL 或策略接口。
- `src/modules/margin/application/queries.rs:42-56` 与 `src/modules/margin/infrastructure/position_queries.rs:128-160` 只支持单状态，无 offset、count、pair/direction/type/time 组合筛选。
- `src/modules/margin/application/queries.rs:110-124` 和 `src/modules/margin/infrastructure/close_executions.rs:89-110` 已提供 owner-scoped close executions，按时间稳定升序并保留 exact Decimal 字符串。
- `migrations/0117_margin_close_executions.sql:1-30` 是部分/完全平仓切片事实源。
- `src/modules/margin/application/lifecycle.rs:56-279` 计算部分和完全平仓；部分平仓会把 position row 更新成剩余切片（`:246-259`）。完全平仓的历史 row 保留最终切片，而不是清零；相关 settlement 逻辑见 `src/modules/margin/infrastructure/settlement.rs:347-380,397-430`。
- 因此服务端统一重建规则应为：
  - 若有显式 `fully_closed` execution，原始 notional/margin/interest 是全部 execution slice 之和，不再额外加最终 position row，避免重复计算。
  - 若没有显式 fully_closed execution，原始值是当前 position row 加所有显式 execution slice，兼容仍在部分平仓的活动仓位和旧版无 body 的最终平仓。
  - 加权平仓均价优先由 execution slices 计算；若旧记录存在未落 execution 的最终残余切片，则用 position `exit_price` 与残余 notional 一并加权，并返回 reconstruction status。
  - 数量除法必须在 Rust/SQL Decimal 域完成，响应为十进制字符串，不能让移动端转 JS Number 后计算。
- 当前 `realized_pnl` 是毛价格盈亏；lifecycle settlement 还会扣 interest（`src/modules/margin/application/lifecycle.rs:146-167`）。画板“已实现净收益 = 平仓收益 - 交易手续费 - 资金费用”需要完整组成。现有 agent commission 是代理分佣，不是用户交易手续费（`src/modules/margin/application/open_position.rs:249-256`），不能冒用。手续费未知时必须为 null/unavailable，净收益也应标记不可用或只显示已知组成。
- `src/modules/margin/application/queries.rs:186-236` 与 `:386-423` 已能构建当前风险字段，可复用成批量 read model，避免前端对每个仓位逐个请求并混用不同观测时刻。
- `src/modules/margin/application/queries.rs:59-93` 的 wallet/positions/cross 是三次独立读取，并非同一数据库快照；若在同一资产页混排，应暴露 `observed_at`/一致性状态，不能暗示完全同时点。

### 8. Rust wallet DTO 与查询审计

- `src/modules/wallet/presentation.rs:208-221` 路由已经支持 owner-scoped ledger。
- `src/modules/wallet/presentation.rs:363-377` 查询参数已经覆盖 asset/change/category/account/direction/ref/time/limit/offset；不需要为画板重新发明基础筛选协议。
- `src/modules/wallet/application.rs:635-718` 构建和校验筛选；`src/modules/wallet/infrastructure/accounts_ledger.rs:771-813` 对 union 后的全局集合执行稳定分页和 count，优于 OrdersView 当前的客户端 fan-out，应保留这种服务端分页模式。
- `src/modules/wallet/infrastructure/accounts_ledger.rs:175-209` 定义筛选，`:835-888` 保证 list/count 条件一致。
- `src/modules/wallet/presentation.rs:393-414` ledger entry 已有 precision、bucket snapshots、fee、refs；移动端应先补齐这些已有字段，再消费新增上下文。
- `src/modules/wallet/presentation.rs:352-361` 的 wallet account 未带 precision；`src/modules/wallet/infrastructure/accounts_ledger.rs:750-768` 查询也未选择 precision。该字段可以无破坏地追加。
- `src/modules/wallet/infrastructure/accounts_ledger.rs:270-320` 的 category 已含 funding、spot、margin、seconds、convert、earn、new_coin、loan、prediction、other，适合正式“交易类型”筛选；change_type 可用于更细粒度选择。

### 9. 最小向后兼容接口方案

所有新增金融数值继续使用十进制字符串；所有读取必须保持 JWT owner 约束；缺失值使用 nullable 字段加明确 `availability/status`，不能默认为 0。现有 route、字段和旧 query 语义均保留。

#### 9.1 Spot 列表：扩展现有 `GET /spot/orders`

保持 `pair`、单值 `status`、`limit` 和现有 `orders` 字段；无破坏地增加：

- 可选 `statuses`（白名单逗号列表，未传时仍用旧 `status`）、`side`、`order_type`、`start_time`、`end_time`、`offset`。
- 响应追加 `total/limit/offset/has_more`，但继续保留顶层 `orders`。
- 每条追加 `pair_id`、`base_symbol`、`quote_symbol`、各自 precision、独立 `order_price` 与 `average_fill_price`、`order_quantity_amount`、`order_quantity_asset_symbol`、`filled_quantity_amount`、`filled_quantity_asset_symbol`、稳定 `order_no`。
- 市价买入的 quote 委托金额由已持久 reservation/read fact 得出；无法权威还原的旧单返回 null + unavailable，不做 base×价格的客户端猜测。
- 当前委托“修改”不属于这次最小只读方案；在原子 amend API、reservation 调整和幂等语义完成前保持禁用。撤单继续使用现有真实 endpoint。

#### 9.2 Margin 列表：扩展现有 `GET /margin/positions`

- 保留单值 `status/limit`；增加可选 `statuses`、`pair_id/product_id`、`direction`、`order_type`、时间范围、`offset` 和 page metadata。
- 追加自包含 display metadata：pair/base/quote symbol、logo、precision、margin mode/asset、稳定 `order_no`。
- 追加服务端 Decimal read projection：当前/原始/已平仓数量，开/平仓加权均价，毛平仓收益、资金费用、交易手续费及 availability、净收益及 availability、收益率、reconstruction status。
- mutation DTO 不改；历史聚合 helper 同时供列表和关联页复用，避免两套公式。

#### 9.3 关联订单：新增只读聚合 endpoint

严格最少请求可以组合现有 position detail + executions + product catalog，但无法权威补齐手续费/净收益，且列表进入多项详情时会产生 N+1。生产可验收的最小方案建议新增：

`GET /margin/positions/:id/associated-orders`

- 保持 owner-scoped；不存在和他人资源均按项目现有 not-found 语义处理。
- 响应：`position_summary`、`open_order`、`close_orders[]`、`reconstruction_status`。
- 每个 order slice 提供 operation/direction、时间、明确命名的 notional amount、filled quantity/unit、average fill price、fee amount/asset/availability、稳定 order_no。
- 汇总提供画板字段，但 trading fee 或 legacy slice 不可知时返回 null/unavailable；不得输出假 0。

#### 9.4 Ledger：扩展现有 `GET /wallet/ledger`

- 完全保留当前 filters、全局 count/page envelope 和 entry 字段。
- 每条追加可选 `trade_context`，只在引用链能权威关联时填充：market、pair id/symbol、base/quote、order type、order id/order_no、operation/side 或 asset_flow、gross executed quantity amount/asset、execution price/trade id、fee amount/asset/status。
- spot `ref_type=spot_trade` 可解析 ref 并联查 trade/orders/pair；base 资产腿显示 base qty，quote 资产腿显示 price×qty 的 quote 毛额。self-trade 或引用不完整时返回 unavailable，不从 amount 正负猜 side。
- margin `ref_type=margin_position` 只关联 owner 的 position/execution；operation 由真实 change_type/执行切片决定。
- 保留 legacy `fee` 以兼容旧客户端，同时新增 `fee_status=known|unavailable`；新 UI 以 status 决定显示金额还是 `--`。

#### 9.5 当前仓位和资产：扩展现有 `GET /margin/wallets`

- 保留 `wallets/positions/cross`，追加可选 `portfolio_assets[]`，并建议追加同一观测批次的 `position_risks[]`。
- 资产字段：asset id/symbol/logo/precision、equity、balance、available、occupied、frozen、locked、cost_price、last_price、unrealized_pnl、price_observed_at、data_status/missing_fields。
- 当前没有持久、可验证的成本价事实时，`cost_price=null` 并显示 `--`；不能从历史账单或首笔价格随意拼一个值。
- `occupied` 的产品定义、是否纳入 base exposure 资产、以及“权益”的计价币口径需要产品确认。确认前接口可返回明确 unavailable，不应让前端自行求和。

#### 9.6 路由兼容

- 保留 `/orders` 与 `/assets/ledger` 路径及 route name，避免破坏 `TradeView.vue:876-879`、`MarketDetailView.vue:274-278` 和资产页调用方。
- `/orders` canonical query 建议为：`current`、`history`、`positions`、`position-history`、`ledger`、`current-strategy`、`strategy-history`。
- 继续接受旧值：`spot` → 当前委托并保留 spot filter；`margin` → 当前委托并保留 margin filter；旧 `positions` → 当前仓位和资产；旧 `history` → 历史委托。保留 `symbol` query。
- 组件必须 watch route query，而非只在 mounted 时读取。
- `mobile/src/router/index.ts:62` 的 `/orders` 应追加 `showBottomNav:false`；`/assets/ledger` 已是隐藏底栏并有 `/assets` back fallback（`:77`）。
- 新增移动端详情路由 `/orders/positions/:id/associated`，隐藏底栏，返回历史仓位时保留其 query/filter/scroll 上下文。
- 不建议把 `/assets/ledger` 粗暴 redirect 到 `/orders?tab=ledger`，因为这会损失资产入口的返回语义；应共享 layout/content 或通过 route props 保留来源。

### 10. 明确禁止伪造演示数据

- HTML 中 BTC/USDT、价格、数量、盈亏、费率、订单号、日期、9:41、电池和信号均是设计 fixture，只能用于字段和视觉对齐。
- 生产记录只来自真实 API；不得在 Vue template/store/adapter 中写死任何财务记录，不得用随机数、mock 数组或静态 fallback 模拟“看起来完整”。
- 请求成功且结果为空才显示正式空态；loading、unauthenticated、first-load error、append error 各自保留真实状态。
- 后端缺字段时显示 `--`/明确“不可用”，或 capability 驱动禁用；不得用 0、当前时间、默认 BTC/USDT、raw amount 符号、资产名或客户端推导值填空。
- 不把 ledger 净 `amount` 当毛成交量，不从正负号推断非交易流水的买卖方向，不把 agent commission 当用户手续费，不把 `margin_ratio` 未经确认改名为 maintenance margin rate。
- 继续使用后端 logo；没有 logo 时可使用 `AssetMark` 的确定性字母 fallback，但不得伪造金融值。
- 所有金额、价格、数量、费率在 API/adapter 层保持 DECIMAL string；不得经 JS `Number` 执行业务计算。

### 11. 需要修改的文件（交给 implement 代理）

#### Mobile 视图、组件与路由

- `mobile/src/views/OrdersView.vue`：改为完整交易记录状态模型和正式画板结构，保留真实撤单/平仓行为。
- `mobile/src/views/WalletLedgerView.vue`：移除卡片合同，接入 trade context、正式筛选和空态，同时保留分页/并发安全。
- `mobile/src/views/AssociatedOrdersView.vue`（建议新增）：实现 08d，不把详情逻辑塞进列表行。
- `mobile/src/components/TransactionRecordsLayout.vue`：按当前状态输出正式 4-tab 窗口，而非同时展示全部 7 项。
- `mobile/src/components/TransactionOrderRecord.vue`：按现货/保证金、当前/历史字段矩阵实现精确行高和按钮状态。
- `mobile/src/components/TransactionRecordEmptyState.vue`：改为 ReceiptText 30、64px 圆底与正式文案。
- `mobile/src/core/transactionRecords.ts`：保留 canonical key/decimal helper，移除客户端作为历史金融权威的聚合责任。
- `mobile/src/core/walletLedger.ts`：接收现有 refs/bucket snapshots 及新增 trade_context/availability。
- `mobile/src/api/trading.ts`：扩展 spot/margin page/filter/read DTO，分离 order price 与 average price，接关联聚合/资产 projection。
- `mobile/src/api/wallet.ts`：补 wallet account precision 和 ledger context 映射。
- `mobile/src/core/types.ts`：补新增只读 DTO，继续使用 DecimalText。
- `mobile/src/router/index.ts`：canonical aliases、query watch 所需路由语义、关联页、隐藏底栏。
- `mobile/src/i18n/messages/zh-CN.ts`、`mobile/src/i18n/messages/en.ts`：补 7 状态、字段、availability、筛选和正式空态文案。
- `mobile/src/components/AssetMark.vue`：预计仅验证复用；除非正式截图暴露几何差异，否则无需改。

#### Rust spot

- `src/modules/spot/presentation.rs`：可选筛选、page envelope、显示/read projection DTO。
- `src/modules/spot/application/queries.rs`：offset、多状态/多条件 query 与 owner 约束。
- `src/modules/spot/infrastructure/read_models.rs`：匹配 list/count、reservation/display metadata 查询。
- `src/modules/spot/routes.rs`：只有在 handler 参数/新读取端点需要时调整；保留 JWT owner 注入。

#### Rust margin

- `src/modules/margin/presentation.rs`：扩展只读 position DTO，新增 associated response。
- `src/modules/margin/routes.rs`：新增 owner-scoped associated-orders 路由。
- `src/modules/margin/application/queries.rs`：历史聚合、关联订单、批量风险/资产 query。
- `src/modules/margin/infrastructure/position_queries.rs`：多条件全局分页、display metadata、历史 projection。
- `src/modules/margin/infrastructure/close_executions.rs`：复用切片查询并携带关联展示事实/可用性。
- 如需拆分，新增专用 `associated_orders` read model 文件，避免把只读展示逻辑写进 mutation domain。

#### Rust wallet

- `src/modules/wallet/presentation.rs`：wallet precision、ledger trade_context/fee status DTO。
- `src/modules/wallet/application.rs`：上下文查询编排，继续执行 owner 过滤。
- `src/modules/wallet/infrastructure/accounts_ledger.rs`：权威引用 join、fee availability、wallet precision；保持 list/count filter 完全一致。

#### 数据库迁移

- 首选利用现有订单、trade、ledger ref、close execution 和 reservation 事实完成只读 projection，不为纯展示复制数据。
- 若稳定 `order_no` 或用户手续费确实需要持久新事实，只能新增 migration；严禁修改 `migrations/0005_spot_orders_trades.sql`、`migrations/0015_spot_order_reservations.sql`、`migrations/0117_margin_close_executions.sql` 等已应用迁移。

### 12. 需要修改或新增的测试文件

- `mobile/tests/wallet-ledger-classification.test.ts:680-800`：替换旧 route fake mapping、卡片色和圆角断言，保留分类/分页/错误行为断言。
- `mobile/tests/pencil-wallet-flow-parity.test.ts:153-183`：把旧 card shell 断言改为 14 画板合同。
- `mobile/tests/pencil-selected-unmapped-pages.test.ts:91-102`：移除 Orders 64px 行旧基线。
- `mobile/tests/secondary-product-order-views.test.ts:28-49`：更新 Orders 导航结构和返回语义。
- `mobile/tests/ui-prototype-alignment-trading.test.ts:158-162`：更新 stale Pencil source/row 断言。
- 建议新增 `mobile/tests/transaction-records-pencil-parity.test.ts`：集中验证 7 状态×2 主题、4-tab 窗口、精确字段/几何、无假状态栏/演示数据。
- 路由/i18n/无障碍测试：验证 canonical/legacy query、关联页返回、底栏隐藏、44px hit target、ARIA/tab focus 和双语 key。
- `tests/spot_routes.rs`：多状态/筛选/offset、全局稳定分页、projection、owner isolation、Decimal string。
- `tests/margin_routes.rs`：历史重建的无平仓/部分平仓/完全平仓/legacy residual、associated owner isolation、费用 unavailable、批量风险。
- `tests/wallet_routes.rs`：trade context 的 spot/base/quote/margin 分支、self-trade 歧义、fee status、过滤 list/count 一致和全局分页。
- 相关 Rust unit tests：覆盖历史 slice 防重复、加权价格、null/availability 与 precision。

### 13. 建议验证矩阵

#### Mobile 自动验证

- `npm --prefix mobile run type-check`
- `npm --prefix mobile run type-check:tests`
- `npm --prefix mobile test`
- `npm --prefix mobile run build:pwa`
- `npm --prefix mobile run build:tauri`

#### Rust 自动验证

- `cargo fmt -- --check`
- `cargo test --test spot_routes --test margin_routes --test wallet_routes`
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`

#### 视觉与交互验证

- 在 390×920 对 7 页面浅/深主题逐一截图对照正式 HTML；验收标题/标签/筛选/行高/padding/gap/分隔线/色值/字体层级/按钮。
- 在 320×720、390×844/920、448×900 验证无横向溢出、文字不相互覆盖、长交易对/大金额/高 precision 可读。
- 验证所有可见按钮至少 44px hit area，键盘 focus、screen reader label、disabled capability 状态明确。
- 验证主题切换后没有旧卡片背景、白边、浅色 token 泄漏到深色。
- 验证 `/orders` canonical 和所有 legacy query、`symbol` 保留、同组件 query 切换、关联页返回、从资产入口打开账单后的 back fallback。
- 验证真实 loading/empty/error/append-error/unauthenticated；断网或字段 unavailable 时不能出现样本 BTC/USDT/数值。

#### 数据与 API 验证

- Spot/margin 多状态结果必须服务端全局排序/分页，不能以多个 limit 30 子列表拼接冒充全局页。
- list 与 count 使用完全相同过滤条件；offset/limit/page metadata 在边界页正确。
- 所有 owner-scoped detail/executions/associated/context join 不泄漏其他用户资源。
- DECIMAL string 从 SQL → Rust DTO → TypeScript adapter → DOM 全链保持字符串；大数和高精度不经 JS Number。
- 覆盖 market buy quote amount、partial close、fully closed、legacy body-less close、缺手续费、self-trade、缺 price observation、空 portfolio 等边界。
- 读取接口不得改变 reservation、position、wallet、ledger；mutation 行为仍通过既有确认、幂等和鉴权路径。

### 14. Files Found

- `/private/tmp/pencil-orders-module.html` — 14 张正式浅深画板及分类说明；本次视觉/字段事实源。
- `.trellis/tasks/09-01-mobile-wallet-ledger-pencil-parity-decimal-precision/prd.md` — 当前任务需求，部分 tab/空态/后端缺口描述已落后于正式导出和当前代码。
- `.trellis/tasks/09-01-mobile-wallet-ledger-pencil-parity-decimal-precision/research/pencil-ledger-and-precision.md` — 先前账单/精度研究，已声明 14 画板覆盖独立卡片合同。
- `mobile/src/views/OrdersView.vue` — 现有现货/保证金当前、历史、仓位页与真实取消/平仓动作。
- `mobile/src/views/WalletLedgerView.vue` — 现有账单筛选、分页、会话/并发保护和旧卡片 UI。
- `mobile/src/components/TransactionRecordsLayout.vue` — 尚未接入的 7-tab 共享布局草案。
- `mobile/src/components/TransactionOrderRecord.vue` — 尚未接入、几何部分接近画板的订单行草案。
- `mobile/src/components/TransactionRecordEmptyState.vue` — 尚未接入且图标/文案不符的空态草案。
- `mobile/src/components/AssetMark.vue` — 后端 logo 与确定性 fallback 图标组件。
- `mobile/src/core/transactionRecords.ts` — canonical tabs、Decimal helper 和客户端历史重建草案。
- `mobile/src/core/walletLedger.ts` — ledger 类型、过滤、分页和严格 Decimal mapping。
- `mobile/src/api/trading.ts` — spot/margin/wallet/risk/executions 适配器。
- `mobile/src/api/wallet.ts` — wallet accounts/ledger HTTP 与映射。
- `mobile/src/core/types.ts` — 资产/账户精度等共享类型。
- `mobile/src/router/index.ts` — `/orders`、`/assets/ledger` 与底栏/返回 metadata。
- `mobile/src/views/TradeView.vue` — 旧 `/orders?tab=spot|margin|positions|history` 调用方。
- `mobile/src/views/MarketDetailView.vue` — 带 symbol 的 Orders 调用方。
- `src/modules/spot/presentation.rs` — spot query/response DTO。
- `src/modules/spot/routes.rs` — spot 路由与 JWT owner 注入。
- `src/modules/spot/application/queries.rs` — spot 用户查询编排。
- `src/modules/spot/infrastructure/read_models.rs` — spot 过滤、排序、订单/保留事实查询。
- `src/modules/spot/domain.rs` — 市价/限价 quantity 与 reservation 语义。
- `src/modules/spot/application/trade_settlement.rs` — spot trade 的 ledger 引用写入。
- `src/modules/margin/presentation.rs` — margin DTO、capabilities、position/risk/execution 响应。
- `src/modules/margin/routes.rs` — margin list/detail/executions/risk/actions 路由。
- `src/modules/margin/application/queries.rs` — margin wallet/position/risk 查询编排。
- `src/modules/margin/application/lifecycle.rs` — 部分/全部平仓与 settlement 计算。
- `src/modules/margin/infrastructure/position_queries.rs` — margin position/wallet SQL read model。
- `src/modules/margin/infrastructure/close_executions.rs` — close execution owner 查询与 Decimal 映射。
- `src/modules/margin/infrastructure/settlement.rs` — remaining slice/full-close 持久化语义。
- `src/modules/wallet/presentation.rs` — wallet account/ledger DTO、完整筛选参数。
- `src/modules/wallet/application.rs` — ledger filter/query 编排。
- `src/modules/wallet/infrastructure/accounts_ledger.rs` — ledger union、全局分页/count、fee enrichment 与 wallet 查询。
- `migrations/0005_spot_orders_trades.sql` — spot order/trade 基础 schema。
- `migrations/0015_spot_order_reservations.sql` — spot reservation 字段。
- `migrations/0117_margin_close_executions.sql` — margin close slices schema。
- `mobile/tests/wallet-ledger-classification.test.ts` — 当前账单分类/路由/旧卡片合同断言。
- `mobile/tests/pencil-wallet-flow-parity.test.ts` — 当前 Pencil 钱包流程旧视觉断言。
- `mobile/tests/pencil-selected-unmapped-pages.test.ts` — 当前 Orders 旧 64px 基线断言。
- `mobile/tests/secondary-product-order-views.test.ts` — 二级订单页面导航断言。
- `mobile/tests/ui-prototype-alignment-trading.test.ts` — 交易原型对齐断言。
- `tests/spot_routes.rs`、`tests/margin_routes.rs`、`tests/wallet_routes.rs` — 对应 Rust 路由集成测试。

### 15. Related Specs

- `.trellis/spec/mobile/index.md:92` — 320/390/448 宽度与横向溢出要求。
- `.trellis/spec/mobile/index.md:125` — 触控目标至少 44px。
- `.trellis/spec/mobile/index.md:197-215` — Pencil snapshot 控制视觉、动态值必须来自 API、禁止 demo data。
- `.trellis/spec/mobile/backend-integration.md:267-328` — ledger identity、capabilities、risk、精确小数与无演示数据约束。
- `.trellis/spec/mobile/navigation-and-localization.md:253-269` — 旧独立卡片合同；已被最新用户要求替代，待专用 spec 更新流程修订。
- `.trellis/spec/backend/wallet-amount-precision.md:146-189` — ledger filters、owner scope 与全局 pagination。
- `.trellis/spec/backend/wallet-amount-precision.md:195-283` — precision 和 exact amount 契约。
- `.trellis/spec/backend/spot-orders.md` — spot 市价/限价、reservation、reference price 与订单安全约束。
- `.trellis/spec/backend/margin-trading-actions.md:19-53` — margin owner scope、executions 与时间字段。
- `.trellis/spec/backend/margin-trading-actions.md:86-90`、`:170-176` — capability/risk 及测试约束。
- `.trellis/spec/backend/order-identifiers.md:3-22` — 用户可见订单号不能直接使用内部自增主键。
- `.trellis/spec/backend/database-guidelines.md:35-73` — 已应用 migration 不可改，只能追加新 migration。
- `.trellis/spec/guides/cross-layer-thinking-guide.md` — 跨层字段与状态流验证。
- `.trellis/spec/guides/code-reuse-guide.md` — 共享 layout/record/read projection 的复用边界。

### 16. External References / Local Versions

- 本次没有使用外部网络资料；结论来自正式本地导出、仓库代码、迁移、测试与 Trellis specs。
- 导出 HTML 在 `/private/tmp/pencil-orders-module.html:9-14` 引用了未锁版本的 Tailwind CDN 和 Google Fonts（Noto Sans SC、Geist Mono）。它们是导出预览依赖，不应成为生产运行时依赖；生产字体/图标必须使用仓库锁定资产。
- `mobile/package-lock.json` 锁定的相关版本：Vue 3.5.39、Vue Router 4.6.4、vue-i18n 11.4.6、lucide-vue-next 0.563.0、Axios 1.18.1、Vite 5.4.21、TypeScript 5.9.3、vue-tsc 2.2.12。
- `Cargo.toml` 的相关约束：axum 0.7、sqlx 0.8、bigdecimal 0.4、serde 1、chrono 0.4。

## Caveats / Not Found

- 正式 HTML 给出了视觉和示例字段，但不是业务公式规范；“委托金额”“维持保证金率”“权益/占用”以及资产页覆盖哪些资产仍需产品确认。本研究明确列出歧义，不以样本数值倒推。
- 未找到保证金用户交易手续费的权威持久来源；agent commission 不能替代。该字段及依赖它的净收益必须允许 unavailable，除非后端新增真实事实源。
- 未找到现货/保证金原子 amend endpoint，也未找到 TP/SL 或策略订单能力；capability 当前明确为 false。视觉按钮/标签可以按画板展示，但交互必须禁用或呈现真实不可用状态。
- 未找到可验证的资产成本价事实，也未找到现有 margin wallet 对“权益/占用/多资产 exposure”的完整产品定义；不得客户端补算。
- 现有 detail + executions 能覆盖关联页的部分字段，但不能独立解决费用可用性、legacy 重建状态和批量查询；因此建议最小新增 owner-scoped 聚合读取 endpoint。
- PRD、旧研究和若干 specs/tests 中仍有 stale 的单一 7-tab、ClipboardList、卡片式行、时间/executions 缺失等描述；本次受 researcher 写入边界约束，只记录事实，没有修改 research 目录之外的文件。
- 本次没有执行实现测试或视觉截图回归，因为没有修改产品代码；上面的命令和矩阵是实现后的验收要求。
- 本次只新增本研究文件；未修改代码、spec、PRD、进度日志或其他任务目录，未执行 commit/push 或任何 git 操作。
