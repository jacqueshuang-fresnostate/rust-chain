# HIPPO 手机端行情终端与全局视觉系统重构

## Goal

参考用户提供的专业交易所行情截图，并结合 Awwwards、Wise、Linear、Coinbase Advanced、OKX 等案例，经 Ego Browser 审计后重构手机端全局视觉系统和关键页面。目标是形成统一的 `HIPPO Instrument Editorial`：发现型页面有鲜明的编辑式品牌主舞台，交易与数据页面保持边到边、精密、紧凑的专业终端质感，同时保留明暗主题、真实后端数据、PWA/Tauri/Android 能力和已经完成的 K 线/订单簿/成交 WebSocket 实时刷新。

K 线渲染必须提供随 PWA/Tauri/Android 产物本地打包的双引擎：默认专业 KLineChart，同时保留可切换的 TradingView Lightweight Charts；不依赖外部图表页面、远程脚本、CDN、iframe、在线 Widget 或第三方行情服务。

在实现层之上新增一份由 Pencil CLI/MCP 引擎生成、可版本管理的全手机端 UI/UX 设计源文件。设计必须直接以当前已恢复的首页视觉为母版：冷白/近黑画布、淡网格、发丝分隔、薄荷主动作、珊瑚风险色、Geist/Geist Mono 数据排版、低圆角和当前选中画板的五入口浮动交易 Dock。Pencil 设计不是独立概念稿，而是覆盖现有路由、真实功能和状态边界的后续实现蓝图。

## Pencil Selected Source of Truth — 2026-08-03

本轮生产实现必须直接读取 Pencil 当前选中节点，而不是依据旧导出图、旧脚本名称或文字描述推测。当前活动文件为 `mobile/pencil/hippo-mobile-uiux.pen`，用户选中的顶层生产基准为：

- `FwNBM` — `01 / Home / Light · Guest`；
- `W1cWyh` — `02 / Home / Dark · Guest`；
- `miHnt` — `03 / Home / Light · Member`；
- `CvipW` — `04 / Home / Dark · Member`；
- `ftTny` — `04 / Market Detail · Light`；
- `VoZfE` — `05 / Market Detail · Dark`；
- `yzOPc` — `06 / Spot Trading · Light`；
- `bo8k5` — `07 / Spot Trading · Dark`。

用户随后扩展了同一次 Pencil 选择。本任务还必须把下列尚未映射到生产端的浅色/深色画板作为唯一结构基准：

- 资产：`CUK3y` / `i6YDBr`；
- 我的（访客）：`dUqOS` / `duJTW`；我的（会员）：`S23rM` / `S0Bj8`；
- 订单（现货）：`kcP5D` / `A85if`；订单（杠杆）：`n6oGO` / `t2GTW4`；
- 登录：`u99Fpg` / `WNbsc`；注册：`MCuqb` / `RGYGj`；
- 资讯：`VGPW0` / `b6EGF`；资讯详情：`Q50Rgr` / `ASvmq`；
- 闪兑：`x9T4CL` / `eXdnN`；闪兑币种面板：`sf288` / `xvVss`；
- 理财：`zIzOm` / `tCHZ9`；借贷：`kIOBX` / `yrsRy`；
- 新币：`oOJ0q` / `ZTtvY`；新币详情：`nFwYy` / `B6Qh9J`。

本扩展只处理当前选中的顶层页面；未被选择的消息中心、安全中心、KYC、充提币等页面不在本切片内。生产实现必须复用真实 API、状态与命名路由，不复制 Pencil 中的演示金额、项目、订单或账户信息；访客、加载、错误、空数据和已登录状态必须由真实运行时决定。

### 2026-08-04 当前选区扩展

用户重新一次选中 84 个明暗与状态画板，并明确要求所有选中页面按当前设计 1:1 修复，同时修复不合理页面跳转。因此上一段“消息中心、安全中心、KYC、充提币不在本切片内”的范围限制从本轮起失效；完整选区、生产差异、精确导出和跳转合同以 `research/pencil-selected-production-gap-20260804.md` 为准。

本轮新增必须完成：合约、秒合约默认/进行中、消息中心、产品中心、预测市场、完整充币/提币/账单/快捷充值流程、二次验证、找回密码、安全中心、KYC、账户绑定、邀请访客/会员和语言设置的明暗生产映射。已完成的首页、行情、行情详情、现货、资产、我的、订单、登录注册、资讯、闪兑、理财、借贷和新币页面必须作为回归基线保持。

