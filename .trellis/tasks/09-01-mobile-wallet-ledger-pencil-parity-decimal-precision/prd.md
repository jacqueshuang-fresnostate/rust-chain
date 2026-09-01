# 手机资金账单 Pencil 1:1 复刻与金额精度治理

## Goal

依据 Pencil 当前资金账单画板 `y6Y7TW`，1:1 重构手机端 `/assets/ledger` 的静态视觉与交互层级，同时保留真实账本分页、鉴权、国际化和异常状态，并解决账本接口缺少资产精度导致长小数撑破移动端布局的问题。

## What I Already Know

- Pencil MCP 当前没有节点选中，但资金账单路由的权威画板是 `y6Y7TW`（`26 / 钱包流水 · 浅色主题`），已通过 MCP 读取完整节点树和截图。
- 画板为 390px 宽：60px 二级 Header、`20px` 水平边距、`6px` 顶部间距、三个 28px 胶囊筛选、56px 连续流水行。
- Header 只包含返回、居中标题和右侧 40px 空占位；没有刷新按钮、英文副标题或页面介绍区。
- 每行采用左侧名称/元信息与右侧金额/次要信息的两列布局，不显示卡片、日期分组标题、内部 ID 或演示金额。
- 当前生产页使用账户分段与横向业务分类芯片、84px 日期分组行和 Header 刷新按钮，视觉结构与画板不一致。
- 现有账本 API 已支持资产、变动类型、账户、分类和起止时间过滤，但 Mobile 适配器只透传分类、账户和变动类型。
- 后端账本响应没有返回 `assets.precision_scale`，Mobile 只能从 `DECIMAL(38,18)` 字符串推断为 18 位，从而在列表显示过长尾数。
- 工作区存在前序 Admin 任务未提交改动；本任务不得覆盖、回滚或混入这些文件。

## Requirements

### Pencil 视觉复刻

- `/assets/ledger` 根画布继续使用钱包页浅色纯白、深色纯黑主题合同。
- Header 精确采用 60px 高、20px 左右内边距、40px 返回/占位轨道、22px Lucide 返回图标、18px 居中标题；移除 Header 刷新动作、英文 eyebrow 和描述。
- 内容区采用 `padding: 6px 20px 20px` 与 10px 区块间距。
- 顶部只显示三个 28px 胶囊筛选：资产、方向、日期；当前生效筛选使用薄荷底和深绿色文字，未选筛选透明且使用弱化文字。
- 流水采用无卡片、无日期分组标题的连续 56px 行；左列显示业务名称和真实元信息，右列显示金额和真实次要信息。
- 行内字体、字号、字重、间距、颜色和对齐对齐 `y6Y7TW`；保留 Lucide-only、无表情符号。
- 320px、390px、448px 宽度无横向溢出；金额列不得把左列挤出或突破视口。

### 真实筛选与交互

- 资产筛选使用真实钱包资产符号，不写死 USDT/BTC；选择后调用账本接口的 `asset_symbol`。
- 方向筛选使用 `all | credit | debit`，分别表示全部、收入（`amount > 0`）和支出（`amount < 0`），在后端分页前过滤。
- 日期筛选提供全部、今天、最近 7 天、最近 30 天预设，使用账本接口 `start_time`/`end_time`，不得仅对当前页做客户端过滤。
- 三个筛选控件使用与现有手机端底部 Sheet 合同一致的可访问选择层：标题、当前项、关闭、遮罩/Escape、焦点管理和 44px 点击目标。
- 任一筛选变化后使当前请求失效、清空旧页并从 offset 0 重新请求；加载更多继续沿用当前筛选条件。
- 保留登录提示、首次加载、空态、首屏错误、已有数据追加错误、重试和分页行为，不使用 Pencil 演示数据替代接口结果。

### 金额精度

- 钱包账本列表/详情行响应新增 `precision_scale`，直接来自关联资产的 `assets.precision_scale`，范围保持 `0..=18`。
- Mobile 账本适配器将 `precision_scale` 作为必填权威字段，不再从数据库固定 18 位字符串猜测显示精度。
- 列表金额按资产精度去除无意义尾零，并采用受限宽度、等宽数字和自适应字号；完整精确值仍保留在可访问标签/详情信息中。
- 本任务不改变账本金额、余额、手续费的数据库值或资金计算，仅修复响应元数据和展示。

