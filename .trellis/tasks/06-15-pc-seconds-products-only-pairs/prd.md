# PC 秒合约交易对只使用秒合约产品

## Goal

修复 PC 端秒合约页面交易对列表的数据源。当前页面加载交易对时通过 `fetchSecondSnapshot = fetchMarketSnapshot` 复用了全市场 `/api/v1/markets`，导致左侧交易对列表会显示普通现货市场，而不是后台配置的秒合约产品交易对。

## What I Already Know

- 用户明确指出：PC 秒合约交易对应只显示秒合约交易对，当前请求 `api/v1/markets` 不正确。
- `pc/src/views/SecondOptions.vue` 通过 `store.loadTickers()` 加载左侧交易对列表。
- `pc/src/stores/second.ts` 的 `loadTickers()` 调用 `fetchSecondSnapshot()`。
- `pc/src/api/second.ts` 当前将 `fetchSecondSnapshot` 直接别名为 `fetchMarketSnapshot`，从而触发 `/api/v1/markets`。
- 用户端秒合约产品接口是 `/api/v1/seconds-contracts/products`，后端用户路由只返回 active 产品。
- 图表、K 线、盘口和最新成交仍可按当前 symbol 使用市场行情接口；本任务只修交易对列表的数据源。

## Requirements

- 秒合约页面左侧交易对列表必须来自 `/seconds-contracts/products`。
- 同一个交易对存在多个周期时，左侧交易对只显示一次。
- 秒合约产品中的 `logo_url` 应继续传递到 PC ticker 的 `icon`，用于显示交易对 Logo。
- 可以按秒合约交易对 symbol 拉取对应 ticker 价格补充，但不得先请求 `/markets` 全量列表。
- 如果某个秒合约交易对 ticker 拉取失败，仍然保留该交易对，价格字段使用安全默认值。

## Acceptance Criteria

- [ ] `pc/src/api/second.ts` 不再把 `fetchSecondSnapshot` 指向 `fetchMarketSnapshot`。
- [ ] PC 秒合约左侧交易对列表只会由秒合约产品生成。
- [ ] adapter 测试覆盖：多周期去重、非秒合约产品不会混入、logo 保留、ticker 补充价格。
- [ ] PC 类型检查通过。

## Out Of Scope

- 不修改后端秒合约产品接口。
- 不修改秒合约下单、结算或订单列表逻辑。
- 不修改图表/K 线/深度/最新成交的行情接口。

## Technical Notes

- 相关文件：`pc/src/api/second.ts`, `pc/src/api/backendAdapters.ts`, `pc/src/stores/second.ts`, `pc/src/views/SecondOptions.vue`, `pc/tests/backendAdapters.test.ts`。
- 本次应优先复用 `BackendSecondsProductsResponse`、`BackendMarketTicker`、`PcMarketTicker`、`displaySymbolFromCompact`、ticker 价格解析 helper。
