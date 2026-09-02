# Pencil 交易记录与既有账本精度链路记录

> **2026-09-02 最新状态**：用户已再次明确要求以 Pen 当前选中的“07 / 订单”整组 14 个浅/深主题画板为唯一视觉真值。主会话已通过 Pen 的 `Export HTML` 正式导出选区到 `/private/tmp/pencil-orders-module.html`，并在 Ego 390×920 视口生成 14 张像素基准图。本文件后文关于“独立响应式圆角卡片”以及把账本净变动复用成成交数量的结论只保留为历史记录，已被最新 PRD 废止；新的页面清单、接口差异和实施依据记录在同任务 `research/` 下的交易记录全模块研究文件中。当前生产实现对缺失的毛成交数量、portfolio equity/occupied 和手续费可用性均保守显示 `--`，不再从账本净额或钱包桶改名推断。

## 用户更正与权威来源

- 2026-09-02 用户将 `/assets/ledger` 的产品名称从“资金账单”更正为“交易记录”，英文固定为 `Transaction Records`。
- 旧 `y6Y7TW/m25xr0` 的 60px Header、28px 胶囊和 56px 行目标已废止；它们不再是源码、行为或视觉验收合同。
- `kcP5D/A85if` 继续是 Header、四栏导航和筛选条的已实施来源标识；其“无卡片、固定 166px”记录区已被用户最新反馈覆盖。
- Pencil transport 当前断开，本轮没有通过 Pencil 读取或修改画板。下述 Pencil 几何是先前任务已记录的历史上下文；本轮卡片合同的权威来源是用户明确更正与已提供的 Ego 运行时证据。

## 2026-09-02 Ego 运行时更正

- 主会话已在 320×720、390×844、448×900 的浅色/深色主题实测 `/assets/ledger`。更正前 `.ledger-list` 的边界为 `x=0`、`padding=0`、`gap=0`；`.ledger-row` 贴边、透明、`radius=0`、固定 166px，仅有 bottom divider。
- 旧页面在所测宽度下没有页面级横向溢出，这是必须保留的行为合同，但不能被当成视觉完成证据。
- 可见问题是：记录无独立 surface/间距，像连续表格；320px 时长总额、数量、手续费、时间和余额相互挤压；448px 时底部时间起点受其他数据长度影响而视觉漂移。
- 用户明确废止“无卡片、仅分隔线”视觉合同，要求 12px 左右画布边距、10px 卡片 gap、16px 圆角、独立明暗 surface、无滚动卡片 `backdrop-filter`、无固定高度裁切，以及稳定两列详情/footer Grid。

## 保留的 kcP5D/A85if 顶部几何

- Pencil Header 位于画板 y=28..86；生产不绘制前 28px 状态栏，由现有安全区负责。Header 内容区固定 58px，左右 padding 16px，左/右轨道各 26px。
- 返回图标是 Lucide `chevron-left` 26×26；生产用 44×44 伪元素扩大点击目标。标题 22px/700 居中，右侧 26×26 空占位。
- 四栏位于内容 y=86..138，高 52px：历史仓位、交易账单（active）、当前策略、历史策略；文本 13px，active 下划线 3px、`#18D38D`。
- 筛选条位于内容 y=138..196，高 58px，左右 padding 16px、gap 24px：币种、交易类型、右侧 Lucide `list-filter` 24px。

## 独立卡片响应式合同

- `.ledger-list` 使用 `padding: 8px 12px 0` 和 10px gap；内容底部另保留安全区。
- `.ledger-row` 保持语义化 `article[role=listitem]`，使用 16px 圆角、半透明 1px 边缘、顶部内高光和 1–2px 极轻层次；没有 `backdrop-filter`。
- 浅色画布/卡片为 `#F4F6F5 / #FFFFFF`，深色为 `#030504 / #101512`。Header、Tabs 和 Filter 仍使用白/黑 chrome，不恢复 `#0b1811` 家族。
- 卡片不再声明 `height/max-height: 166px`，仅保留 160px `min-height`并由内容增长。资产/总额是 header；事项/数量与账户+收支+成交元信息/手续费是两列详情 Grid；时间/余额是独立 footer Grid。
- 390/448px footer 保持两个独立 `minmax(0, 1fr)`，因此时间左起点不受其他值影响、余额右对齐；340px 及以下 footer 改为两行，时间和余额不再抢占同一行宽度。
- 总额、数量、手续费、时间和余额均保留 mono/tabular 视觉与独立省略边界；`title` 与 article ARIA 保留精确值。

## 主题色

