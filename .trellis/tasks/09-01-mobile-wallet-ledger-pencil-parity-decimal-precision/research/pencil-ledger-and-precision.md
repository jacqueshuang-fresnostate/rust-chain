# Pencil 交易记录与既有账本精度链路记录

## 用户更正与权威来源

- 2026-09-02 用户将 `/assets/ledger` 的产品名称从“资金账单”更正为“交易记录”，英文固定为 `Transaction Records`。
- 旧 `y6Y7TW/m25xr0` 的 60px Header、28px 胶囊和 56px 行目标已废止；它们不再是源码、行为或视觉验收合同。
- 新唯一视觉真值是 `mobile/pencil/hippo-mobile-uiux.pen` 的 `kcP5D`（浅色）与 `A85if`（深色），画布均为 390×920。
- 本轮权威几何和色值由主会话提供，不再连接 Pencil MCP。主会话已用 390px Ego 与本地真实 DTO Mock 验证新页面几何基本对齐，并给出事项、方向和手续费语义修正。

## kcP5D/A85if 精确几何

- Pencil Header 位于画板 y=28..86；生产不绘制前 28px 状态栏，由现有安全区负责。Header 内容区固定 58px，左右 padding 16px，左/右轨道各 26px。
- 返回图标是 Lucide `chevron-left` 26×26；生产用 44×44 伪元素扩大点击目标。标题 22px/700 居中，右侧 26×26 空占位。
- 四栏位于内容 y=86..138，高 52px：历史仓位、交易账单（active）、当前策略、历史策略；文本 13px，active 下划线 3px、`#18D38D`。
- 筛选条位于内容 y=138..196，高 58px，左右 padding 16px、gap 24px：币种、交易类型、右侧 Lucide `list-filter` 24px。
- 流水从内容 y=196 开始；每行高 166px、padding `12px 18px`、纵向 gap 9px，仅 bottom 1px border。四行起点为 y=12/51/82/113，高度为 30/22/22/19px。
- 文本合同：symbol 20/650、总额 18/500；第二行事项 15/600、数量标签 13/500 muted、数量 15/500；账户 14/600、方向 15/650、成交元信息和手续费 12/500；时间 13/400、余额标签 13/500、余额值 14/500。

## 主题色

| Token | kcP5D 浅色 | A85if 深色 |
| --- | --- | --- |
| page | `#FFFFFF` | `#000000` |
| ink | `#111714` | `#F3F7F5` |
| tab muted | `#7B8680` | `#8F9B94` |
| row muted | `#8A948F` | `#8F9B94` |
| tab bottom | `#EEF1EF` | `#18231D` |
| row bottom | `#EDF1EF` | `#17221C` |
| active | `#18D38D` | `#18D38D` |
| credit/positive | `#0DBE7B` | `#45EFAE` |
| debit/negative | `#FF5878` | `#FF5878` |

## 生产数据映射

- `WalletLedgerEntry` 提供 `accountType`、`symbol`、`changeType`、`category`、`amount`、`fee`、`balanceAfter`、`precisionScale` 与 `createdAt`；没有交易对和交易 side。
- `fetchWalletAccounts()` 已提供 symbol 与可选 `logoUrl`。页面按 token/session generation 加载资产目录，构建 symbol → logo URL 映射，并交给既有 `AssetMark` 处理 URL 与语义化 symbol 回退。
- 第二行左侧不能永久显示 `--`。由于没有交易对字段，页面使用现有 `entryLabel(entry)` 将真实 `changeType` 本地化为交易事项名称，不猜出 `SYMBOL/USDT`。
- 第三行中间文字必须表达 amount 方向：正数调用现有 `directionCredit`（收入），负数调用 `directionDebit`（支出），零值显示 `--`。这里不再重复“现货成交/现货结算”，也不把收入/支出包装成买入/卖出。
- 数量与成交元信息复用该账本事件的真实绝对变动额；余额直接来自账本快照。所有数值继续通过 DecimalText 与 `precisionScale` 格式化。
- 后端 fee 合同是非负手续费。视觉上手续费代表扣除，因此 `walletLedgerFeeDebitAmount()` 对非零 fee 生成负 DecimalText，零保持 `0`；格式化前后均不进入 Number/parseFloat/toFixed。
- 时间只由真实 `createdAt` 生成本地 `YYYY/MM/DD HH:mm:ss`；没有任何 Pencil 演示数据落入生产模板。

## 顶栏路由审计

- `OrdersView` 已支持 query `tab=positions | margin | history`，因此四栏映射为：
  - 历史仓位 → `{ name: 'orders', query: { tab: 'positions' } }`
  - 交易账单 → `{ name: 'wallet-ledger' }`
  - 当前策略 → `{ name: 'orders', query: { tab: 'margin' } }`
  - 历史策略 → `{ name: 'orders', query: { tab: 'history' } }`
- 以上均为现有真实路由；未创建新路由、未改 `OrdersView`。

## 保留的行为与精度合同

- 页面继续使用既有 `createWalletLedgerPaginationController`；资产、方向、日期和 session/filter generation 都参与请求身份，旧请求、旧会话和旧筛选响应不会写回。
- 首屏错误、追加错误、空态、首次加载、加载更多、同 offset 重试和原始行数推进 offset 保持不变。
- 三个筛选继续复用可访问底部 Sheet 与 `useModalDialog` 的遮罩、Escape、焦点陷阱和焦点恢复。
- 后端 `precision_scale`、严格 Mobile 适配器和 DecimalText 金额路径是前一实现中已完成且继续有效的精度修复；本轮没有修改后端或资金逻辑。

## 已实施的聚焦合同

- `wallet-ledger-classification.test.ts`：锁定 `kcP5D/A85if`、专用 Header、四栏有效路由、筛选、166px 行、逐层字体、精确色值、logo 元数据和 DecimalText；行为测试覆盖正/负/零 amount 到收入/支出/null 的映射，以及非零/零 fee 的扣除文本。
- `pencil-wallet-flow-parity.test.ts`：更新 Pencil ID、明暗主题、连续列表、返回行为、可访问点击目标与真实数据约束。
- `pencil-selected-page-parity-20260807.test.ts`：将交易记录页的选中画板对更新为 `kcP5D/A85if`。
- 回归 `wallet-secondary-views.test.ts` 与 `route-accessibility.test.ts`，确认钱包真实行为和双语文档标题未退化。
- 最新结果：上述 5 文件 43/43 通过，`npm --prefix mobile run type-check` 与 `npm --prefix mobile run type-check:tests` 通过；最终多宽度明暗主题视觉复核与完整 release gate 留给主会话收口。

## 相关规范

- `.trellis/spec/mobile/index.md`：Pencil 页面、44px 点击目标、320–448px 无溢出和发布门禁。
- `.trellis/spec/mobile/backend-integration.md`：DecimalText、强合同适配、分页和陈旧请求隔离。
- `.trellis/spec/mobile/navigation-and-localization.md`：业务父级返回、有效路由与中英文对称。
- `.trellis/spec/backend/wallet-amount-precision.md`：本轮未改但继续依赖的资产精度来源与账本快照合同。
