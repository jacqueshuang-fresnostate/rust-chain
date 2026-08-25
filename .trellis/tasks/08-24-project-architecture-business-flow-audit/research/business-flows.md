# Research: 核心业务流程与资金不变式审计

- Query: 只读审计注册/钱包初始化、充值/提现、现货、杠杆、秒合约、闪兑、借贷、理财、预测/新币、代理归属与在线客服；检查幂等、事务、余额锁定、账本、状态机、补偿、outbox/MQ、私有 WS 与读模型一致性。
- Scope: internal
- Date: 2026-08-24

## Findings

### 结论摘要

- 已确认 **9 项 P0、10 项 P1**。最高风险集中在：提现广播结果不确定时错误解冻、新币可由客户端定价且不受总量约束、解禁费只改状态不扣款、抵押借贷无 LTV/估值/清算、秒合约不按到期时点定价、预测市场可在结束后继续下注、闪兑接受无新鲜度的缓存价格、全仓资金可无风险校验转出，以及配置的充值费未实际收取。
- 多数单模块资金写入已经采用 MySQL 事务、`FOR UPDATE` 与业务状态幂等；主要缺口位于跨时间、跨存储、跨服务与平台总账层，而非简单的“余额更新没开事务”。
- 在线客服的消息持久化、精确代理隔离、发送幂等、已读游标和改派事务符合项目规格；其 WS 明确只是可丢刷新提示。代理归属建立也具备邀请码行锁、一次绑定与事务计数；风险在返佣的后续撤销/退款补偿。

### P0-01 提现广播结果不确定时会在重试耗尽后解冻，可能形成链上已付款而站内余额退回

- **证据**：
  - `src/modules/wallet/infrastructure/withdrawals.rs:42-48`，`HttpWalletChainGateway::broadcast_withdrawal` 的合同明确指出传输失败/超时可能发生在远端已受理之后，此时不得释放 frozen，应按 `request_id` 重试或查询。
  - `src/workers/wallet_chain.rs:144-180`，`run_once_with_gateway` 将所有 `Err` 统一累计次数，达到 `max_attempts` 后调用 `release_withdrawal_in_tx(..., "failed", ...)` 释放全部预留。
  - `src/workers/wallet_chain.rs:340-375` 与 `src/modules/wallet/infrastructure/withdrawals.rs:593-618`：后到的 confirmed 回执若本地已是 `failed`，既不能重新登记 broadcasted，也不满足进度更新允许的状态，不能自动修复。
- **失败场景**：网关已按 `request_id` 广播链上交易，但 API 响应连续超时；本地达到最大次数后把 `amount + fee` 退回 available。链上交易随后确认，用户同时获得链上资产和恢复的站内余额。
- **建议**：区分“确定性未受理”与“结果未知”；传输超时、连接中断、无效响应一律进入 `manual_review/unknown` 并保留 frozen；网关增加按 `request_id` 查询接口，只有权威查询确认未受理或确定性业务拒绝才允许解冻。
- **验收**：构造“远端受理但每次响应超时”，超过最大重试后请求仍保留 frozen；后续 confirmed 回执能且只能核销一次；确定性拒绝仍能一次性释放；进程重启和重复回执结果一致。
- **依赖**：链网关幂等/查询合同、提现状态枚举与迁移、运维人工复核队列。

### P0-02 新币申购/上市后购买由客户端决定支付与分配比例，且未执行 `total_supply` 上限

- **证据**：
  - `src/modules/new_coin/application.rs:311-369`，`create_new_coin_subscription_with_internal` 只校验 `quote_amount`、`quantity` 为正，未校验 `quote_amount == issue_price × quantity`、计价资产白名单或配额。
  - `src/modules/new_coin/infrastructure.rs:637-711`，`create_subscription_order` 不在事务内重锁项目；按客户端 `quote_amount` 扣款，再按客户端 `quantity` 分配，`issue_price` 仅作为解禁记录价格参数传入。
  - `src/modules/new_coin/application.rs:447-501`，`create_new_coin_purchase_with_internal` 接受客户端 `price`，直接计算 `price × quantity`。
  - `src/modules/new_coin/infrastructure.rs:726-806`，购买事务虽锁项目和交易对，却不读取/核对权威成交价，仍按请求价格扣款与分配。
  - `migrations/0006_new_coin_lifecycle.sql:1-23` 定义 `total_supply`；但 `src/modules/new_coin/infrastructure.rs:855-868` 的下单规则查询不读取该字段，`apply_new_coin_allocation`（`920-988`）也没有已分配量/剩余量检查或原子扣减。
- **失败场景**：用户提交极小正价格/支付额和极大数量，低成本获得大量新币；并发请求可持续分配超过项目 `total_supply`。
- **建议**：由服务端签发报价或使用新鲜权威市场价；项目配置明确计价资产；在项目/库存行上维护 `reserved/allocated/remaining_supply`，与订单、扣款、锁仓同事务原子扣减；订阅按发行价或后台审批的配额计算，不接收独立且不自洽的金额与数量。
- **验收**：篡改 price、quote asset、quote amount/quantity 比例均在动账前失败；并发总分配永不超过 `total_supply`；同键同参重放不重复占用供给，同键异参冲突。
- **依赖**：新币定价与配额产品规则、库存字段迁移、资产精度与报价服务。

