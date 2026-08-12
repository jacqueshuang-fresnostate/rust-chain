# 手机 K 线统一切换 Lightweight Charts

## Goal

将手机端现货交易页和行情详情页共用的 K 线渲染器统一为 npm/Vite 本地打包的 `tradingview/lightweight-charts`，去除 KLineChart 双引擎与运行时选择层，同时保持现有 HIPPO REST/WebSocket 行情、实时形成中蜡烛、均线、成交量、主题、语言、缩放和平移合同不变。

## Requirements

* `MobileMarketChart` 仅挂载一个基于 `lightweight-charts@5.2.0` 的本地图表，不加载 TradingView iframe、widget、远程脚本或外部数据源。
* 删除 `klinecharts` 依赖、旧 `KLineChartMarketChart.vue` 和图表引擎偏好/切换 UI；清理相关无效 i18n 文案与父组件参数。
* 保留真实 OHLCV、MA5/MA10/MA20、成交量、最后一根蜡烛增量更新、追加更新和替换历史时的时间戳锚定视口恢复。
* 主题和语言在原图表实例上更新；触摸拖动、双指缩放、横向惯性和安全 resize 适配手机 WebView。
* 遵守 Lightweight Charts 署名要求，在图表内启用官方 attribution logo/link，不额外引入业务数据请求。
* 现货与行情详情的 REST/WebSocket 会话、交易路由、盘口和成交逻辑不得改变。

## Acceptance Criteria

* [x] `mobile/package.json` 和 lockfile 仅保留 `lightweight-charts@5.2.0`，不存在 `klinecharts` 运行时依赖。
* [x] 现货交易页与行情详情页均通过 `MobileMarketChart` 使用本地 Lightweight Charts。
* [x] 不再显示图表引擎切换按钮，也不再读取/写入 `hippo_mobile_market_chart_engine`。
* [x] 实时同蜡烛更新使用 series `update`，新蜡烛追加且不重置用户视口；区间更换后的完整数据才执行 fit。
* [x] 明暗主题、中文/英文、ResizeObserver、卸载清理、手势滚动和缩放合同通过回归测试。
* [x] Mobile 聚焦测试、全量测试、type-check、PWA build、Tauri build 和 `git diff --check` 通过。

## Definition of Done

* 代码、依赖、测试、Mobile 规范和进度记录同步更新。
* 无远程 TradingView 集成；市场数据仍完全由现有后端接口提供。

## Technical Approach

复用现有 `TradingViewMarketChart.vue` 的 Lightweight Charts v5 实现，将其重命名为与厂商无关的 `LightweightMarketChart.vue`，由 `MobileMarketChart.vue` 唯一挂载。保留通用 `classifyMarketChartDataUpdate` 与 logical viewport 工具，删除只服务于 KLineChart 的 period、symbol precision、bar-space viewport 与持久化引擎选择工具。

## Decision (ADR-lite)

**Context**：项目当前已经同时安装并实现 KLineChart 与 Lightweight Charts，双引擎增加包体、测试面和设置层；用户明确要求统一切换。

**Decision**：将 Lightweight Charts 设为手机端唯一 K 线引擎，并启用官方 attribution logo 满足公开应用署名链接要求。

**Consequences**：包体和维护面收敛，手机触摸体验更统一；KLineChart 特有 DataLoader/内置指标能力退出，均线继续由本地纯函数计算并渲染。

## Out of Scope

* 不修改后端 K 线、ticker、盘口或成交接口。
* 不接入 TradingView Advanced Charts、Widget、iframe 或第三方 datafeed。
* 不改秒合约当前微型折线图。

## Technical Notes

* 当前项目已经固定 `lightweight-charts@5.2.0`，现有组件支持 Candlestick/Histogram/MA、theme observer、locale 原地更新和 timestamp-anchored viewport。
* Lightweight Charts 官方仓库描述其为基于 HTML5 Canvas 的高性能金融图表；许可证要求公开应用提供 TradingView attribution/link，内建 attribution logo 可满足该要求。
* 相关规范：`.trellis/spec/mobile/pwa-and-shell.md`、`.trellis/spec/mobile/backend-integration.md`。
