# 参考 Bitget 重构手机端现货 K 线

## Goal

参考 Bitget 手机端现货 `BTC/USDT` 行情页的内容层级和 K 线工作区形式，重构 HIPPO 手机端现货行情详情页，使报价摘要、内容切换、周期工具栏、指标信息和图表画布形成一套更清晰、更适合触控的移动端行情工作台，同时保留项目已有的实时行情与本地图表引擎。

## What I already know

- Bitget 在 390×844 手机视口中采用“交易对与交易/图表切换 → 报价摘要 → 图表/最新成交/币种信息切换 → 周期工具栏 → 大尺寸 K 线画布”的顺序。
- Bitget 图表画布约占 470px 高，主图、MA 指标和成交量同屏；当前 HIPPO 内联图表区只有约 204px，K 线主图被明显压缩。
- HIPPO 已有真实 REST + WebSocket 行情会话，支持 `1m`、`5m`、`15m`、`1h`、`1d`，并会实时合并 K 线、深度与成交数据。
- HIPPO 已内置 `klinecharts@10.0.0` 与 `lightweight-charts@5.2.0` 两种本地渲染器；此前产品约束明确禁止加载外部 TradingView iframe、脚本或行情数据。
- 当前详情页把图表、订单簿和最新成交纵向堆叠，信息密度过高；旧“行情/币种概述”导航也没有形成真正的内容分区。

## Assumptions

- “按照 Bitget 去做”指参考其手机端信息架构、尺寸比例和触控方式，而不是复制 Bitget 品牌、外部 TradingView iframe 或站点导航。
- 保留 HIPPO 深色/浅色主题、Lucide 图标、品牌色和固定底部交易操作区。
- 深度、最新成交和币种概述都使用现有真实数据；不新增演示行情或无后端支持的伪控件。

## Decision (ADR-lite)

- **Context**：直接嵌入 Bitget 使用的外部 TradingView iframe 会破坏离线/PWA/Tauri 一致性，并违反本项目本地图表引擎合同；只放大现有画布又无法解决内容堆叠和移动端层级问题。
- **Decision**：采用 Bitget 式“报价摘要 + 内容页签 + 图表工具栏 + 大画布”结构；把图表、深度、最新成交和币种概述作为同一区域的四个真实内容页签。图表继续消费 `MarketDetailView` 统一提供的 `KlinePoint[]`，渲染器切换只改变本地表现层。
- **Consequences**：行情详情首屏更接近专业交易所的图表工作台，图表可读性提高；原订单簿与成交功能不会消失，但不再与图表同时纵向挤压。布局合同测试需要同步从旧固定高度结构升级为新页签结构。

## Requirements (evolving)

- 在交易对区域提供清晰的“交易/图表”模式切换；“交易”进入当前交易对现货交易页，“图表”保持选中且不产生无效导航。
- 报价摘要保持最新价、折合价、24h 涨跌、最高、最低、成交量和成交额，并继续以共享 ticker 为唯一可见价格权威。
- 报价摘要下方提供图表、深度、最新成交、币种概述四个可访问页签；切换页签不得重启或清空行情会话。
- 图表页签提供适合拇指点击的周期工具栏，周期来源只使用 `MARKET_KLINE_INTERVALS`。
- 内联图表明显增高，主 K 线、MA5/MA10/MA20 和成交量同屏可读；展开图表、主题同步、语言更新和用户视口保持行为不退化。
- 本地 KLineChart 与本地 TradingView 模式继续可切换，禁止引入外部 iframe、脚本、数据源或远程 loader。
- 深度与最新成交页签继续展示实时数据和已有空状态；币种概述只展示现有交易对及 24h 真实字段。
- 320px 窄屏、390px 常用手机、横屏、安全区、键盘焦点、44px 触控目标和减弱动效均保持可用。

## Acceptance Criteria (evolving)

- [x] 390×844 视口中的内容顺序、分区和图表占比与 Bitget 手机端参考结构一致，且使用 HIPPO 视觉语言。
- [x] 图表内联可视高度不低于 360px，K 线、均线与成交量不会被压成不可读的窄条。
- [x] “交易/图表”切换和四个内容页签均有正确选中态、键盘语义和真实功能。
- [x] 切换图表/深度/成交/概述或本地图表引擎时，现有 REST/WebSocket 会话不会重连、清空或改写权威最新价。
- [x] `1m`、`5m`、`15m`、`1h`、`1d` 周期仍可用且形成中 K 线实时更新。
- [x] 深色与浅色模式均通过本地手机视口视觉检查；页面无横向溢出，固定操作区不遮挡内容。
- [x] 相关源码合同测试、Mobile 单测、类型检查、PWA 构建和 `git diff --check` 通过。

## Definition of Done

- 手机端行情详情模板、状态、i18n 和样式完成重构。
- 旧布局合同测试更新为新的 Bitget 参考布局合同，并保留实时行情与本地图表引擎回归覆盖。
- 使用 Ego 在 390×844 手机视口对本地页面进行图表、深度、成交、概述和主题视觉验收。
- 更新相关 Trellis 规格与 `docs/superpowers/PROGRESS.md`。

## Out of Scope

- 不复制 Bitget 品牌、文案、底部导航或登录流程。
- 不接入 Bitget/TradingView 外部 iframe、远程脚本或远程图表数据源。
- 不修改后端行情协议、行情提供商、交易下单或钱包结算逻辑。
- 不重构现货下单页、合约页、秒合约页或 PC 管理后台。

## Technical Notes

- 页面入口：`mobile/src/views/MarketDetailView.vue`。
- 本地图表门面：`mobile/src/components/MobileMarketChart.vue`。
- 本地渲染器：`mobile/src/components/KLineChartMarketChart.vue`、`mobile/src/components/TradingViewMarketChart.vue`。
- 协议与实时会话：`mobile/src/api/marketSocketProtocol.ts`、`mobile/src/core/marketDetailSession.ts`。
- 重点测试：`mobile/tests/market-detail-reference-layout.test.ts`、`mobile/tests/market-detail-stream.test.ts`。
- 研究记录：`research/bitget-mobile-chart-reference.md`。