### P0-03 新币解禁费接口只把状态改为 paid，不扣钱包也不写流水

- **证据**：
  - `src/modules/new_coin/application.rs:158-185`，`pay_new_coin_unlock_fee` 仅核对应收参数后调用 `mark_unlock_fee_paid`；注释 `162-163` 明确当前实现不扣钱包、不写流水。
  - `src/modules/new_coin/infrastructure.rs:393-417`，`mark_unlock_fee_paid` 只执行 `UPDATE asset_unlock_records SET fee_paid_status='paid'`。
  - `src/modules/new_coin/repository.rs:68-76`，既有仓储合同反而要求钱包扣款、流水和 paid 状态在同一事务并防止重复收费，当前异步实现违反该合同。
  - `src/modules/new_coin/infrastructure.rs:455-588`，释放路径只要看到 paid 且到期，就将 locked 等额转 available。
- **失败场景**：用户提交记录中预期的资产和金额，余额为零也能得到 `paid=true`，随后释放全部锁仓资产。
- **建议**：缴费事务按“解禁记录→缴费资产钱包”锁序，检查 pending 和余额，扣 available、写费用流水/平台收入腿、置 paid；重复调用返回原支付结果且不重复扣款。
- **验收**：余额不足不改状态；成功时钱包减少值、流水金额和应收快照完全一致；两个并发缴费仅一次生效；缴费事务回滚后 release 仍被拒绝。
- **依赖**：费用收入账户/总账设计、钱包锁序、现有移动端响应兼容。

### P0-04 抵押借贷没有估值、LTV、抵押资产规则或清算，任意正数抵押可支持最高额度贷款

- **证据**：
  - `src/modules/loan/application.rs:276-365`，创建订单对抵押借贷仅在 `298-315` 校验抵押资产存在、金额为正与精度；未计算抵押价值或健康度。
  - `migrations/0071_user_loans.sql:1-25` 的产品字段没有 LTV、清算阈值、抵押资产白名单或价格源配置。
  - `src/modules/loan/application.rs:556-593`，批准后直接将全部 principal 贷记用户钱包。
  - `src/workers/loan_overdue.rs:104-159` 只将到期订单推进为 overdue；未发现抵押品处置、追保或清算资金路径，且该 worker 默认关闭（`src/workers/loan_overdue.rs:18-25`）。
- **失败场景**：用户以无价值资产或极小抵押金额申请产品最大贷款，管理员批准后转走本金并违约；系统没有自动止损或可执行的抵押处置。
- **建议**：产品增加抵押资产白名单、初始/维持 LTV、清算 LTV、价格源和新鲜度；申请与批准均以服务端估值重算并快照；实现健康度扫描、补仓/冻结、幂等清算和抵押品处置；逾期任务生产默认必须显式配置并可观测。
- **验收**：低于最低抵押价值的申请/批准失败；价格过期时拒绝放款；价格下跌跨阈值只清算一次；本金、抵押、利息、坏账和状态可逐笔对账。
- **依赖**：贷款产品模型、行情/oracle、抵押清算规则、平台总账。

### P0-05 秒合约按 worker 实际处理时的最新价结算，而不是到期时点价格

- **证据**：
  - `src/workers/seconds_contract_settlement.rs:347-370`，候选只按 `expires_at <= now` 筛选，但投影不包含到期时点价格或行情标识。
  - `src/workers/seconds_contract_settlement.rs:392-423`，`cached_ticker_price` 读取当前 Redis 最新 ticker，只要求相对本轮 `now` 在 60 秒内。
  - `src/workers/seconds_contract_settlement.rs:426-530`，该最新价直接决定胜负并写入 `settlement_price`，未校验 `observed_at` 与订单 `expires_at` 的距离。
- **失败场景**：订单 12:00:00 到期时应为赢，worker/Redis 故障后 12:03 恢复，此时价格反向；订单按 12:03 价格结算，结果取决于处理延迟和重试次数。
- **建议**：结算使用不可变的事件时间价格（到期秒对应 tick、1s/1m candle close 或带序号行情存档），持久化 `settlement_price_observed_at/source_id`；没有合格到期价时保持待确认，不能改用未来最新价。
- **验收**：同一订单准时与延迟 5 分钟处理结果完全一致；到期前/后窗口外 ticker 均不能结算；重复 worker 和人工结算使用同一价格快照。
- **依赖**：历史行情存储、到期价格选取规则、订单快照字段。

### P0-06 预测市场报价/下单不检查本地结束时间与同步新鲜度，可在已知结果后继续下注

- **证据**：
  - `src/modules/prediction/infrastructure.rs:99-198` 的报价路径在 `119-142` 只检查展示状态与 settlement open。
  - `src/modules/prediction/infrastructure.rs:292-328` 的下单事务重检同样没有 `now < end_at` 或同步年龄条件。
  - `src/modules/prediction/infrastructure.rs:1304-1325` 的市场锁查询已经读取 `end_at`、`last_synced_at`，但上述资金闸门未使用这些字段。
  - `.trellis/spec/backend/prediction-markets.md:93-104` 要求 inactive/closed/settled/refunded 拒绝，当前仅依赖异步同步更新状态。