| Token | kcP5D 浅色 | A85if 深色 |
| --- | --- | --- |
| chrome | `#FFFFFF` | `#000000` |
| record canvas | `#F4F6F5` | `#030504` |
| record card | `#FFFFFF` | `#101512` |
| ink | `#111714` | `#F3F7F5` |
| tab muted | `#7B8680` | `#8F9B94` |
| row muted | `#8A948F` | `#8F9B94` |
| tab bottom | `#EEF1EF` | `#18231D` |
| state/legacy row line | `#EDF1EF` | `#17221C` |
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

- `wallet-ledger-classification.test.ts`：在保留路由、筛选、logo、DecimalText、请求隔离和金融语义行为覆盖的同时，锁定独立 `article` 卡片、12px/10px 列表间距、16px 圆角、无 166px 强制高度、两列详情/footer Grid、340px 双行 footer 和新 surface token。
- `pencil-wallet-flow-parity.test.ts`：保留 Pencil ID、返回行为、可访问点击目标与真实数据约束，并把记录区验收更正为独立响应式卡片。
- `pencil-selected-page-parity-20260807.test.ts`：将交易记录页的选中画板对更新为 `kcP5D/A85if`。
- 回归 `wallet-secondary-views.test.ts` 与 `route-accessibility.test.ts`，确认钱包真实行为和双语文档标题未退化。
- 本轮 RED 阶段两个直接视觉合同测试如预期失败；实现后同两文件 23/23 通过。最终聚焦测试、应用/测试类型检查和 diff 结果在本交付末尾追加。

## 2026-09-02 本轮 Ego 复核与质量结果

- 本轮使用本地 DTO Mock 和 `prefers-reduced-motion: reduce` 重新复核 320×720、390×844、448×900 的浅色/深色页面；此复核没有读取 Pencil。
- 三种宽度的 `.ledger-list` 均为 12px 左右边距和 10px gap，卡片 x 均为 12px，宽度分别为 296/366/424px；HTML 与 Body 的 `clientWidth === scrollWidth`，页面级横向溢出均为 false。
- 320px 卡片内宽稳定为 270px、footer 为单列两行，卡片高 209px；390/448px 卡片高均为 188px，footer 为两列，每张卡的时间起点一致为 x=27，余额右边界分别稳定为 x=363/421。
- 旧全局 `.ledger-list article` 规则曾泄漏 `gap: 10px` / `justify-content: space-between` 到新 Grid；卡片现显式重置为单个 `minmax(0, 1fr)` 轨道、`gap: 0`、stretch 对齐，所有卡片内宽不再随数据长度漂移。
- 浅色卡片计算 surface 为 `rgb(255,255,255)`，深色为 `rgb(16,21,18)`；全部卡片 radius 为 16px、`backdrop-filter: none`，无内部横向溢出。
- 最终 Mobile 聚焦 5 文件 44/44 通过，`npm --prefix mobile run type-check`、`npm --prefix mobile run type-check:tests` 与 `git diff --check` 通过。

## 2026-09-02 独立复核补充

- 本次复核没有连接或读取 Pencil。源码审计发现新记录画布色同时成为 `.page` 根背景后，会透入既有 `safe-area-inset-top` 留白；桌面浏览器中该 inset 为 0，因此此前 320/390/448px 截图不能覆盖这一原生设备边界。
- `ledger-header::before` 现只按 `env(safe-area-inset-top)` 向上延伸白/黑 chrome，并随 sticky Header 覆盖真实安全区；没有绘制固定 28px 状态栏，也不改变 Header 的 58px 内容高度。
- 聚焦测试现读取真实 `prototype-base.css` 冲突规则，核对卡片对 Flex、对齐、gap、border 和直系详情 Grid 的显式重置；同时拒绝任意断点下的卡片 `height/max-height`、卡片及子区的标准/前缀 `backdrop-filter`，并逐个锁定总额、数量、手续费、时间与余额的独立省略边界。
- 页面脚本区、路由、筛选 Sheet、分页、鉴权、DecimalText、Logo 请求代际和 ARIA 数据映射均未修改；行为测试继续作为源码合同之外的独立证据。

## 相关规范

- `.trellis/spec/mobile/index.md`：Pencil 页面、44px 点击目标、320–448px 无溢出和发布门禁。
- `.trellis/spec/mobile/backend-integration.md`：DecimalText、强合同适配、分页和陈旧请求隔离。
- `.trellis/spec/mobile/navigation-and-localization.md`：业务父级返回、有效路由与中英文对称。
- `.trellis/spec/backend/wallet-amount-precision.md`：本轮未改但继续依赖的资产精度来源与账本快照合同。