导航必须按业务父子关系和选中壳层修正：根 Dock 使用 replace；消息中心显示 Dock；合约/秒合约不显示 Dock；充币详情深链返回当前资产的网络选择；认证步骤 preserve `redirect` 且不堆积历史；Profile 访客设置进入语言、会员设置进入安全中心；产品新闻和说明进入对应资讯页面，不得用无关页面代替。

当本节与之前的概念性描述在结构、几何或材质上冲突时，以这些选中节点的实时 MCP 结构和截图为准。首页必须区分访客品牌主舞台与登录后资产曲线，不得再给访客展示完整 `--` 资产图；根 Header、搜索、双 CTA、八项产品、行情日报、三行行情及五入口浮动交易 Dock 必须匹配选中画板。行情详情必须匹配 390px 选中结构：64px 品种 Header、42px 两项页面 Rail、112px 行情摘要、48px 周期栏、204px 本地图表、28px 指标图例、48px 盘口/成交标签、272px split 盘口和 64–67px 底部操作区。真实 API、WebSocket、图表引擎、路由、可访问性和安全区行为继续由生产代码负责。

现货交易默认态必须直接匹配 `yzOPc` / `bo8k5`：不叠加 Root Header；页面先显示 64px 交易对 Header（返回、24px 资产标记、交易对及涨跌、收藏、分享），随后使用左右连续工作台，将买卖表单置于左侧、148px 五档卖盘/中间价/五档买盘置于右侧。表单顺序为买卖切换、委托类型、价格、数量、百分比、交易额、止盈止损禁用占位、可用余额和 46px 提交按钮；其后是委托/仓位与资产入口、当前交易对筛选、真实空资产或真实余额状态，以及默认折叠的本地图表入口。五入口 Dock 继续由全局 Shell 提供。图表、最新成交和更多行情工具可以在折叠入口展开后出现，但不得改变该选中默认态的首要几何。

## What I Already Know

- 目标页面为 `mobile/src/views/MarketDetailView.vue`，路由 `/markets/:symbol`。
- 当前页面已有真实 ticker、K 线、订单簿、最新成交、现货/合约导航及分享功能。
- K 线、订单簿和最新成交已经通过详情页专用 WebSocket 实时更新，REST 仅作为首屏兜底。
- 现有 `MobileMarketChart` 使用 npm 本地打包的 TradingView `lightweight-charts`；用户要求专业本地 K 线框架，同时明确 TradingView 也必须保留。
- 选用 `klinecharts@10.0.0` 基础版：官方声明零运行时依赖、Canvas、移动端和 TypeScript 支持；通过 npm/Vite 本地打包，不使用 KLineChart Pro 或任何默认第三方数据源。
- `OrderBookPanel` 当前为纵向卖盘/中间价/买盘结构。
- 后端实际支持的 K 线周期为 `1m/5m/15m/1h/1d`。
- 用户希望参考图中的紧凑 Header、密集行情摘要、边到边图表、指标质感、订单簿/成交切换和固定底部操作区，而不是继续使用松散的大卡片结构。

## Visual Direction

- **Dark theme**: near-black graphite canvas, hairline separators, luminous but restrained positive/negative data, cool metallic controls, dense mono numerals, minimal large-radius cards.
- **Light theme**: bright cool-white instrument surface, graphite text, subtle satin/metal gradients and thin shadows; it must not become a gray, faded translation of the dark theme.
- Use code-owned layered gradients, inset highlights, grid texture and restrained depth rather than external decorative imagery.
- Retain HIPPO green/coral semantic colors and existing skeuomorphic header-control language; do not copy the reference brand, magenta palette, labels, leverage badge, unsupported strategies, or promotional content.
- Lucide icons only; no emoji.

## Requirements

### Global Visual System

1. 将全局材质收敛为 Canvas、Instrument plate、Primary action 三层；移除同屏互相竞争的网格、轨道、阴影和卡片描边。
2. 统一 Root Header、二级 Page Header、44px 拟物图标按钮、输入框、选择器、按钮、弹窗、空态、加载态和错误态。
3. 根导航按当前选中首页画板实现五入口浮动 Dock：`首页 / 行情 / 交易 / 资产 / 我的`，中央交易入口使用 56px 薄荷圆形动作；现货、合约和秒合约仍保持独立页面与真实路由，通过首页产品入口、行情详情操作区和产品中心继续可达，不把三种交易业务合并成一个页面。
4. 明亮模式使用清透冷白、近黑石墨、薄荷绿和克制珊瑚；暗色模式使用近黑石墨、冷白和语义色。禁止大面积褪色灰蓝和低对比文本。
5. 每页只保留一个视觉主角；数字才使用等宽排版，小写标签、标题、说明和按钮形成稳定字号阶梯。
6. 动效只服务首屏主角、路由方向和状态反馈；数据表单不永久漂浮，`prefers-reduced-motion` 必须关闭位移与物理回弹。

