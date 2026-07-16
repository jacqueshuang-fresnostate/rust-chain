# PC端现货路由改为spot

## Goal

PC 端现货交易页面的公开 URL 应该使用 `/spot`，不再使用 `/trade`。这样页面路径和业务含义一致，也避免“Trade”泛指交易入口时和现货页面混淆。

## What I Already Know

- 当前 PC 路由在 `pc/src/router/index.ts` 中定义为 `trade/:symbol?`，路由 name 为 `Trade`，组件为 `Trade.vue`。
- 首页 CTA 仍直接跳转 `/trade`。
- Header、Market、Home 和 Trade 页面内部主要使用 route name `Trade` 跳转交易对，改路由 path 后应自动生成 `/spot/:symbol?`。
- `pc/AGENT.md` 的 URL Persistence 示例仍写 `/trade/BTC_USDT`，需要同步为 `/spot/BTC_USDT`，避免后续实现回退。

## Requirements

- 将 PC 端现货交易路由 path 从 `trade/:symbol?` 改为 `spot/:symbol?`。
- 将首页直接跳转 `/trade` 的入口改为 `/spot`。
- 保持现有现货页面组件和 route name 稳定，避免扩大重命名范围。
- 交易对跳转继续保留 symbol 参数，例如 `/spot/BTC_USDT`。
- 更新 PC 文档中现货 URL 示例为 `/spot/BTC_USDT`。
- 增加轻量测试，防止现货路由和首页入口回退到 `/trade`。

## Acceptance Criteria

- [x] `pc/src/router/index.ts` 中现货页面 path 为 `spot/:symbol?`。
- [x] 首页 CTA 不再跳转 `/trade`。
- [x] Header、Market、Home、Trade 页面按 route name 跳转交易对后会生成 `/spot/:symbol?`。
- [x] `pc/AGENT.md` 中现货 URL 示例不再使用 `/trade/BTC_USDT`。
- [x] PC 端测试/type-check 覆盖或验证本次路由变更。

## Definition of Done

- 只修改 PC 端现货路由、入口和相关文档/测试。
- 执行最贴近改动的 PC 端测试和类型检查。
- 更新 `docs/superpowers/PROGRESS.md`。

## Technical Approach

使用最小改动：保留 `Trade.vue` 和 route name `Trade`，只调整 URL path 为 `spot/:symbol?`。现有通过 `{ name: 'Trade', params: { symbol } }` 的跳转会随路由 path 自动生成新 URL；只有字符串路径 `/trade` 需要直接替换为 `/spot`。

## Decision (ADR-lite)

**Context**: 用户明确要求 PC 端现货路径应该是 `spot` 而不是 `trade`。  
**Decision**: URL 层改为 `/spot`，不做组件和 route name 大规模重命名。  
**Consequences**: 改动范围小、风险低；代码内部仍存在 `Trade.vue` 命名作为组件历史名称，但用户可见路径符合业务语义。

## Out of Scope

- 不重构现货交易页面组件名称。
- 不调整 `launchpad/trade/:symbol?`，它不是现货交易路径。
- 不改后端 `/spot/*` API。

## Technical Notes

- 已搜索 `/trade`、`Trade`、`spot` 相关引用。
- 相关文件：`pc/src/router/index.ts`、`pc/src/views/Home.vue`、`pc/AGENT.md`、`pc/tests/*`。
- PC 端测试环境使用 Node 内置 test runner 和 `--experimental-strip-types` 风格；新增路由契约测试优先避免引入 Vue SFC 测试环境。
