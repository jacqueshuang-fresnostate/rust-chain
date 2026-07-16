# PC竞猜页面参考 Polymarket 分类详情优化

## Goal

将 PC 端 `/prediction` 竞猜页面升级为更接近 Polymarket 中文站的信息架构：列表页支持分类/话题浏览，市场卡片可点击进入详情页，详情页展示市场信息并复用现有虚拟资产报价与下单能力。

## What I Already Know

- 用户明确要求竞猜页面参考 `https://polymarket.com/zh`，可以有分类，并且可以点进去查看详情。
- 当前 PC 已有 `pc/src/views/Prediction.vue`，包含市场列表、分类筛选、搜索、排序、右侧下单面板。
- 后端已有 `GET /api/v1/prediction/markets` 和 `GET /api/v1/prediction/markets/:id`，无需新增详情 API。
- 现有 `pc/src/utils/predictionLocale.ts` 已支持动态市场文本中文 fallback，本任务应继续复用。
- 现有 `createPredictionQuote` / `createPredictionOrder` 已完成后端签发报价与本地钱包结算，页面不能绕过 quote 直接下单。

## Requirements

- `/prediction` 列表页采用 Polymarket 风格的信息架构：
  - 顶部展示“浏览/话题”式分类入口。
  - 支持全部、热门、成交量、即将结束等发现维度。
  - 市场卡片展示图片、分类、标题、描述、YES/NO 概率、成交量、流动性、截止时间。
- 市场卡片支持点击进入详情页。
- 新增或扩展路由 `/prediction/:id`，用于展示市场详情。
- 详情页展示：
  - 返回列表入口。
  - 市场标题、分类、状态、图片、描述。
  - 成交量、流动性、截止时间、最后同步时间等关键指标。
  - YES/NO 概率区域。
  - 下单面板：选择方向、下注资产、下注金额、获取报价、确认下注。
- 列表页和详情页都必须支持动态文本本地化。
- 无登录态下尝试报价/下注应继续跳转登录页。
- 详情 API 加载失败或 ID 不存在时，应显示可恢复的空状态，并提供返回列表入口。

## Acceptance Criteria

- [ ] `/prediction` 可以看到分类/话题导航和 Polymarket 风格市场卡片。
- [ ] 点击市场卡片会导航到 `/prediction/:id`。
- [ ] `/prediction/:id` 会调用详情 API 并展示市场详情。
- [ ] 详情页可以选择 YES/NO、选择下注资产、输入金额、获取报价并提交订单。
- [ ] PC 中文环境下，分类、标题、描述和 YES/NO 文案继续使用本地化 fallback。
- [ ] 现有竞猜下单 API 契约不变。
- [ ] PC type-check 通过，新增/更新的静态测试通过。

## Definition of Done

- 更新 PC 路由、页面和必要 i18n 文案。
- 添加或更新 focused PC 测试，覆盖详情路由和列表可进入详情。
- 运行最贴近改动的测试与 `npm --prefix pc run type-check`。
- 更新 `docs/superpowers/PROGRESS.md`。

## Technical Approach

- 保持后端 prediction contract 不变，仅复用 `fetchPredictionMarket(id)`。
- 将 `Prediction.vue` 改成同时处理列表和详情模式：
  - `route.params.id` 存在时进入详情模式。
  - 列表模式加载市场列表和配置。
  - 详情模式优先使用列表缓存中的市场，随后调用详情 API 刷新详情。
- 复用现有本地化 helper 和 quote/order 方法，避免重复业务规则。
- 使用 Tailwind 与现有 PC 端深色 SaaS/交易风格，不新增第三方 UI 依赖。

## Decision (ADR-lite)

**Context**: 用户要的是 Polymarket 风格分类与详情体验，而不是新增一套竞猜业务。

**Decision**: 前端单页升级并新增动态详情路由，后端和数据库契约保持不变。

**Consequences**: 交付范围小、风险低；未来如果要支持多 outcome、评论、实时 order book，可以在详情页内继续扩展。

## Out of Scope

- 不新增数据库字段、后端路由或 Polymarket 同步策略。
- 不实现 Polymarket 原站的评论区、活动流、订单簿深度图、多 outcome CLOB 交易。
- 不改变本地 quote/order/settlement 的资金结算规则。

## Research References

- [`research/polymarket-page-reference.md`](research/polymarket-page-reference.md) — Polymarket 中文列表页/详情页结构参考与本项目映射。

## Technical Notes

- Relevant files:
  - `pc/src/views/Prediction.vue`
  - `pc/src/router/index.ts`
  - `pc/src/api/prediction.ts`
  - `pc/src/utils/predictionLocale.ts`
  - `pc/src/i18n/index.ts`
- Relevant spec:
  - `.trellis/spec/backend/prediction-markets.md`
