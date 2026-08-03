# 本地 K 线框架选型

## 目标

为行情详情页提供本地优先且可切换的双图表引擎：专业 K 线使用 `klinecharts`，TradingView 模式继续使用 `lightweight-charts`。两者均随应用本地打包且不请求外部图表资源或数据源。

## 官方资料

- KLineChart 官方仓库：<https://github.com/klinecharts/KLineChart>
  - HTML5 Canvas 金融 K 线框架；零依赖；约 40 KB gzip；内置指标和画线；支持移动端；完整 TypeScript 类型；Apache-2.0。
- KLineChart 官方环境指南：<https://klinecharts.com/en-US/guide/environment>
  - 生产项目推荐通过 npm/pnpm/yarn/bun 与 ESM 构建工具安装，不依赖 CDN；移动端要求现代 WebView，并关注触摸滚动冲突、容器尺寸和 resize。
- KLineChart 官方快速上手：<https://klinecharts.com/guide/quick-start>
  - 支持 Vue；在组件挂载后 `init`，通过本地数据加载器提供 OHLCV 数据。
- npm 官方包页：<https://www.npmjs.com/package/klinecharts>
  - 当前稳定版 `10.0.0`，内置 TypeScript 声明，运行时依赖为 0，Apache-2.0。
- KLineChart Pro 官方入门：<https://pro.klinecharts.com/en-US/getting-started.html>
  - Pro 默认数据能力可对接 Polygon API；本项目不采用 Pro，避免额外数据源和产品层依赖。
- TradingView Lightweight Charts 官方文档：<https://tradingview.github.io/lightweight-charts/>
  - 可通过 npm 安装并在项目中 ESM import；是 Canvas 金融图表库。
- TradingView `LayoutOptions.attributionLogo`：<https://tradingview.github.io/lightweight-charts/docs/5.1/api/interfaces/LayoutOptions>
  - 该可选 Logo 会生成指向 TradingView 的外部链接；本地终端在初始化与主题更新时均显式设为 `false`，避免运行时外部锚点。
- TradingView Lightweight Charts 官方仓库：<https://github.com/tradingview/lightweight-charts>
  - 当前项目使用的 `5.2.0` 为 ESM、TypeScript、Apache-2.0。

## 方案比较

### A. 本地双引擎（采用）

- 默认 `klinecharts@10.0.0`，可切换到本地 `lightweight-charts@5.2.0` TradingView 模式。
- 两者均通过 npm 安装并由 Vite 打入 PWA/Tauri/Android 产物，不加载 CDN、iframe、远程脚本、在线 Widget 或图表服务。
- KLineChart 专门面向 K 线，原生支持移动缩放、拖动、主图指标、成交量窗格、样式覆盖和 TypeScript；运行时依赖为 0。
- TradingView 模式延续已经验证的蜡烛、MA、成交量、主题和实时更新适配，但关闭可选 attribution Logo，不在本地图表 DOM 中生成外部链接。
- HIPPO 持有全部数据加载与实时合并逻辑；两个引擎只消费同一份本地 `KlinePoint[]`，切换不重连行情。

### B. 仅继续 `lightweight-charts`

- 当前实际也是 npm 本地打包，并不调用 TradingView 数据接口。
- 保留 TradingView 体验，但无法提供用户要求的独立专业 K 线框架模式。

### C. 自研 Canvas

- 可完全掌控，但需要自行承担坐标轴、十字线、缩放、惯性、DPR、指标窗格和无障碍维护成本。
- 在已有成熟零依赖框架时收益不足，且真机回归风险更高。

## 集成约束

1. 安装基础包 `klinecharts@10.0.0`，保留本地 `lightweight-charts@5.2.0`；不得引入 KLineChart Pro 或 TradingView 在线 Widget/Charting Library。
2. 源码、构建产物和图表 DOM 不得出现远程 KLineChart/TradingView 脚本、CDN、iframe、外部锚点、Pro/default datafeed；TradingView 本地模式在 create/apply options 中均关闭 attribution Logo。
3. `MarketDetailView` 的 REST/WebSocket、竞态隔离、周期切换和真实数据优先级保持不变。
4. `MobileMarketChart` 在挂载后初始化，在卸载时 dispose；主题、ResizeObserver、低动态和沉浸模式继续工作。
5. 两个模式都展示真实蜡烛与 MA5/MA10/MA20、真实成交量；页面外部图例继续由同一真实 K 线数组计算。
6. 同一形成中蜡烛更新不得重置用户缩放/拖动视口；只有首批数据或周期切换可重置可视范围。
7. 图表引擎选择持久化在本地；只挂载当前引擎，切换不得重建 REST/WebSocket 会话。
8. 测试同时验证双包依赖、provider 标记、无外部图表 URL、引擎切换、指标/成交量创建、主题和清理合同。
