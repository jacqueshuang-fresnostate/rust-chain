# 手机资金划转与杠杆账户资产可见性

## Goal

将生产手机端 `AssetsView` 的资金划转弹窗按 Pencil 画板 `v6phV` / `TuWXq` 与资产选择画板 `tPkL1` / `tPkD1` 重构，并补齐“后台决定哪些资产允许转入杠杆账户、用户端能独立查看杠杆账户余额”的跨层合同。在保持真实钱包、幂等划转、焦点管理和错误反馈合同不变的前提下，实现数量英雄区、毛玻璃路径仪器条、资产持仓行、二级资产选择弹窗、后台资产开关和现货/杠杆余额分账展示。

## What I already know

- Pencil 设计已在 `docs/superpowers/specs/2026-08-13-transfer-sheet-immersive-design.md` 和 `mobile/pencil/scripts/37-transfer-sheet-immersive.js` 中明确记录尺寸、层级、色彩与文案。
- 主划转画板为 `v6phV`（浅色）和 `TuWXq`（深色）；资产选择画板为 `tPkL1`（浅色）和 `tPkD1`（深色）。
- 生产实现位于 `mobile/src/views/AssetsView.vue`，已接入真实现货/杠杆钱包、真实资产 Logo、`/margin/transfers`、幂等键、返回钱包快照和共享 `useModalDialog`。
- 当前生产弹窗仍使用并排描边账户盒、原生 `<select>` 和普通数量输入框，与 Pencil 结构不一致。
- `assets` 当前没有杠杆转入开关，`POST /margin/transfers` 只校验资产是否 active，因此任意启用资产都能转入杠杆钱包。
- `GET /margin/wallets` 当前只返回已经创建的 `margin_wallet_accounts` 行，手机资产页又把现货和杠杆余额合并展示，用户无法明确看到杠杆账户余额。
- 当前 Pencil MCP 的 VS Code 传输不可用，匹配版本的 headless CLI 又因设计资源 Base URI 无法加载画布；本任务使用仓库中已保存的 Pencil 画板 ID、设计规格和生成脚本作为精确来源，不读取或解析 `.pen` 文件内容。

## Requirements

### 主划转 Sheet

- 保留底部 Sheet、遮罩、Grab Bar、标题和关闭按钮，Sheet 宽度、顶部圆角、安全区与 Pencil 画板一致。
- 数量英雄区放在路径条之前，显示 `划转数量 · {asset}`、大号十进制输入、真实可划转余额以及“全部”按钮。
- 空数量输入显示 `0.00`；不得把占位值作为真实提交金额。
- “全部”使用当前来源钱包的真实 `available`，来源钱包缺失时保持不可用且显示 `--`。
- 路径改为单条毛玻璃仪器条：左侧来源账户、中间 mint 圆形交换按钮、右侧目标账户。
- 资产改为无原生 select 外观的持仓行，使用 `AssetMark` 和后端返回的 Logo；点击进入资产选择二级 Sheet。
- 提示与主按钮沿用 Pencil 文案和 50px mint 主按钮，同时保留提交中、成功和错误反馈。

### 资产选择 Sheet

- 资产行点击后在同一 Teleport 覆盖层中切换到 Pencil `39b` 二级 Sheet，不创建重叠的两个 `aria-modal` 对话框。
- 提供毛玻璃搜索框、当前来源账户的真实资产列表、真实 Logo、真实可划转余额和选中勾选态。
- 搜索按资产符号大小写不敏感过滤；无结果显示本地化空状态。
- 选择资产后返回主划转 Sheet，并清空旧数量与旧反馈，避免把上一资产数量提交到新资产。
- 二级 Sheet 的关闭按钮和 Escape 先返回主 Sheet；主 Sheet 的关闭按钮、遮罩和 Escape 才关闭整个划转流程。

### 状态、数据与可访问性

- 保留 `transferWalletFunds` 参数、幂等请求与后端返回的现货/杠杆钱包快照更新逻辑，不增加额外刷新。
- 切换来源账户后只允许选择该来源真实存在的钱包资产；不存在来源钱包时显示 `--` 并禁用提交。
- 提交中禁止关闭、交换方向、打开资产选择器和重复提交。
- 所有新增用户文案进入中英文 i18n，不使用表情符号；图标仅使用 Lucide。
- 保留 Teleport、焦点陷阱、焦点恢复、Escape、滚动锁和不小于 44px 的触控目标。
- 320×720、390×844、448×900 下均不得横向溢出；短屏时 Sheet 内容可纵向滚动，但标题和主按钮保持可见。
- 支持浅色、深色和 `prefers-reduced-motion`。

