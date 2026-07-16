# 秒合约交易对数据源检查

## 发现

- `SecondOptions.vue` 的左侧交易对列表来自 `store.tickers`。
- `store.loadTickers()` 调用 `fetchSecondSnapshot()`。
- `fetchSecondSnapshot` 当前是 `fetchMarketSnapshot` 的别名，后者会请求 `/api/v1/markets` 并映射所有现货/市场交易对。
- 用户端 `/api/v1/seconds-contracts/products` 后端路由为 `list_active_products`，适合作为 PC 秒合约可交易对来源。

## 结论

秒合约交易对列表应由 seconds products 生成，按 symbol 去重，并可通过 `/markets/{symbol}/ticker` 仅补充已配置秒合约交易对的行情字段。