### Core Page Hierarchy

1. 首页减少资产图、快捷工具、双 CTA、公告和底栏的同时竞争；在访客/登录状态都建立一个明确的主舞台，并压低公告和低频工具权重。
2. 行情列表压缩 Editorial Hero，合并搜索与分类控制层，强化连续行情列表和价格/涨跌可扫描性。
3. 现货行情详情继续满足下方既有专业终端结构与本地双 K 线引擎合同。
4. 现货、合约和秒合约的 Header、周期栏、工具按钮、输入与买卖动作使用同一交易工作台语言，但保持三个独立栏目和既有业务行为。
5. 资产页在真实数据、空数据和访客状态下都使用诚实的主舞台；没有数据时不展示误导性完整分布。
6. 我的页面重排访客/登录主卡、登录/注册权重、语言与客服入口，消除无意义的大面积空白。
7. 消息中心、借贷、安全中心及重点二级页面迁移到一致的 Page Header、状态板、分组、表单和空态系统；空数据不展示充满 `--` 的伪产品卡。

### Pencil UI/UX Blueprint

1. 在 `mobile/pencil/hippo-mobile-uiux.pen` 建立完整设计源文件，并保留可重复执行的 Pencil `execute` 生成脚本、页面清单和导出预览。
2. 设计文件至少覆盖：设计系统、首页明暗模式、行情、行情详情、现货、合约、秒合约、资产、我的、消息、新闻、订单、产品中心、闪兑、理财、借贷、新币、预测、充币、提币、账单、快捷充值、登录注册、二次验证、找回密码、KYC、安全、账号绑定、邀请和语言设置。
3. 每个页面使用真实路由和真实业务字段命名，明确默认、加载、空、失败、访客、登录后、聚焦、禁用和确认反馈，不新增后端不存在的功能。
4. 现货交易必须作为重点高保真页面重新设计：交易对与实时价格为首要信息，K 线与周期工具、split 订单簿、最新成交、买卖切换、限价/市价、价格/数量输入、百分比、余额、委托和提交动作形成连续工作台；不得与合约或秒合约合并。
5. Pencil 画布中的图标统一使用 Lucide，界面文案不使用 emoji；主操作和表单触控目标以 44–52px 为基准。
6. Pencil 导出的关键预览至少包含首页、行情、现货、资产、我的、产品、钱包流程和账户安全流程，并提供整份多页 PDF 便于审阅。

### Page Structure

1. Match the selected compact instrument Header exactly:
   - transparent 44px safe back button at the leading edge;
   - 24px asset mark, `BASE/QUOTE` and pair selector chevron;
   - 44px favorite control and 44px share action at the trailing edge;
   - no spot badge or price microline inside the Header.
2. Match the selected 42px market rail with only two truthful page sections:
   - active `行情` section with a 22x2 mint indicator;
   - `币种概述` section backed by real ticker/base-asset information rather than a dead control.
3. Rebuild the price summary as a dense two-column instrument block:
   - latest price, approximate quote value, signed 24h change;
   - 24h high, low, base volume and quote turnover when available;
   - no fabricated rank, mark price, turnover or market tags.
4. Build a seamless chart workstation:
   - a 48px interval rail using the shared supported interval source and the compact local engine control at the trailing edge;
   - the inline chart itself is 204px high at 390px and owns the 32px expand action;
   - put the compact MA5/MA10/MA20/volume legend in a separate 28px row below the chart;
   - real MA5/MA10/MA20 values and chart overlays computed from loaded K-lines;
   - real volume summary;
   - accessible expand/collapse control that creates an immersive chart surface without using the browser Fullscreen API;
   - preserve pinch/drag behavior, loading/empty/error states and live K-line updates.
   - default to locally bundled `klinecharts@10.0.0` base package and provide a compact persisted switch to locally bundled TradingView `lightweight-charts@5.2.0`;
   - never use TradingView online Widget/Charting Library, KLineChart Pro/default datafeed, remote chart assets or third-party market data;
   - render real candles and MA5/MA10/MA20 plus real volume in both engines; disable the optional Lightweight Charts attribution logo so local chart mode creates no external anchor or service integration.