- **失败场景**：外部事件已结束或结果已公开，但 Polymarket 同步延迟，本地仍为 open；用户获得旧赔率并下注，形成确定性套利。
- **建议**：报价和消费报价的事务内都强制 `CURRENT_TIMESTAMP < end_at`；增加 `last_synced_at` 最大年龄和独立本地关盘任务；订单消费时再次验证报价绑定的市场版本/价格版本。
- **验收**：结束边界前可下单、等于或晚于 `end_at` 必拒绝；同步停止超过阈值时不得报价/下单；并发关盘与下单只有一个终态胜出且无资金半提交。
- **依赖**：市场时钟语义、同步 SLA、市场版本字段与关盘 worker。

### P0-07 闪兑市场报价只读取 `last_price`，没有 `observed_at`/新鲜度校验

- **证据**：
  - `src/modules/convert/infrastructure.rs:360-392`，市场汇率读取 Redis ticker 后只解析并验证正数 `last_price`，没有读取或比较 `observed_at`。
  - `src/modules/convert/application.rs:65-150` 使用该汇率生成可确认报价；`src/modules/convert/infrastructure.rs:432-585` 随后按报价完成真实双资产钱包结算。
  - 相邻高风险资金路径（例如 `.trellis/spec/backend/seconds-contracts.md:75-90`、`.trellis/spec/backend/margin-trading-actions.md:21-31`）均要求 60 秒新鲜度，闪兑缺失同等边界。
- **失败场景**：行情写入停止但 Redis 键长期存在；用户在市场大幅变动后继续按旧价格兑换，直接消耗平台库存/负债能力。
- **建议**：统一使用包含 symbol、last_price、observed_at 的严格 ticker 解析器；市场定价超过配置年龄立即失败；报价持久化行情时间、来源和版本，确认时校验该报价版本而非重新猜测。
- **验收**：缺失、错 symbol、非正、未来、超过 60 秒的 ticker 均不能创建报价且零资金变动；有效报价可按快照在有效期内确定性结算。
- **依赖**：统一行情 DTO、闪兑报价表字段、价格新鲜度配置。

### P0-08 全仓保证金可直接转回现货，未验证转出后的账户权益是否仍高于维持保证金

- **证据**：
  - `src/modules/margin/application/account_settings.rs:49-155` 的 `transfer_margin_funds` 在 margin→spot 分支没有账户风险检查。
  - `src/modules/margin/infrastructure/transfers.rs:134-193` 仅验证 `margin_wallet.available >= amount`，随后直接两侧动账。
  - `src/modules/margin/application/queries.rs:242-288` 与 `src/workers/margin_liquidation.rs:729-780` 表明全仓权益明确包含 `margin_wallet.available + position margins + PnL - interest`，因此 available 是风险资本的一部分。
  - `.trellis/spec/backend/margin-trading-actions.md:53-57` 定义全仓账户级权益和统一强平，产品能力允许 cross。
- **失败场景**：账户只比维持保证金多很小缓冲，但 margin wallet 仍有 available；用户把 available 全转走，事务提交后账户立即低于维持保证金并产生坏账窗口。
- **建议**：margin→spot 前用新鲜且同一批标记价计算转后权益，保留维持保证金和安全缓冲；在事务内锁全仓仓位与钱包并以账户 version 防 TOCTOU；liquidating/price-unavailable 状态禁止转出。
- **验收**：最大可转额恰为风险缓冲；多仓/对冲/含利息账户均正确；转账与开仓、计息、平仓、强平并发时不会提交低于阈值的转出。
- **依赖**：风险计算复用、稳定锁序/账户版本、行情预取策略。

### P0-09 已配置的充值手续费没有进入充值入账计算

- **证据**：
  - `migrations/0063_asset_deposit_withdraw_fee_settings.sql:1-4` 为资产增加 `deposit_fee`。
  - `src/modules/wallet/infrastructure/deposits.rs:398-425` 的充值目标/资产查询没有读取该费率。
  - `src/modules/wallet/infrastructure/deposits.rs:697-745` 的确认入账将事件 `amount` 全额增加 available 并按全额写 deposit 流水。
  - `migrations/0087_p0_financial_safety.sql:83-107` 的充值事件快照没有 gross/fee/net 字段；冲正（`src/modules/wallet/infrastructure/deposits.rs:486-562`）也只能按全额反向。
- **失败场景**：运营配置非零充值费并向用户展示，链上到账 100 后系统仍贷记 100；平台持续少收费用，且历史记录无法还原当时费率与净额。
- **建议**：事件首次确认时固化 gross、fee rule/version、fee、net；只贷记 net，费用进入平台账户/总账；冲正按原快照反向，禁止按当前配置重算。
- **验收**：固定费/比例费的边界、精度截断、最小充值和冲正测试均满足 `gross = net + fee`；重复确认/冲正不重复动账。
- **依赖**：充值费业务口径、事件/记录迁移、平台费用账户。

### P1-01 注册后的钱包初始化依赖未配置且不自建拓扑的 MQ 消费链