### 后台可划转资产配置

- 为资产增加 `margin_transfer_enabled` 布尔配置；新建资产默认关闭，避免未经配置的资产直接进入杠杆账户。
- 新迁移只为已经被杠杆产品引用或已经存在杠杆钱包记录的历史资产回填开启，既保留真实存量业务，又不把无关资产自动放开。
- 后台资产列表、详情、新增和修改均返回/提交该字段；列表以中文“允许转入杠杆”显示，新增和修改表单使用明确的开关控件。
- 新发起的现货到杠杆划转必须同时满足资产 active 且 `margin_transfer_enabled=true`；关闭开关后仍允许已有杠杆余额转回现货，避免资金被困在杠杆账户。
- 已成功划转的同幂等键重放仍返回原结果，不受资产开关后续变化影响。

### 用户端杠杆余额可见性

- `GET /margin/wallets` 返回所有“允许转入”的 active 资产（即使用户尚未创建杠杆钱包也返回零余额），同时保留已有杠杆钱包资产，并明确返回 `margin_transfer_enabled`。
- 手机端现货转入时只展示后端允许转入的资产；杠杆转出现有余额时不因开关关闭而隐藏。
- 资产页提供“总览 / 现货账户 / 杠杆账户”账户维度；现货账户和杠杆账户使用上下排列的全宽卡片，杠杆账户选中后独立展示真实 available、frozen、locked、Logo、估值与币种数量，不再只能看到现货和杠杆的合计。
- 账户维度切换不触发额外资金请求，复用同一次现货钱包与杠杆钱包响应；加载、错误、空状态及余额隐藏逻辑保持真实。

## Acceptance Criteria

- [x] 生产主划转 Sheet 的结构顺序为标题、数量英雄、路径仪器条、资产行、提示/反馈、主按钮。
- [x] 主划转 Sheet 不再包含原生 `<select>`，资产选择使用二级 Sheet。
- [x] 数量英雄显示真实资产和真实可用余额；“全部”不会在余额缺失时伪造 `0`。
- [x] 路径交换、资产搜索/选择、金额校验、成功钱包快照更新和错误反馈行为通过测试。
- [x] 资产列表使用后端 Logo，并具备选中态、空状态和真实余额。
- [x] 焦点、Escape、遮罩、提交中锁定和安全区合同不回退。
- [x] `data-pencil-source` 包含 `v6phV TuWXq tPkL1 tPkD1`。
- [x] Mobile 聚焦测试、全量测试、type-check、PWA build、Tauri build 和 `git diff --check` 通过。
- [x] 后台资产可新增/修改 `margin_transfer_enabled`，列表与审计快照能读取该配置。
- [x] 关闭开关后，新的现货转入杠杆请求被拒绝，杠杆转回现货与既有幂等重放继续可用。
- [x] `/margin/wallets` 返回配置资产的零余额行、已有杠杆钱包余额、后端 Logo 和 `margin_transfer_enabled`。
- [x] 手机端现货转入资产列表只包含允许资产，并以上下排列的全宽卡片独立展示现货与杠杆账户真实余额。
- [x] 后端、后台 Web、Mobile 聚焦与全量质量门禁通过。

## Definition of Done

- SQL 迁移、后端 DDD 层、后台 Web、生产 Vue、i18n、回归测试、Backend/Admin/Mobile 规范和 `docs/superpowers/PROGRESS.md` 同步更新。
- 设计来源只作为视觉合同；不修改用户当前未提交的 Pencil 画板、脚本、清单或设计规格。
- 不改变划转请求字段、幂等键、双钱包固定锁序或两侧流水原子提交语义。

## Out of Scope

- 资产页主页面、充值、提现、账单入口的重新设计。
- Pencil 画板本身的再次生成或保存。
- 新增第三种账户类型或跨链划转。

## Technical Notes

- 视觉来源：`docs/superpowers/specs/2026-08-13-transfer-sheet-immersive-design.md`。
- Pencil 主 Sheet 关键值：高度约 520px、顶圆角 20px、内部 16px、主按钮 50px、Amount Hero 30/700、Route Bar 52px、Asset Row 52px。
- 生产数据合同：`.trellis/spec/mobile/backend-integration.md` 的 Selected Financial Confirmation Handoff。
- Shell/弹窗合同：`.trellis/spec/mobile/pwa-and-shell.md` 和 `.trellis/spec/mobile/index.md`。