5. Consolidate market microstructure below the chart into a switchable panel:
   - `订单簿` and `最新成交` tabs with proper `aria-pressed`/panel semantics;
   - order book uses a dense split buy/sell layout similar to the reference while retaining real levels and depth bars;
   - latest trades remains live, deduplicated and uses real time/price/quantity.
6. Rebuild the fixed bottom action deck:
   - one 242x52 dominant real spot-trade pill on a 390px reference canvas;
   - real futures and orders routes as two compact 40px secondary actions on its left;
   - safe-area aware and never overlaps the final market rows.

### Behavior Preservation

- Do not change HTTP endpoints, WebSocket payloads, session/race logic, route names or trading request behavior.
- Both chart frameworks must be imported from npm packages and bundled by Vite; runtime source must contain no chart CDN, remote script, iframe, TradingView online Widget/Charting Library or KLineChart Pro/default datafeed.
- Switching engines must not reconnect, replace or clear the existing market REST/WebSocket detail session; only one rendering engine may be mounted at a time.
- Interval changes must continue replacing the live detail session without clearing visible order-book/trade state.
- Header/nav actions must be real: scroll to an existing section or navigate to an existing typed route.
- No fake favorite, alert, grid strategy, leverage selector, indicator selector, unsupported market statistic or demo data.
- Keep all copy in zh-CN and English locale resources.
- Preserve 44px minimum touch targets, keyboard focus, reduced-motion behavior and sticky-layer ordering.

### Responsive and Texture Contracts

- At 320x720, 360x800, 390x844 and 448x900 CSS pixels: no document horizontal overflow, clipped labels or bottom-action overlap.
- Portrait is the primary composition; landscape must remain usable and avoid Header content collision.
- Dark and light themes must both retain high contrast and visible surface separation.
- Use no page-level horizontal scroll, no nested vertical scroll trap, and no fixed-height content that cuts off chart labels.
- Full-chart mode must lock background scroll, respect top/bottom safe areas, expose a clear close control and restore the previous scroll position.

## Acceptance Criteria

- [ ] `mobile/pencil/hippo-mobile-uiux.pen` 可由 Pencil CLI 打开、读取、截图和导出，不是空白或扁平截图文件。
- [ ] Pencil 文件包含完整页面清单和共享 token/组件样板；首页样式可以在行情、交易、资产、产品、钱包和账户流程中连续识别。
- [ ] 现货交易拥有独立高保真画板，包含实时行情摘要、双本地图表引擎入口、订单簿/最新成交、买卖表单、真实余额语义和委托入口。
- [ ] Pencil 画板无未完成 placeholder、节点越界、内容裁切、低对比文字或小于设计合同的主要交互控件。
- [ ] Pencil CLI `get_app_state`、结构快照/节点检查、关键画板截图和多页导出均成功。

- [ ] 首页、行情、交易、资产、我的及消息/借贷/安全中心在 390px 首屏各有唯一清晰的视觉主角，且共享同一色彩、材质、Header 和控件语法。
- [ ] 选中画板的五入口浮动 Dock 与中央交易动作完成生产映射；现货、合约和秒合约页面仍各自独立可达，安全区、44px 触控和当前项语义完整。
- [ ] 明亮模式清透、高对比且不暗淡；暗色模式近黑、克制且没有大面积割裂白卡。
- [ ] 访客、空数据、加载、错误和登录后状态都不会出现误导性 `--` 产品卡、完整空图例或禁用态风格的主按钮。
- [ ] 输入、选择器、按钮、弹窗和空态在重点页面使用共享 token/组件，聚焦边框没有双重或局部矩形残留。
- [ ] 首屏与路由动效不会造成上下拉扯、上一页残影、固定 Header 被遮挡或减少动效模式下的位移。

- [ ] Page hierarchy visibly matches the reference pattern: compact instrument Header → slim nav → dense quote summary → chart workstation → market-data tabs → fixed action deck.
- [ ] Dark mode has near-black graphite, hairline and luminous market-data texture; light mode is bright, crisp and intentionally designed.
- [ ] MA5/MA10/MA20 overlays and legend values are derived from actual K-line points and update with live candles.
- [ ] `klinecharts@10.0.0` and TradingView `lightweight-charts@5.2.0` are both bundled locally, with no remote chart code or data requests.
- [ ] Default KLineChart and selectable TradingView modes both use HIPPO-provided OHLCV only, show MA5/MA10/MA20 and volume, and preserve the current viewport during forming-candle updates.
- [ ] The engine selection is keyboard/touch accessible, locally persisted, restores after restart and does not reconnect market data.
- [ ] Chart expansion is fully operable, accessible, safe-area aware and reversible.
- [ ] Order-book and latest-trades tabs switch real live content without reconnecting or losing data.
- [ ] Split order book renders real bids/asks with valid depth bars and remains readable at 320px.
- [ ] Spot trade, futures and orders actions navigate to existing routes with the active pair context.
- [ ] Existing K-line/depth/trade WebSocket and REST race tests remain green.
- [ ] Focused layout/source tests, type-check, full mobile tests, PWA build and Tauri build pass.
- [ ] Latest Android APK is installed and the redesigned page is inspected on the connected phone in light/dark themes.

