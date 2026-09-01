# Admin 业务订单号加载失败根因

## 现象

- 秒合约订单接口：`/admin/api/v1/seconds-contracts/orders` 第 1 行缺少必填字段 `order_no`。
- 理财申购接口：`/admin/api/v1/earn/subscriptions` 第 1 行缺少必填字段 `order_no`。

## 数据流

1. `AdminResourcePage` 从表格 `columns` 自动生成 `rowContract`。
2. 当前 `requiredFields` 等于所有列的 `key`。
3. `listAdminResource` 对每一行使用 `hasOwnProperty` 校验这些字段。
4. `orderNoColumn()` 的列键是 `order_no`，因此该展示列被误当作接口 DTO 必填字段。
5. 秒合约与理财后端响应本来不包含 `order_no`，严格校验先抛错，导致 `formatBusinessOrderNo()` 的既有回退渲染永远没有机会执行。

## 后端核对

- `SecondsContractOrderResponse` 返回订单 ID、用户、产品/交易对、金额、状态、结果和时间等字段，没有 `order_no`；对应 SQL 也未查询该字段。
- `EarnSubscriptionResponse` 返回申购 ID、用户、产品/资产、金额、收益率、状态、幂等键和时间等字段，没有 `order_no`；对应 SQL 也未查询该字段。
- 预测业务存在真实 `order_no`，说明不同业务 DTO 并不共享统一持久化订单号字段。

## 影响面

`orderNoColumn()` 当前由以下业务配置复用：

- 借贷订单 `LN`
- 预测订单 `PM`
- 现货订单 `SP`
- 新币申购 `NC`
- 新币购买 `NP`
- 闪兑订单 `CV`
- 秒合约订单 `SC`
- 理财申购 `EA`

因此 endpoint 白名单或只修复秒合约/理财会留下相同结构性问题。

## 修复结论

- 保留 `adminResources.ts` 的严格行校验；这是发现真实 DTO 漂移的防线。
- 在列模型上显式标记 `derived`，默认来源仍为 `api`。
- 从列定义生成 `requiredFields` 时排除派生列。
- `orderNoColumn()` 统一标记派生来源；其渲染继续优先读取后端 `order_no`/`orderNo`，缺失时使用时间和 ID 合成。
- 测试同时证明：派生列被排除、普通列仍必填、金额列仍进行 Decimal 校验、所有订单号配置都继承派生语义。

