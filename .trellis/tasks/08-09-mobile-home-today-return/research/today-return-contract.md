# 今日收益接口口径研究

## 现状

- 手机端首页 `HomeView.vue` 的“今日收益”金额和比例均固定为 `--`。
- 当前后端只有现货/保证金钱包余额与流水，没有资产组合日快照、统一成本基础或现成收益接口。
- 业务表中可精确取得的收益包括：秒合约结算盈亏、预测订单结算净额、已平仓杠杆盈亏扣利息、理财赎回净收益。
- 现货成交没有用户级 FIFO/移动平均成本账本；仅凭余额和成交不能稳定还原“已实现现货收益”。
- Redis ticker 可将非稳定币收益换算为 USDT，但行情缺失时必须显式标记不完整。

## 可选口径

### A. UTC 自然日资产组合盈亏

需要日初资产快照、净入金剔除、历史价格、活动秒合约/预测/理财本金、杠杆未实现权益和贷款负债。当前没有统一快照及成本基础，直接实现容易把充值、划转或活动本金误计为收益。

### B. 当前持仓近 24 小时涨跌

可用 ticker 的 24 小时变动乘当前持仓快速估算，但不考虑持仓变化、充值、交易时间和产品结算，且“24 小时”不等同“今日”。不适合作为权威账户收益。

### C. UTC 自然日已实现业务收益（推荐）

按完成时间聚合可审计的结算记录：

- 秒合约：胜出为 `stake * payout_rate`，失败为 `-stake`。
- 预测：`payout + refund + fee_refund - stake - fee`。
- 杠杆：人工平仓 `closed` 与系统强平 `liquidated` 均按
  `realized_pnl - interest_amount`。
- 理财：当日 `earn_redeem` 钱包流水减订阅本金。

按资产聚合后用当前 USDT ticker 换算；USDT/USDC/USD 按 1 计价。当前行情必须交易对匹配、价格为正，且 `observed_at` 不得晚于计算截止、距计算截止不超过 60 秒；缺失、畸形、错配、非正数、未来和过期行情一律视为缺价。理财流水按用户、资产、引用类型和订阅 ID 精确连接；同一订阅的历史重复赎回流水只采用最早权威记录。返回 `scope=realized`、UTC 起止时间、计价币种、收益金额、收益成本基础、收益率、完整性状态和缺失价格资产。该口径不把充值、提现、内部划转计为收益，也不伪造现货成本或未实现收益。

## 建议接口

`GET /wallet/today-return`

响应字段：

- `scope`: 固定 `realized`
- `reporting_asset`: `USDT`
- `amount`: 已实现收益 USDT
- `basis_amount`: 已结算业务成本基础 USDT
- `rate`: `basis_amount > 0` 时为 `amount / basis_amount`，否则 0
- `period_start_at`: 当前 UTC 自然日开始
- `calculated_at`: 计算时间
- `status`: `complete | partial`
- `missing_price_assets`: 无法换算的资产代码

## 边界

- 无结算业务时返回完整的零收益，不返回缺失值。
- 预测退款仍以原始 `stake + fee` 作为成本基础；退款与手续费退款只改变净收益，避免把退回金额再当作收益或重复扣减成本。
- 任何非零收益/成本资产缺少有效、当前 USDT 价格时标记 `partial`；前端继续显示非数值状态，不能展示部分合计。
- 手机端请求按精确登录 token 隔离并采用 latest-request-wins；换号、退出和卸载后的迟到响应不得写回。隐私隐藏优先于缺价详情，不能通过缺价资产代码泄露账户活动。
- 当前任务不声明包含现货成本收益、持仓未实现盈亏或完整净资产收益；这些需要独立资产快照/成本账本项目。

## 影响范围

- 后端：`src/modules/wallet/{routes,application,infrastructure,presentation}.rs`
- 后端测试：`tests/wallet_routes.rs` 及钱包领域/查询测试
- 手机端：`mobile/src/api/wallet.ts`、`mobile/src/views/HomeView.vue`、相关首页测试和中英文资源
- 规范：钱包金额/移动端后端集成合同