- **证据**：
  - `src/modules/auth/application.rs:461-521` 注册事务写用户、推荐关系和 `user.created` outbox 后提交，钱包不在注册事务内创建。
  - `src/modules/events/service/production_dispatch.rs:138-173` 只有 `user.created` 真正触发钱包初始化；`src/modules/events/infrastructure.rs:260-304` 的初始化本身是幂等的 `INSERT IGNORE ... SELECT assets`。
  - `src/workers/event_inbox.rs:58-112` 与 `src/main.rs:284-311`：队列名缺失/空白即完全不启动实时消费和数据库补偿；仓库 `.env:22` 的 `EVENT_INBOX_QUEUE_NAME` 为空。
  - `src/modules/events/service/rabbitmq.rs:141-211` 明确要求队列预先声明和绑定，本实现不建拓扑；现有 compose/部署样例未发现该队列/绑定配置。
  - `src/modules/events/service/rabbitmq.rs:107-135` 未启用 publisher confirm，代码合同说明不能证明 broker 已持久接收。
  - `src/modules/wallet/infrastructure/accounts_ledger.rs:685-703` 只返回既有钱包行；`src/modules/spot/infrastructure/wallet_accounts.rs:37-56` 对缺失钱包直接拒绝下单。
- **失败场景**：用户注册成功但事件无队列可路由或消费未启动，钱包列表为空、现货下单失败；outbox 已标记发布时 broker 实际未持久化还可能永久遗漏。
- **建议**：部署时声明 durable queue、binding `user.*.created` 和 DLQ；生产队列为空应启动失败而非静默关闭；启用 publisher confirms；增加按 users×assets 扫描的幂等钱包补偿任务和缺口指标。
- **验收**：全新环境注册后在 SLA 内出现全部资产账户；停 RabbitMQ 后恢复可自动补齐；删除一条钱包后补偿可恢复且不影响余额；无绑定/无 confirm 时健康检查失败。
- **依赖**：RabbitMQ 拓扑/IaC、部署变量、补偿 worker 与监控。

### P1-02 提现风控限频规则在该入口永远不生效

- **证据**：`src/modules/wallet/application.rs:702-719` 的提现创建显式向 `enforce_risk_control` 传 `None`，注释直接说明该路径不持有 Redis、限频不生效；`src/modules/risk/application.rs:77-103` 在 Redis 缺失时返回无 request count，跳过限频维度。
- **失败场景**：后台配置每用户/资产每分钟次数限制后，攻击者仍可高频提交提现尝试，放大安全凭据猜测、数据库/审计负载与人工审核队列压力。
- **建议**：从 `AppState` 传入 Redis 连接，或为高风险提现使用数据库原子限频兜底；Redis 故障时提现限频应 fail-closed 或进入加强验证，而不是静默放行。
- **验收**：配置 N 次后第 N+1 次在任何 API 实例均被拒绝；Redis 故障行为符合明确策略并产生日志/指标；拒绝不消耗提现幂等键或冻结余额。
- **依赖**：提现用例签名/AppState、风险降级策略、Redis 高可用。

### P1-03 “现货撮合”没有自动用户对用户订单簿，行情触发路径固定与系统流动性账户成交

- **证据**：
  - `src/modules/spot/application/triggering.rs:35-76` 每次行情仅扫描四类触发候选并逐单执行。
  - 同文件 `301-458`、`461-618` 的买卖执行均创建系统流动性对手单，并依赖 `ensure_spot_liquidity_inventory_in_tx`。
  - `src/modules/spot/infrastructure/wallet_accounts.rs:225-287` 要求系统做市用户预充值库存。
  - 用户对用户四腿结算存在于 `src/modules/spot/application/settlement.rs:31-179`，但公开调用是后台手工 fill（同文件 `214-234`），未发现按价格时间优先持续匹配 crossed orders 的 worker/队列。
- **失败场景**：两个用户的买卖限价已经交叉，但没有后台 fill 时仍不互相成交；系统流动性库存不足则所有可触发订单失败，即使用户侧存在可成交对手单。
- **建议**：若产品承诺交易所撮合，引入单 pair 串行撮合/确定性撮合序号、价格时间优先、成交幂等键和恢复游标；系统流动性只作为普通受控做市账户参与，而不是唯一自动对手方。
- **验收**：交叉订单无需管理员即可按价格时间优先成交；多实例/重启不重复成交；部分成交、撤单竞态、库存耗尽和账本四腿均可重放验证。
- **依赖**：产品确认（做市柜台还是订单簿）、撮合分区/序列、行情与私有事件合同。

### P1-04 多条资金路径违反统一资产精度合同

- **证据**：
  - 规格 `.trellis/spec/backend/wallet-amount-precision.md:12-22` 要求所有用户输入和计算金额服从 `assets.precision_scale`，目标金额在报价、订单、钱包、流水前统一截断。
  - 现货交易对后台仅校验非负精度且无上限/资产交叉校验（`src/modules/admin/service/market.rs:698-716`）；成交额直接 `price × quantity`（`src/modules/spot/application/settlement.rs:62-73`）后原样写钱包（`133-153`）。
  - 杠杆开仓规则只校验产品范围/杠杆（`src/modules/margin/application/open_position.rs:296-318`），抵押扣款使用原始 `request.margin_amount`（`182-190`）。
  - 理财申购只按 DECIMAL(38,18) 口径校验，产品规则不含资产精度（`src/modules/earn/application.rs:459-513`、`src/modules/earn/infrastructure.rs:600-620`），申购/赎回按原始或 18 位计算值动账（`src/modules/earn/infrastructure.rs:685-775`）。
  - 新币只做正数校验（`src/modules/new_coin/application.rs:342-344,478-489`），分配原始数量（`src/modules/new_coin/infrastructure.rs:920-967`）。
  - 快捷充值资产查询只取 id/symbol（`src/modules/quick_recharge/infrastructure.rs:744-758`），`actual_amount` 原样贷记（`771-809`）。
