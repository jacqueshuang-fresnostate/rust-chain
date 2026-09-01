# Pencil 资金账单与精度链路调研

## Pencil 权威结构

- 画板：`y6Y7TW`，名称 `26 / 钱包流水 · 浅色主题`，390×920。
- Header：60px，`padding: 10px 20px`，左右 40px，返回 Lucide 22px，标题 Geist 18/750。
- Body：`padding: 6px 20px 20px`，纵向 gap 10px。
- Filters：横向 gap 8px；每项高 28px、圆角 14px、左右 padding 12px、11px 文本和 11px chevron；生效项 `#D9F9EB/#087B52`，未选项透明/`#7A8B80`。
- Ledger row：56px、左右列 gap 12px；左列名称 Geist 13/700、meta 11/450；右列金额 Geist Mono 13/650、次要信息 Geist 10/500。
- 当前画板没有 Header 动作、英文 context、介绍 Hero、日期组标题、卡片边框、模拟底部导航或空态卡片。
- 深色对应画板为 `m25xr0`，结构和全部尺寸与 `y6Y7TW` 相同；根背景为 `#000000`，主文字为 `#F2F7F4`，弱化文字为 `#7A8B80`，生效筛选为 `#103326/#61F1B6`，收入金额为 `#61F1B6`。
- 旧空态画板 `Bcug6/IVMAO` 仍保留 36px 拟物 Header 和 34px 方形筛选，未与当前 `y6Y7TW/m25xr0` 同步。因此运行时空态只复用“没有流水/调整筛选或等待钱包事件产生”和 `file-search` 的真实状态语义，Header 与筛选几何必须继续使用当前账单画板。

## 生产实现差异

- 当前 `WalletLedgerView.vue` 有 Header 刷新、账户 segmented control、横向分类 chips、日期组标题、84px 行和多层徽标。
- 现有错误/加载/空态、鉴权、分页和请求代际合同完整，应重排视觉而不是删除行为。
- Pencil 的三个筛选是资产、方向、日期；后端已有资产与起止时间字段，缺少方向白名单；Mobile 适配器尚未透传资产/时间。

## 小数溢出根因

- `wallet_ledger` 与 `margin_wallet_ledger` 金额列存储为 `DECIMAL(38,18)`，API 当前按固定 18 位字符串返回。
- `WalletLedgerEntryResponse` 没有资产精度；Mobile 的兼容逻辑只能从 `amount/fee/balance_after` 的字符串位数推断，通常得到 18。
- 资产表已经有权威 `precision_scale`，联合查询也已经关联 `assets a`，因此可零额外请求地返回 `a.precision_scale`。
- 正确修复是返回权威精度并在显示层去尾零/约束宽度，而不是把金额转成 Number、全局改变定点存储或更改账本快照。

## 相关规范

- `.trellis/spec/backend/wallet-amount-precision.md`：资产精度来源、账本金额与固定存储合同。
- `.trellis/spec/mobile/backend-integration.md`：DecimalText、强合同适配、陈旧请求隔离与真实数据状态。
- `.trellis/spec/mobile/index.md`：钱包 Pencil 页面、Header 和 320–448px 无溢出合同。