## Definition of Done

- Visual structure, responsive states, interactions and real-data semantics are covered by focused tests.
- `npm --prefix mobile run type-check`, `npm --prefix mobile test`, `npm --prefix mobile run build:pwa` and `npm --prefix mobile run build:tauri` pass.
- Android Debug APK builds, installs, starts and is visually verified on the connected phone.
- Dependency/source and rendered-DOM tests prove the chart has no CDN, iframe, remote script, external anchor, online Widget/Charting Library or Pro/default datafeed; TradingView mode remains the locally bundled Lightweight Charts renderer.
- Mobile UI/backend specifications and `docs/superpowers/PROGRESS.md` are updated.
- Work is committed, Trellis task archived and session journal recorded.

## Technical Approach

1. Keep all data/session code in `MarketDetailView.vue` intact and refactor only view state needed for section navigation, market-data tab and chart expansion.
2. Refactor `MobileMarketChart.vue` into a local dual-engine wrapper: add `klinecharts@10.0.0` as the default renderer and retain the existing `lightweight-charts@5.2.0` renderer as TradingView mode; share normalized points/theme state, configure real MA5/MA10/MA20 and volume, stable live updates, resize/dispose lifecycle, local preference persistence, and avoid resetting the viewport on every forming-candle tick.
3. Extend `OrderBookPanel.vue` with an explicit split layout variant instead of duplicating order-book normalization or changing the default layout used by other pages.
4. Add local, theme-aware CSS texture to the market-detail surface and component variants; do not modify global tokens unless a reusable missing semantic token is proven necessary.
5. Update locale keys and focused tests for DOM hierarchy, routes, tabs, chart series, responsive geometry and interaction contracts.

## Decision (ADR-lite)

**Context**: The reference includes many exchange-specific functions not currently backed by HIPPO APIs. Copying its labels and controls would create misleading or dead UI.

**Decision**: Reproduce the hierarchy, density and material quality, but map every visible control to HIPPO's real content: chart, order book, latest trades, spot trade, futures and orders. Technical depth comes from real MA overlays, live depth bars and live trade/K-line state rather than fabricated widgets.

**Consequences**: The result will feel like the reference while remaining truthful and maintainable. Unsupported strategy, alert, leverage and ranking surfaces are intentionally absent.

## Out of Scope

- No backend, database, WebSocket protocol or trading engine changes.
- No redesign of PC client or admin client.
- No backend,数据库或协议变更；交易请求、订单语义、认证和真实路由保持不变。
- No KDJ/BOLL/SAR engine, depth chart, alert service, grid strategy or favorite synchronization.
- No copying of reference branding, icons, promotional labels or proprietary assets.
- No KLineChart Pro, TradingView online Widget/Charting Library, CDN, iframe, remote chart script or third-party market datafeed.

## Technical Notes

- Primary files include `mobile/src/styles/{base,prototype-parity}.css`, `mobile/src/components/{RootHeader,AppBottomNav}.vue`, `mobile/src/views/{HomeView,MarketsView,TradeView,AssetsView,ProfileView,MessageCenterView,LoanView,SecurityView}.vue`, plus existing `MarketDetailView.vue` and chart/order-book components.
- Likely test files: `mobile/tests/android-ui-trading-prototype-v16.test.ts`, `mobile/tests/ui-prototype-alignment-trading.test.ts`, `mobile/tests/market-news-support-views.test.ts`, `mobile/tests/market-detail-stream.test.ts`, plus a focused new/updated layout contract test.
- Related specs: `.trellis/spec/mobile/index.md`, `.trellis/spec/mobile/pwa-and-shell.md`, `.trellis/spec/mobile/navigation-and-localization.md`, `.trellis/spec/mobile/backend-integration.md`.
- Framework research: `research/local-kline-framework.md`.
- Award/reference audit: `research/award-mobile-ui-audit.md` and `research/screenshots/award-audit/`.