- **失败场景**：precision=2 的资产被写入 0.001 等不可表示业务金额；后续某些模块截断、另一些模块保留，产生余额/流水/订单不一致、舍入套利和无法对账的 dust。
- **建议**：所有资金用例统一加载并快照资产精度；用户源金额超精度拒绝，计算目标/费用向零截断；交易对 `qty_precision <= base precision`，可生成的 quote amount 必须适配 quote precision。
- **验收**：每条列举路径对 precision 0/2/8/18 做输入与计算回归；数据库断言所有 wallet/ledger 金额符合资产精度且 after 快照逐笔可复算。
- **依赖**：共享金额库、交易对/产品后台校验、历史 dust 清理方案。

### P1-05 杠杆利息使用当前产品利率追溯整个窗口，并永久丢失不足整小时的时间

- **证据**：
  - `src/workers/margin_interest.rs:265-288` 的 `lock_position` 联出产品**当前** `hourly_interest_rate`，仓位没有利率快照/有效期历史。
  - `src/workers/margin_interest.rs:213-235` 按 `full_elapsed_hours` 计息后把 `interest_accrued_at` 直接写为 `now`。
  - `src/workers/margin_interest.rs:291-311` 只收费完整小时；例如经过 1h30m 收 1h 后检查点跳到当前时刻，30m 不会进入后续窗口。
- **失败场景**：worker 停机 24 小时期间管理员在末尾调高利率，恢复后 24 小时全按新利率收费；或 worker 每 70 分钟运行一次，长期每轮永久少计 10 分钟。
- **建议**：开仓快照利率或维护带生效时间的利率历史，按区间分段；检查点只推进 `elapsed_full_hours`，保留余数，或按秒精确计息并明确舍入。
- **验收**：分割同一时间区间为任意 worker 调度，累计利息完全相同；利率切换前后各按对应区间；重启/重放不重复或漏计。
- **依赖**：计息产品口径、利率历史/仓位快照迁移、历史数据回填。

### P1-06 逐仓穿仓缺口被截零但没有坏账记录或补偿状态

- **证据**：`src/workers/margin_liquidation.rs:576-680` 的 `liquidate_position_by_id` 在 `621` 将负 equity 截为零 payout；注释 `583` 明确逐仓路径不单独登记坏账。相比之下，全仓路径在 `699-880` 计算并持久化 bad debt，`migrations/0087_p0_financial_safety.sql:1-4` 也只给 cross account 增加坏账字段。
- **失败场景**：行情跳空令逐仓权益为 -100，仓位被标记 liquidated、用户获得 0，但平台 100 的损失没有结构化事实，风险/财务汇总看不到真实缺口。
- **建议**：逐仓清算记录增加 `bad_debt_amount`，按 `max(-equity,0)` 与终态同事务落库；定义保险基金/平台损失账户及后续追偿状态。
- **验收**：正/零/负权益清算均满足 `payout=max(equity,0)`、`bad_debt=max(-equity,0)`；重放不重复；汇总与总账可对上。
- **依赖**：强平记录迁移、保险基金/会计政策、后台报表。

### P1-07 预测订单/市场结算的重放语义不完整，且整市场单事务无上限

- **证据**：
  - `src/modules/prediction/infrastructure.rs:355-365,406-413`，订单唯一冲突后只按用户+幂等键返回旧订单，不比较本次 `quote_id`。
  - `src/modules/prediction/infrastructure.rs:441-445`，市场达到终态后直接返回，不比较重放的 result/refund policy。
  - `src/modules/prediction/infrastructure.rs:461-560` 一次 `SELECT ... FOR UPDATE` 取出该市场全部 open 订单，无 LIMIT/检查点，并在同一事务逐个锁钱包和结算。
- **失败场景**：客户端误复用 key 到另一报价却收到旧订单成功响应；管理员用相反结果重放已结算市场也看似成功；热门市场订单过多导致长事务超时/死锁，整个市场永久无法结算。
- **建议**：订单保存 request fingerprint 并比较 quote；终态重放核对结果和退款策略；市场结算引入 run/version、分批 claim、逐单幂等结果和完成计数，零 open 后再原子 finalize。
- **验收**：同键异 quote 冲突；终态异结果冲突并告警；十万订单可中断续跑，任一订单只结算一次，最终汇总数/金额一致。
- **依赖**：结算运行表/字段、后台 API 语义、批处理与监控。

### P1-08 闪兑报价跨 MySQL/Redis 的状态机有过期 TOCTOU，反向兑换钱包锁序还会形成死锁

