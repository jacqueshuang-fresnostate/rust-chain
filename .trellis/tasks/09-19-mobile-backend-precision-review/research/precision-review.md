# 手机端与后端金额精度复审

日期：2026-09-19。范围为当前工作区代码，不是生产账务审计。

## 结论

有问题，但不是所有链路都用浮点数算钱。手机端存在已复现的响应金额失真和小额显示为零；后端主要使用 BigDecimal，但杠杆输入、现货乘积写入和计息截断仍有缺口。不能把数据库 DECIMAL 或 Rust BigDecimal 等同于端到端无损。

## P1：杠杆开仓缺少金额精度校验

- `src/modules/margin/presentation.rs:79`：请求保证金为 BigDecimal，没有 DTO 层小数位限制。
- `src/modules/margin/application/open_position.rs:93`：只检查保证金为正。
- `src/modules/margin/application/open_position.rs:363`：产品校验只检查区间、杠杆档位和小时利率，没有检查保证金的资产精度或 DECIMAL(38,18) 容量。
- `src/modules/margin/application/open_position.rs:180`：名义金额乘法结果未先检查存储容量；原保证金继续传给插入和扣款。
- `src/modules/margin/infrastructure/settlement.rs:63`：余额减原始金额后直接绑定 SQL；平台分录入口只检查零和，没有资产量化兜底（`src/modules/wallet/infrastructure/shared.rs:192`）。

实际提取当前校验函数运行，`1.0000000000000000001`（19 位小数）在产品区间 1..100、杠杆 2、利率 0 时通过。结果不能无损写入 18 位小数列；即便只有 9 位，也可能超出 8 位资产合同。存在落库舍入/拒绝、请求与持久化快照不一致的风险，不能依赖客户端过滤。没有实库验证最终结果，不宣称已经产生线上资金损失。

建议：源保证金超精度直接拒绝；同时检查乘积容量。派生损益、利息、返还和分录按明确的同币种规则统一处理，不能只截断余额。

## P1：现货价格和数量分别合法，不代表成交额合法

- `src/modules/spot/domain.rs:223`：订单构造检查交易对价格、数量各自精度。
- `src/modules/spot/service.rs:890`：预留直接使用价格乘数量。
- `src/modules/spot/infrastructure/trade_settlement.rs:462`：资产查询只返回两个 ID，不带资产精度。
- `src/modules/spot/application/order_creation.rs:325`：原预留额传入订单写入和钱包冻结。
- `src/modules/spot/infrastructure/wallet_accounts.rs:78`：冻结时原样计算、绑定两个钱包余额。
- `src/modules/spot/application/settlement.rs:96`：人工成交额也是直接乘积；自动执行同样如此（`src/modules/spot/application/triggering.rs:406`）。
- `src/modules/admin/service/market.rs:701`：交易对配置只限制精度非负，没有最大值或与资产精度的约束。
- `migrations/0003_assets_wallet_ledger_locks.sql:15`：钱包余额列为 DECIMAL(38,18)。

执行实际交易对配置校验、订单构造及预留函数：

```text
8+8 位：1.00000001 * 1.00000001 = 1.0000000200000001（16 位）
10+10 位：1.0000000001 * 1.0000000001 = 1.00000000020000000001（20 位）
```

前者不能直接用于要求 8 位的报价资产；后者已经超过数据库小数位容量。缺少统一量化会把交易金额决定权留给存储边界；部分成交、撤单返还可能进一步暴露尾差。此处确认的是源代码路径和超精度结果，不是已复现实库不守恒。

建议：冻结、成交、部分成交累计、撤单释放采用一致的量化与余量规则，绑定真实资产精度；增加价格/数量配置上限。不要分别舍入钱包两桶来“修复”。

## P2：手机闪兑响应失去原始金额，手续费可显示为零

- `mobile/src/api/swap.ts:58`：报价四个金额/汇率字段转为 Number，历史同样处理（第 85 行）。
- `mobile/src/core/format.ts:32`：formatAmount 默认只展示 4 位小数，并经 asNumber。
- `mobile/src/views/SwapView.vue:403`：确认层付款、到账、手续费都使用上述数值格式化。

执行真实适配器，HTTP 仅内存模拟：

```text
请求 from_amount：9007199254740993.000000000000000001（完整保留）
响应适配后金额：9007199254740994
手续费：0.00000001；确认层所用格式化结果：0
确认请求：{ quote_id: "precision-review" }
```