## Acceptance Criteria

- [x] 生产页关闭所有弹层时与 Pencil `y6Y7TW` 的 Header、筛选条、56px 行、间距、字体层级和颜色 1:1 对齐。
- [x] 页面不再显示旧账户分段、横向分类芯片、日期分组标题、Header 刷新按钮或额外介绍区。
- [x] 资产、方向和日期三个筛选均可打开选择层并驱动服务端分页查询。
- [x] 筛选切换、并发响应、错误重试和加载更多不会混入旧条件数据。
- [x] API 每条账本记录返回正确 `precision_scale`，Mobile 拒绝缺失或越界字段。
- [x] 18 位存储字符串不会在 320px 设备上产生横向页面溢出，完整 Decimal 文本不会进入 Number/parseFloat/toFixed 财务路径。
- [x] 浅色、深色、加载、空、错误、已有数据追加错误与未登录状态完成验证。
- [x] 后端聚焦测试、Mobile 聚焦测试、类型检查、lint、release gate 和 `git diff --check` 通过。

## Definition of Done

- 后端账本 DTO/SQL/过滤合同与测试完成。
- Mobile API/Core/View/i18n/行为测试完成。
- 使用 Pencil MCP 和 Ego 浏览器分别完成结构与运行时视觉复核。
- 相关 Backend/Mobile code-spec 与 `docs/superpowers/PROGRESS.md` 更新。

## Technical Approach

- 后端联合账本源继续复用一个 `assets` JOIN，在共享 select 中加入 `a.precision_scale`；新增方向白名单并让行查询/COUNT 复用同一谓词。
- 扩展 Mobile `WalletLedgerFetchOptions` 和请求生命周期快照，使资产、方向、日期与既有 session/filter generation 一起参与陈旧响应判定。
- 将页面模板压缩为 Pencil 的 Header + 三胶囊 + 连续列表；筛选详情放入默认关闭的底部 Sheet，不改变静态基线画面。
- 以 DecimalText 保留业务值，列表显示通过资产精度格式化并通过 CSS 控制最小宽度和溢出；不在财务链路转换为 JS Number。

## Decision (ADR-lite)

**Context**：只改 CSS 会继续保留错误的信息层级；只在客户端过滤方向/日期会让分页总数和后续页面不正确；从固定 18 位数据库文本推断精度会制造冗长小数。

**Decision**：以 Pencil `y6Y7TW` 为静态视觉真值，在底部 Sheet 中承载真实筛选；方向、日期和资产均在服务端分页前过滤；账本响应显式携带资产精度。

**Consequences**：关闭 Sheet 时可保持 1:1 基线，真实筛选与分页仍正确；接口是加法字段与白名单扩展，旧调用方可忽略新增字段，Mobile 新版本严格要求该字段。

## Out of Scope

- 不修改账本写入、余额结算、手续费计算或数据库表结构。
- 不把 Pencil 演示状态、网络或金额写入生产数据。
- 不重构 PC 端或 Admin 资金账单。
- 不在本任务中全局改变所有 BigDecimal API 的固定 18 位序列化合同；仅修复资金账单缺少资产精度所造成的展示问题。

## Technical Notes

- Pencil：`mobile/pencil/hippo-mobile-uiux.pen`，画板 `y6Y7TW`。
- Mobile：`mobile/src/views/WalletLedgerView.vue`、`mobile/src/core/walletLedger.ts`、`mobile/src/api/wallet.ts`、中英文 locale 与账单测试。
- Backend：`src/modules/wallet/{presentation,application}.rs`、`src/modules/wallet/infrastructure/accounts_ledger.rs`、`tests/wallet_routes.rs` 与单元测试。
- 规范：`.trellis/spec/mobile/backend-integration.md`、`.trellis/spec/backend/wallet-amount-precision.md`。