- **证据**：
  - `src/modules/convert/application.rs:65-150` 先持久化 MySQL 报价，再写 Redis；缓存失败会留下数据库报价但客户端无法确认。
  - `src/modules/convert/application.rs:152-187` 在事务外用 Redis 判断存在/过期，随后进入结算。
  - `src/modules/convert/infrastructure.rs:432-585` 的结算事务没有锁定并复核 MySQL 报价 expiry/consumed 状态。
  - 同文件 `483-486` 按 from 后 to 的业务方向锁钱包，代码注释已指出 A→B 与 B→A 存在反向锁等待。
- **失败场景**：报价在 Redis 校验后、资金事务提交前过期仍成交；Redis 写失败形成孤儿；同一用户并发 A→B、B→A 可死锁并向客户端返回不确定错误。
- **建议**：MySQL 报价作为权威状态，确认事务 `FOR UPDATE` 校验 owner、request hash、expires_at、consumed_at 并一次消费；Redis仅缓存；钱包统一按 `(user_id,asset_id)` 排序加锁，并对数据库死锁做有界幂等重试。
- **验收**：过期边界与并发确认只有一次成功；Redis 全不可用仍可按明确策略确认/拒绝；双向并发无环形锁或能透明重试且不重复动账。
- **依赖**：报价表状态字段、缓存降级策略、共享钱包锁序工具。

### P1-09 代理返佣没有随上游退款/作废补偿，自动结算也不复核来源终态

- **证据**：
  - `src/modules/prediction/infrastructure.rs:389-400` 在预测订单创建时即按 stake 生成 pending 返佣。
  - 同文件 `478-506` 可将无效市场订单退本金/手续费，但没有拒绝或反冲对应 `agent_commission_records`。
  - `src/workers/agent_commission_settlement.rs:138-154` 只按 pending、账龄、id 扫描；`87-99` 直接调用打款，不复核 source 仍有效。
  - `src/modules/admin/application/agents.rs:547-613` 在锁住 pending 后直接贷记代理用户钱包并置 settled；未发现 reversal/clawback 状态或反向流水。
- **失败场景**：预测市场一小时后返佣已自动打款，之后市场被判 invalid 且用户手续费全退；平台没有收入却已支付佣金，且无法自动追回。其他可撤销业务同样缺统一补偿协议。
- **建议**：返佣增加 `eligible_at/source_status/reversed_at`；上游退款事务同步拒绝 pending 佣金，已结算则生成不可变反向返佣/应收；打款事务必须按 source_type 复核权威终态和可返佣基数。
- **验收**：退款发生在打款前/后两种时序最终净返佣均符合政策；并发退款与结算只产生一个可解释结果；每笔正反流水可按 source_id 对账。
- **依赖**：各产品返佣确认时点、佣金状态机迁移、负佣金/追偿政策。

### P1-10 缺少平台级双重记账/清算账户，内部产品的资产守恒无法从仓库内证明

- **证据**：
  - `migrations/0003_assets_wallet_ledger_locks.sql:25-42` 的 `wallet_ledger` 只记录单个用户/资产/余额桶 after 快照，没有 journal transaction、counter account 或每资产借贷平衡约束。
  - 闪兑 `src/modules/convert/infrastructure.rs:492-580`、贷款放款/还款 `src/modules/loan/application.rs:556-681`、理财申购/赎回 `src/modules/earn/application.rs:634-716,686-775`、秒合约 `src/modules/seconds_contract/application.rs:369-416` 与 worker `465-500`、预测 `src/modules/prediction/infrastructure.rs:1490-1725` 都主要表现为用户钱包单边减少/增加；仓库内未发现对应 treasury/clearing/reserve 总账。现货系统流动性用户是少数具备真实对手账户的例外。
- **失败场景**：代码错误多贷记、产品亏损、利息/收益/赔付或代理佣金导致平台净负债变化时，只能看到用户余额，无法用“每业务事务借贷和为零”快速定位差额，也无法证明用户负债受储备覆盖。
- **建议**：建立不可变双重记账 journal（业务事务 ID、资产、账户、借/贷腿），为平台库存、费用收入、贷款应收、理财负债、保险基金、坏账配置系统账户；钱包余额作为总账派生/受控读模型，并做每日储备与链上资产对账。
- **验收**：每笔内部业务按资产满足 debit=credit；钱包三桶与总账子账户可全量重算；故意删除/重复一腿时巡检必告警；储备—用户负债—平台权益形成可解释等式。
- **依赖**：财务科目与资产负债政策、迁移/回填、外部托管与链上对账。本项基于仓库内“未发现”，若另有仓库外权威总账需以接口和对账证明降级。

### P1-11 财务私有 WS 事件全部为单进程尽力广播，业务 outbox 没有覆盖这些状态变更

- **证据**：
  - `src/modules/events/service/websocket.rs:499-537`，`EventBroadcastHub` 是容量有限的进程内 broadcast，慢消费者、无消费者、重启与跨实例均可丢失，不落库、不重试。
  - `src/modules/events/service/production_dispatch.rs:138-164` 除 `user.created` 外，钱包、现货、闪兑、新币等已枚举事件消费均直接 `Ok(())` 无副作用。
  - 实际财务事件由各模块/worker 直接 `private_user` 发布，例如秒合约 `src/workers/seconds_contract_settlement.rs:533-560`、强平 `src/workers/margin_liquidation.rs:927-949`、理财 `src/workers/earn_auto_redemption.rs:300-315`、新币 `src/modules/new_coin/application.rs:193-220,399-429`；未写同事务 outbox。
  - 在线客服明确把此行为作为提示合同：`src/modules/support/application.rs:1-6,142-165` 与 `.trellis/spec/backend/online-support.md:14-16,105-126` 要求 REST 最终对齐；其他财务读模型也需要同等明确的轮询/版本恢复。
