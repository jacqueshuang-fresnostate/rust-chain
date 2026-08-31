# Mobile Trade / Seconds 资金精度终检（2026-08-31）

## 结论

Trade/Seconds 执行路径及其上游 Seconds、盘口、ticker、wallet、trading DTO 已全部改为
字符串支撑的 `DecimalText` 权威值；后端 JSON number 只允许进入明确的兼容展示字段，
不得再成为下单、限额、余额、收益或风控决策来源。FMD-P1-03 的端到端阻塞已关闭。

## 已修复

- `decimal.ts` 的 `maxScale` 改为按去除尾随零后的有效小数位判断。
- Trade 市价、数量/金额互推、确认快照及 BBO 建议只消费十进制文本；缺少精确盘口时
  只回退精确 ticker，不再字符串化盘口 `number`。
- Trade 钱包、保证金上下限和百分比快捷量在进入确认/提交前要求精确字段；缺失时
  fail-closed。
- Seconds 将图表/旧 ticker 数字限制在展示函数；确认参考价只读取 `lastPriceText`。
- Seconds stake/range/wallet/payout/PnL 执行派生只接受精确文本；旧订单数字字段不再进入
  盈亏或收益率计算。

## Phase A 前发现的跨层阻塞（现已全部关闭）

1. `mobile/src/api/seconds.ts` 的 `SecondsCycle` 没有 `payoutRateText`，且
   `payout_rate` 先经 `asNumber`；因此当前 Seconds 确认会安全拒绝下单，直到产品 DTO
   提供并严格映射精确赔率文本。
2. `mobile/src/core/secondsOrder.ts` 的 stake/payout/entry/settlement 仍全是 `number`，没有
   对应 DecimalText；刷新后的历史和结算 PnL 只能显示不可用，不能精确重建。
3. `mobile/src/core/types.ts` 的 `OrderBookLevel` 及 REST/WS depth adapter 只保留数字
   price/quantity；Trade 限价建议目前只能使用精确 ticker fallback，无法消费真实精确
   ask/bid。
4. `mobile/src/core/marketMapper.ts`、`mobile/src/api/marketSocketProtocol.ts`、
   `mobile/src/api/{wallet,trading,seconds}.ts` 仍允许把后端 JSON number 转成看似精确的
   DecimalText。这些字段可进入余额、限额和参考价决策，必须改用严格 string DTO
   映射，数字字段仅保留独立展示属性。

运行时探针把 `9007199254740993.000000000000000001` 作为 JSON number 输入后，market
ticker、Seconds order 和 margin limits 均得到 `9007199254740994`，证明兼容 adapter
不能作为执行值来源。

## 当时规划的最小切片（已完成）

- 在上述 API/mapper/type 文件增加严格 DecimalText 字段，并保留独立 numeric display
  字段；拒绝 malformed/non-string 金融 DTO。
- 为产品赔率、订单 stake/payout/entry/settlement、盘口 price/quantity 增加超 `2^53` 与
  `1e-18` 行为测试，再接回本次已准备的严格 view/core 路径。

## Phase A DTO 收口结果（2026-08-31 23:18）

本轮按收窄后的 Phase A 所有权完成了上述阻塞 1–3：

- `SecondsCycle` 的 `payoutRateText`、`minStakeText`、`maxStakeText` 现在是必备的权威字段；
  payout/min 缺失或为 JSON number 直接抛出 `SecondsContractError`，无上限明确表示为
  `maxStakeText: null`，不再制造 `"0"`。
- `SecondsOrder` 的 stake/payout/entry/settlement 均保留必备的 DecimalText 属性；前两项
  缺失或非字符串抛出 `SecondsOrderContractError`，后两项仅在后端明确缺失/null 时保持
  `null`，存在但畸形时同样 fail-closed。历史净盈利与亏损先以 DecimalText 精确乘法/取负
  生成 `amountText`，兼容 number 只在最终展示属性 `amount` 产生。
- REST/WS depth 的每一档必须提供严格正数十进制字符串，映射结果同时携带
  `priceText/quantityText`；REST 合同错误抛出 `MarketDepthContractError`，WS 整帧返回
  `null`，numeric `price/quantity` 仅用于排序和旧展示。
- 行为测试证明 `9007199254740993.000000000000000001` 与
  `0.000000000000000001` 在周期、订单、盈亏与盘口 DTO 中逐字保留，并覆盖 JSON-number
  与指数形式拒绝。聚焦测试 41/41、源码类型检查和测试类型检查均通过，无调用方编译阻塞。

按本轮明确边界，原阻塞 4 中的 `marketMapper` ticker 以及 wallet/trading DTO 严格化未在
Phase A 修改，仍作为后续独立切片；`marketSocketProtocol` 本轮只关闭 depth 权威字段。

## Phase B DTO 与最终门禁收口（2026-09-01）

- `marketMapper` 的 `lastPriceText` 只接受严格正数十进制字符串；JSON number 继续作为
  兼容图形坐标，但不会伪造权威价格文本。
- `wallet.ts` 为余额、冻结、锁定、充值/提现费用、阶梯费率、提现记录、快捷充值和划转
  增加严格 DecimalText 字段；缺失或非字符串必填资金字段抛出
  `WalletFinancialContractError`，不制造 `"0"`。
- `trading.ts` 为现货订单、杠杆产品、钱包、持仓、全仓账户和风险快照建立严格资金字段；
  现货/杠杆下单公开输入使用 DecimalText，市价参考价缺失时 fail-closed，风险数字仅在
  最终展示适配处转换为 number。
- Trade/Seconds 视图通过 `tradeFinancial.ts`、`secondsFinancial.ts` 消费上述权威字段；
  旧源码正则测试已迁移到新适配器合同，行为门禁仍是主要证据。

最终验证：`npm --prefix mobile run release:gate` 全部通过，Mobile 607/607；PWA 与 Tauri
双构建、制品断言、raw/gzip bundle、source-size 与 critical behavior test-quality 预算均
通过。Ego Browser 在 320/390/448px 对首页、现货、秒合约、订单、资产等关键路由复核，
无横向溢出、破图或重复 ID；延时复核确认每条路由只有一个 `main#main-content`，标题与
announcement 正确。