这是金额原文丢失与不当显示策略两个问题。不能称为本轮证明了闪兑实际扣款错误：Mobile 请求使用 DecimalText，最大额使用 availableText，确认只发原报价 ID（`mobile/src/views/SwapView.vue:131`、`mobile/src/api/swap.ts:49`、`mobile/src/api/swap.ts:67`），并不把失真的响应数值重新传给后端扣款。

建议：报价、订单全程保留 DecimalText；确认层保留精确原文，小额非零费用不得显示成零。

## P2：手机理财持仓和贷款还款账单仍丢精度

- `mobile/src/api/earn.ts:105`：持仓本金只有 Number，无精确文本。
- `mobile/src/api/loan.ts:83`：贷款本金、抵押额、利息、应还额转 Number。
- `mobile/src/views/EarnView.vue:330`：持仓展示采用浮点格式化。
- `mobile/src/views/LoanView.vue:604`：还款确认显示已转换的 repaymentAmount。

同一大额样本在真实适配器中同样变为 `9007199254740994`。损失不只是 UI 少展示几位：原值在客户端模型内已丢失，无法再用于精确账单核对。

申购/借款请求与抵押请求仍使用 normalizeDecimalText；赎回/还款只发送记录 ID。本轮未证明客户端账单失真会修改实际还款金额。费率也保留为 Number，属于后续统一模型的检查点，但不能用构造的超规格费率断言合法产品一定会算错预估。

## P2：杠杆计息尾差依赖任务批次

- `src/workers/margin_interest.rs:334`：每批利息在相乘后 with_scale(18)。
- `src/workers/margin_interest.rs:258`：正增量入库后推进计息点，没有保存本批截断余数；零增量分支不推进，不能混为一谈。

实际函数运行：借款 `0.00000000015`、小时利率 `0.00000001`，每小时计提一次，两小时合计 `2e-18`；两小时一次补算得到 `3e-18`。这是已复现的确定性截断尾差，不是 f64 问题。例子要求支持该本金精度的资产，不能据此推断普通币种会出现显著损失。

建议：明确逐小时独立舍入还是累计应计差额；如要求恢复前后金额一致，保留高精度余数或使用累计应计减已计提。实际业务规则待修复任务确定。

## 已检查的保护

- `Cargo.toml:41` 已启用 serde_json arbitrary_precision；检索 src 中 f64/f32，没有发现已检查交易结算以 f64 计算金额，命中主要为时间和日志。
- Mobile DecimalText 的输入、范围比较、交易派生和秒合约快照已有高精度测试，本轮定向测试通过。
- `src/modules/convert/application.rs:89` 获取源/目标资产精度，拒绝源金额超精度；`src/modules/convert/service.rs:242` 和第 250 行量化费用及目标金额。
- 理财申购读取资产精度校验，赎回按币种量化；贷款本金和计息同样有精度保护。不能把杠杆/现货缺口泛化到这些产品。
- 提现报价本就按资产精度生成权威金额（`src/modules/wallet/application.rs:779`）；属于明确报价量化。不能将其与隐式 Number 丢位混为一谈。

## 验证

从仓库根目录执行：

```sh
node .trellis/tasks/09-19-mobile-backend-precision-review/research/mobile-repro.mjs
node .trellis/tasks/09-19-mobile-backend-precision-review/research/backend-repro.mjs
cargo test --lib -- precision decimal convert loan earn --skip explicit_inventory_concurrency_rollback_replay_and_revision
```

Rust 61/61；显式排除需要独立数据库的库存用例，不把其缺环境提前返回计为实库验证。Rust 复现使用真实源函数和当前已编译依赖，仅外围 DTO/错误类型为夹具；现货域文件完整引用。没有替代 SQL 层，故不证明实际数据库落库结果。

在 mobile 目录执行：

```sh
node --test --experimental-strip-types tests/finance-decimal-lifecycle.test.ts tests/finance-retry-adapters.test.ts tests/trade-seconds-decimal-derivations.test.ts tests/withdrawal-quote-contract.test.ts tests/seconds-api-adapter.test.ts
```

Mobile 36/36。真实适配器由 Vite SSR 加载、网络被内存替身替换，未操作线上。

只新增任务审查材料、复现脚本和进度记录，业务代码未修改。未运行全量构建、浏览器或数据库端到端；未核对生产资产配置、历史尾差或所有链适配器。规范不改：现有金额规范已有资产精度合同，本轮记录实现缺口，尚未决定新的舍入政策。