- **失败场景**：写请求落在实例 A，用户 WS 连在实例 B，永远收不到成交/强平/结算提示；若客户端只靠增量事件更新余额/订单，会长期显示旧状态且无法判断漏了几条。
- **建议**：二选一并写入合同：①财务状态同事务 outbox→broker/pubsub→所有 API 实例 hub，并提供 event id/sequence 和重放游标；②明确 WS 仅提示，所有客户端定时/重连拉取权威 REST 快照并使用版本号检测缺口。关键强平/提现通知另设可靠通知通道。
- **验收**：双实例、断线、hub lag、进程重启测试后客户端能自动收敛到 MySQL 权威状态；重复事件不重复应用；事件乱序不会回退读模型版本。
- **依赖**：事件拓扑、客户端 store/reconciliation、读模型版本/游标。

### 流程覆盖与已确认的正向不变量

| 流程 | 已确认实现 | 结论 |
|---|---|---|
| 注册/钱包初始化 | 注册、邀请码计数、推荐关系、`user.created` outbox 在同一事务；钱包批量初始化幂等 | 事务设计正确，生产启动/拓扑存在 P1-01 |
| 充值 | 链事件以 `(network, tx_hash, event_index)` 去重；确认、钱包、流水同事务；余额不足的冲正进入人工处理 | 基础状态机稳健，收费缺口见 P0-09 |
| 提现 | 申请、available→frozen、流水同事务；确认只从 frozen 核销；重复确认幂等 | 广播不确定性 P0-01；限频 P1-02 |
| 现货 | 订单/钱包稳定锁序、预留、四腿结算、成交幂等和佣金同事务 | 精度 P1-04；自动订单簿缺失 P1-03 |
| 杠杆划转/开平仓 | 双向划转固定 spot→margin 锁序并写配对流水；仓位终态先锁仓位并按原 wallet_scope 返还 | 全仓转出 P0-08；精度/计息/逐仓坏账见 P1-04/05/06 |
| 秒合约 | 开仓扣款、订单、流水同事务；订单终态防重复派奖；赔付按资产精度截断 | 到期价格口径 P0-05 |
| 闪兑 | 报价/订单具备唯一标识，最终双钱包变更与流水同事务 | 新鲜度 P0-07；报价状态与锁序 P1-08 |
| 借贷 | 申请、抵押冻结、审批/还款状态均有行锁与事务；还款释放抵押 | 风险模型缺失 P0-04 |
| 理财 | 申购扣款、赎回状态和入账同事务；手工/自动赎回复用计算口径和费用快照 | 精度 P1-04；平台总账 P1-10 |
| 预测 | 后端 quote 单次消费、stake 冻结/fee 扣减、派奖/退款与状态同事务 | 关盘 P0-06；重放/批结算 P1-07 |
| 新币 | 订单、支付、资产分配与锁仓在单事务；解禁释放 locked→available 写双腿流水 | P0-02/P0-03 |
| 代理归属 | 注册/后绑邀请码均锁码、校验用量与 active 代理链，一次绑定；返佣生成与原业务事务一致并有唯一来源键 | 归属可靠；退款补偿 P1-09 |
| 在线客服 | 用户唯一会话、不可变消息、同正文幂等重放/异正文冲突、精确代理权限、已读目标归属验证、改派与会话同步同事务 | 持久读模型符合规格；WS 恢复边界并入 P1-11 |

### 横向控制检查

| 控制 | 现状 |
|---|---|
| 幂等 | 提现、现货成交、杠杆划转、秒合约、客服较完整；预测重放、新币异参重放、贷款异参重放仍有缺口。 |
| 事务/行锁 | 单模块资金路径普遍使用事务与 `FOR UPDATE`；风险集中在 Redis/MySQL、HTTP/DB、事件/DB 的跨边界。 |
| 余额锁定 | 充值/提现、现货、杠杆、预测、新币释放均先锁钱包；全仓转出缺少风险资本保留。 |
| 账本 | 用户余额变更大多有流水与 after 快照；缺少平台总账、统一业务事务号及内部资产守恒。 |
| 状态机 | 提现、订单、解禁、客服状态有终态守卫；预测批结算和外部广播不确定状态需要补充。 |
| 补偿 | inbox 有数据库 retry 设计，充值异常有 manual review；但部署未启用 inbox，返佣退款、逐仓坏账、网关不确定广播缺补偿。 |
| Outbox/MQ | `user.created` 真正使用事务 outbox；其他财务事件主要绕过 outbox，且 MQ 拓扑/confirm 不完整。 |
| 私有 WS/读模型 | MySQL/REST 是可恢复事实源；WS 无持久化、无全局序列、跨实例不可达，客户端必须显式快照对齐。 |

### Files Found

- `src/modules/auth/application.rs` — 注册事务、推荐绑定与 user-created outbox。
- `src/modules/events/{infrastructure.rs,service/rabbitmq.rs,service/production_dispatch.rs,service/websocket.rs}` — outbox/inbox、RabbitMQ 与进程内 WS。
- `src/workers/{event_inbox.rs,wallet_chain.rs,margin_interest.rs,margin_liquidation.rs,seconds_contract_settlement.rs,agent_commission_settlement.rs}` — 核心异步状态推进与补偿边界。
- `src/modules/wallet/{application.rs,infrastructure/deposits.rs,infrastructure/withdrawals.rs}` — 充值/提现资金状态机。
- `src/modules/spot/application/{settlement.rs,triggering.rs}` 与 `infrastructure/wallet_accounts.rs` — 现货结算、系统流动性与钱包锁。
- `src/modules/margin/application/{account_settings.rs,open_position.rs,queries.rs}`、`infrastructure/{transfers.rs,settlement.rs}` — 划转、仓位与风险资金。
- `src/modules/seconds_contract/application.rs` — 秒合约开仓/人工结算。
- `src/modules/convert/{application.rs,infrastructure.rs}` — 闪兑报价缓存与双钱包结算。
- `src/modules/loan/application.rs`、`src/workers/loan_overdue.rs` — 借贷生命周期与逾期扫描。
- `src/modules/earn/{application.rs,infrastructure.rs,redemption.rs}` — 理财申购、费用快照、赎回。
- `src/modules/prediction/infrastructure.rs` — 预测报价、订单、退款/派奖与批结算。
- `src/modules/new_coin/{application.rs,infrastructure.rs,repository.rs}` — 新币申购、购买、锁仓、缴费与释放。
- `src/modules/agent/infrastructure.rs`、`src/modules/admin/application/agents.rs` — 分层返佣生成和钱包打款。
- `src/modules/support/{application.rs,infrastructure.rs}` — 客服会话/消息/游标/改派。
- `migrations/0003_assets_wallet_ledger_locks.sql` — 钱包、流水、锁仓基础模型。
- `migrations/0006_new_coin_lifecycle.sql`、`0063_asset_deposit_withdraw_fee_settings.sql`、`0071_user_loans.sql`、`0087_p0_financial_safety.sql`、`0105_agent_routed_online_support.sql` — 相关持久化合同。

### Code Patterns

- **良好模式**：业务行/幂等占位 → 稳定顺序 `FOR UPDATE` 钱包锁 → 余额与流水 → 业务终态 → commit；现货 `settle_spot_fill`、提现 `confirm_withdrawal_in_tx`、客服 `append_message_in_tx` 是代表。
- **高风险模式**：在事务外先读 Redis/HTTP 事实，事务内不重检权威版本；见闪兑确认、预测异步同步、提现广播。
- **高风险模式**：把当前观测值用于历史事件时点结算；见秒合约和杠杆利率。
- **高风险模式**：只记录用户钱包一腿而缺系统账户对手腿；见贷款、理财、赔付、返佣等内部产品。
- **事件模式**：先提交 MySQL 后调用进程内 `hub.publish`，保证“不推送回滚事实”，但不保证送达、顺序或重放。

### Related Specs

- `.trellis/spec/backend/wallet-amount-precision.md`
- `.trellis/spec/backend/deposit-addresses.md`
- `.trellis/spec/backend/spot-orders.md`
- `.trellis/spec/backend/order-identifiers.md`
- `.trellis/spec/backend/margin-trading-actions.md`
- `.trellis/spec/backend/seconds-contracts.md`
- `.trellis/spec/backend/loan-products.md`
- `.trellis/spec/backend/earn-products.md`
- `.trellis/spec/backend/prediction-markets.md`
- `.trellis/spec/backend/new-coin-mobile-contract.md`
- `.trellis/spec/backend/agent-hierarchy.md`
- `.trellis/spec/backend/realtime-websockets.md`
- `.trellis/spec/backend/online-support.md`
- `.trellis/spec/backend/user-authentication.md`
- `.trellis/spec/backend/auth-sessions.md`
- `.trellis/spec/backend/database-guidelines.md`

### External References

- 无。本轮为仓库内部静态审计，没有使用网络资料或对外部服务作运行时验证；RabbitMQ、Redis、MySQL 与链网关结论均以仓库内适配器合同和调用代码为依据。

## Caveats / Not Found

- 未连接生产 MySQL/Redis/RabbitMQ、链网关或 Polymarket，未检查真实部署变量、队列绑定、数据规模、历史坏账和运行告警；结论是代码/迁移/样例配置层面的确定性审计。
- 按用户要求未执行写入性集成测试，也未修改 research 目录外任何文件。
- 仓库内未发现：新币供给扣减、解禁费钱包扣款、贷款 LTV/清算、充值费净额入账、逐仓坏账、返佣反冲、内部产品平台总账对手账户、财务私有事件 outbox。若这些能力存在于仓库外，需用接口、幂等合同和对账结果补证。
- 在线客服 WS 可丢是已写入项目规格的设计选择，因此消息正确性本身未列缺陷；P1-11 针对的是多实例财务读模型若没有同等明确的 REST/版本恢复机制。
- 现货当前实现可能被产品定义为“系统做市成交”而非中央订单簿；若如此，P1-03 应改名为产品/文案一致性问题，但系统库存依赖和无用户自动撮合这一实现事实不变。
