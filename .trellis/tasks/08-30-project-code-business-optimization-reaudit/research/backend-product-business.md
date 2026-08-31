# Research: Rust 后端产品与业务流程当前复审

- Query: 复审当前 Rust 秒合约、借贷、理财、预测、新币、代理返佣、在线客服、合成行情与后台配置链路，按“请求 → 权威规则 → 钱包/账本 → worker/outbox → 读模型/客户端”重证现存缺口，并与 2026-08-24 审计比较。
- Scope: internal
- Date: 2026-08-30
- Baseline: 当前任务基线记录为 `main@fac1def`、migration 至 `0117_margin_partial_close.sql`；见 `.trellis/tasks/08-30-project-code-business-optimization-reaudit/research/baseline-and-p0-verification.md:5-20`。
- Runtime-evidence flag: `静态=是` 表示当前源码已形成可达路径；`测试=有/缺` 表示仓库内是否发现对应回归；`运行时=否` 表示未连接生产 MySQL/Redis/Mongo/RabbitMQ、未取得线上指标或数据样本。
- Severity: P0 仅用于可直接造成资金、结算/价格时点或不可恢复账务正确性风险；P1 用于显著业务闭环、并发恢复或运营可用性风险。

## Findings

### 结论摘要

本轮保留 **3 条 P0、8 条 P1**。历史“新币客户端定价/超发、假缴解禁费、借贷无 LTV、秒合约按处理时价、预测结束后仍下注”等结论在当前代码中已有实质修复，不重复列为未修复项。当前最高风险集中在：

1. 合成行情可用于秒合约开仓，却没有写入秒合约结算依赖的事件时间历史表；
2. 理财直接允许超过资产精度的本金及计算结果进入钱包；
3. 借贷到期后只有 `overdue` 标记，没有按期限回收抵押、信用违约或核销闭环；
4. 平台分录、返佣反冲、批量结算、持久重试和运营接管仍不完整。

| ID | 优先级 | 领域 | 当前结论 | Runtime-evidence flag |
| --- | --- | --- | --- | --- |
| BPB-P0-01 | P0 | 秒合约 × 合成行情 | 合成 ticker 不归档，相关秒合约可永久待结算且本金已扣 | 静态=是；测试=缺交叉测试；运行时=否 |
| BPB-P0-02 | P0 | 理财 | 申购与赎回绕过 `assets.precision_scale`，可写入业务非法金额 | 静态=是；测试=缺资产精度测试；运行时=否 |
| BPB-P0-03 | P0 | 借贷 | 到期只标逾期；健康清算仍只看 LTV，信用贷/低 LTV 抵押贷可无限逾期 | 静态=是；测试=缺到期回收测试；运行时=否 |
| BPB-P1-01 | P1 | 跨产品账务 | platform journal 仅覆盖新币解禁费和借贷，理财、新币销售与返佣发放缺平台对手腿/对账 | 静态=是；测试=部分；运行时=否 |
| BPB-P1-02 | P1 | 代理返佣 × 预测 | 无效市场退款不拒绝或反冲来源佣金，worker 也不复核来源终态 | 静态=是；测试=缺退款反冲；运行时=否 |
| BPB-P1-03 | P1 | 预测运营控制 | 后台隐藏可被下一轮同步覆盖；市场更新/结算缺少 actor、reason、revision 和同事务审计 | 静态=是；测试=缺同步后保持隐藏；运行时=否 |
| BPB-P1-04 | P1 | 预测结算 | 单事务锁定并结算全部订单，且终态/下单幂等重放不比较原始意图 | 静态=是；测试=缺大批量/异参重放；运行时=否 |
| BPB-P1-05 | P1 | workers | 理财、返佣、解禁缺持久 retry/next-attempt/dead-letter，固定头部候选可饿死后项 | 静态=是；测试=仅小规模继续语义；运行时=否 |
| BPB-P1-06 | P1 | 在线客服 | 代理停用不立即重算既有会话归属，未分配队列可暂时看不到已失去服务能力的会话 | 静态=是；测试=缺停用联动；运行时=否 |
| BPB-P1-07 | P1 | 合成行情恢复 | 恢复任务在 HTTP 内同步执行，重认领无 owner/fence/heartbeat，崩溃恢复依赖原 preview token | 静态=是；测试=缺进程崩溃/超时竞争；运行时=否 |
| BPB-P1-08 | P1 | 借贷限额/客户端 | 只有单笔产品上下限，无用户总敞口；健康读模型未被 PC/mobile 使用，也无补抵押/部分还款入口 | 静态=是；测试=缺并发总敞口；运行时=否 |

### 当前端到端保护矩阵

| 领域 | 请求与鉴权 | 服务端权威规则 / 事务与账本 | 异步、读模型与客户端 | 当前评价 |
| --- | --- | --- | --- | --- |
| 秒合约 | `src/modules/seconds_contract/application.rs::open_order`（274-417）从会话用户开仓 | 锁产品/周期，Redis 取入场价，订单、available 扣款、wallet ledger、佣金同事务 | `src/workers/seconds_contract_settlement.rs::settle_order_by_id`（258-346）依赖 MySQL 历史 tick；mobile `mobile/src/api/seconds.ts:26-69` 读产品/订单并开仓 | 外部 feed 事件时间结算已修；合成 tick 生产链断裂 |
| 借贷 | `src/modules/loan/routes.rs::user_routes`（50-62）与 admin 审批路由（64-84） | `create_loan_order_use_case`（295-506）和 `approve_loan_order_use_case`（850-1010）快照条款、oracle/LTV、钱包与平台分录 | overdue/health worker 推进状态；PC `pc/src/api/loan.ts:147-209`、mobile `mobile/src/api/loan.ts:55-90` 仅创建/取消/还款 | LTV 修复成立；到期、总敞口及用户自救闭环缺失 |
| 理财 | `subscribe_earn_product_with_events`（`src/modules/earn/application.rs:459-489`） | `subscribe_in_tx`（568-654）与 `redeem_subscription_in_tx`（656-713）原子写订阅、钱包、流水 | `src/workers/earn_auto_redemption.rs:131-204` 扫描到期项；mobile `mobile/src/api/earn.ts:35-68` | 幂等/费用快照较强；精度、结算证据、持久重试不足 |
| 预测 | `src/modules/prediction/routes.rs:25-72` 分用户 quote/order 与 admin 配置/结算 | `create_order_in_tx`（`infrastructure.rs:305-436`）消费报价并冻结/扣费；`settle_market_in_tx`（466-599）结算钱包 | `run_sync_loop`（`application.rs:36-69`）、本地关盘 worker；PC/mobile 有完整市场、报价和订单客户端 | 本地关盘已修；运营覆盖、批量结算和返佣反冲仍有缺口 |
| 新币 | `src/modules/new_coin/application.rs` 解析用户请求，仓储事务重读项目规则 | `create_subscription_order`（`infrastructure.rs:379-529`）和 `create_purchase_order`（531-686）权威定价、供给预留、扣款/分配原子提交 | unlock scanner 释放；mobile `mobile/src/api/newCoin.ts:79-180` 覆盖项目、申购、购买、缴费、释放 | 历史两项 P0 已修；平台销售对手腿与 worker 恢复仍缺 |
| 代理佣金 | 来源业务在同一事务调用 `insert_agent_business_commission_in_tx`（`src/modules/agent/infrastructure.rs:27-108`） | 差额佣金按资产精度快照为 pending；结算事务给代理钱包入账 | `src/workers/agent_commission_settlement.rs:60-177` 自动结算；agent/admin 有列表读模型 | 生成原子且幂等；来源终态、反冲、持久 retry 缺失 |
| 客服 | user/agent/admin 三套鉴权路由；身份来自 token | 消息不可变、精确 owner、游标和改派在 MySQL 事务内 | REST 权威，进程 WS 仅刷新提示；mobile `support.ts:66-105`、web `OnlineSupportWorkbench.tsx` | 基础模型是强项；代理状态切换未联动既有分配 |
| 合成行情 | admin 创建版本化策略并显式预览/执行恢复 | realtime 有 strategy lease/version；Mongo/Redis 无跨库事务，以 upsert/CAS 收敛 | realtime worker 常驻；手动恢复在 HTTP 请求内执行，任务列表只读 | 实时租约较强；结算 tick 归档与恢复任务接管不足 |

---

### BPB-P0-01：合成行情可开秒合约，但不产生结算历史，订单可永久待结算

- **当前证据 / 符号**：
  - `src/modules/seconds_contract/infrastructure.rs::lock_active_product`（651-733）只要求产品、交易对和资产 active，没有限制 `trading_pairs.market_type` 或要求可归档价格源。
  - `src/modules/seconds_contract/application.rs::open_order`（274-417）从 Redis 取得入场价后即原子扣减本金；因此 strategy/internal pair 同样能形成已扣款的 `opened` 订单。
  - `src/modules/seconds_contract/infrastructure.rs::select_settlement_price_snapshot`（943-977）只从 `market_price_ticks` 选 `[expires_at, expires_at+5s)` 第一条历史价格；没有历史行必须返回 `None`。
  - 外部 feed 由 `src/workers/market_feed.rs::GenerationBoundMarketIngestionSink::archive_ticker`（138-185）插入 `market_price_ticks`。
  - 合成 worker 调用 `src/modules/market/infrastructure/adapters/ingestion.rs::ingest_and_publish_synthetic_ticker`（149-174）；该函数只做 Redis CAS、触发现货/杠杆和 WS，不插 MySQL。调用点见 `src/workers/synthetic_market.rs::process_leased_strategy`（274-330）。
  - `src/workers/seconds_contract_settlement.rs::settle_order_by_id`（287-293）无快照时保持 pending；`fetch_due_orders`/`reschedule_settlement_attempt`（221-255）只每 60 秒重试，没有退款或人工终态。
- **影响**：后台一旦把秒合约产品配置到 strategy/internal pair，用户本金已从 available 扣除，但该订单没有合法结算证据，可能无限保持 opened；不能用 Redis 最新价补救，否则破坏事件时间结算。
- **建议**：把“合成 tick 被 Redis CAS 接受”与 append-only `market_price_ticks(source='strategy')` 归档纳入同一个受 generation/version fence 保护的摄取入口；在修复前，产品保存和开仓应拒绝没有历史归档能力的 pair。增加最大待结算年龄、运营异常队列和确定性的原路退款状态机。
- **兼容**：新 writer 可增量上线；历史缺失价格不得回填臆造。现有无证据订单应进入 `manual_review`，只允许按本金/明确政策退款并写审计、钱包流水和平台分录。
- **测试**：strategy 产品开仓→合成 tick 归档→事件时点结算；Mongo/Redis/MySQL 任一步失败；重复 tick/worker 重启/旧 generation；窗口永远无 tick 的超时退款与结算竞争。
- **工作量 / 依赖**：L；依赖统一 market ingestion 端口、migration（恢复状态/attempt 元数据）、运营异常列表。
- **Runtime-evidence flag**：静态=是；测试=现有秒合约与合成行情测试各自存在但缺交叉链路；运行时=否，需统计 strategy pair 秒合约产品和超龄 opened 订单。

### BPB-P0-02：理财绕过资产精度，申购与收益可把非法小数写入钱包

- **当前证据 / 符号**：
  - 全局合同 `.trellis/spec/backend/wallet-amount-precision.md:12-22` 明确 `assets.precision_scale` 是业务精度，用户金额和计算金额写钱包前必须量化。
  - `src/modules/earn/service.rs::validate_amount`（800-817）明确只校验数据库 18 位；注释（801-803）承认小数位较少的资产也会接受超精度申购。
  - `src/modules/earn/application.rs::subscribe_in_tx`（596-649）锁产品后不读取 asset precision，直接扣用户 available；`src/modules/earn/infrastructure.rs::debit_wallet_for_subscription_in_tx`（682-716）原值写 wallet/ledger。
  - 赎回计算在 `src/modules/earn/redemption.rs::scaled_amount`（约 153）固定为 18 位；`credit_wallet_for_redemption_in_tx`（`infrastructure.rs:743-775`）未经资产精度量化直接加 available。
- **影响**：对 precision 小于 18 的资产，用户可提交超精度本金，且 APR/费用计算通常产生更多小数位；`wallet_accounts` 与 `wallet_ledger` 虽可存储，却违反资产业务合同并污染提现、兑换、对账及客户端展示。
- **建议**：产品锁后同时锁/读 active asset metadata；申购金额按“尾零不计额外精度”拒绝超精度；所有本金、毛收益、各费用、净收益和 redeem amount 由共享 helper 向零截断到资产精度，且账户 after 与流水 after 完全一致。
- **兼容**：先以只读 reconciliation 列出历史超精度余额/订阅；不要静默修改用户余额。按财务批准的 rounding-adjustment 交易逐笔修正，并保留原值、调整值和原因。
- **测试**：precision=2/8 的超精度申购零副作用；APR 产生长小数时手工/自动赎回得到同一量化结果；幂等重放、边界尾零、费用总和及 wallet/ledger snapshot 一致。
- **工作量 / 依赖**：M；依赖共享 amount helper 和历史精度对账脚本/运营政策。
- **Runtime-evidence flag**：静态=是；测试=未发现 earn 资产精度回归；运行时=否，需查询 active earn 资产精度与超精度余额。

### BPB-P0-03：借贷到期状态机没有本金回收、违约或核销闭环

- **当前证据 / 符号**：
  - `src/workers/loan_overdue.rs::run_once_with_dependencies`（57-83）只把到期订单逐笔推进；注释明确不计罚息、不改钱包、不写账本或事件。
  - `mark_order_overdue`（167-201）唯一动作是 `disbursed -> overdue` 和写 `overdue_at`。
  - `src/modules/loan/liquidation.rs::fetch_loan_liquidation_candidates`（116-143）只扫描抵押贷；`liquidate_loan_order_if_required`（146-296）在 `ltv < liquidation_ltv` 时返回 `NotRequired`（219-223），即使订单已经 overdue。
  - `src/modules/loan/service.rs::calculate_interest_amount`（37-78）actual-days 利息最多计到 `term_days`（62-66），逾期后不继续累计。
  - `migrations/0113_loan_liquidation_accounting.sql:5-12` 终态只有 repaid/liquidated，没有 collection/default/written_off/restructured；`src/modules/loan/routes.rs:50-84,253-331` 也没有到期处置、核销或重组运营入口。
- **影响**：稳定币抵押贷只要 LTV 未越线即可无限逾期；信用贷没有任何自动回收/违约/坏账终态。平台本金已放出，却没有代码内可证明的到期处置和财务闭环，利息还在期限后停止增长。
- **建议**：把到期政策快照到订单（grace period、逾期利率/费用、抵押处置规则、信用贷 collection/default/write-off）；到期后按独立于风险 LTV 的期限状态机执行。所有处置写 immutable settlement、平台分录、审计和幂等 action key；提供管理员待办、重试、人工核销/重组入口。
- **兼容**：新字段 nullable + 新订单必填；历史 overdue 统一进入 `manual_review`，不得追溯发明罚息。低 LTV 抵押与信用贷由运营确认政策后再迁移终态。
- **测试**：低波动抵押贷到期、信用贷到期、grace 边界、还款与到期处置竞争、worker 重启、部分失败、坏账分录平衡和历史订单 fail-closed。
- **工作量 / 依赖**：XL；依赖产品/法务/财务违约政策、migration、平台 journal、运营工作台和通知能力。
- **Runtime-evidence flag**：静态=是；测试=现有 `tests/loan_risk.rs` 覆盖 LTV/清算但缺期限处置；运行时=否，需核对 overdue 账龄、类型和本金余额。

### BPB-P1-01：平台总账覆盖仍是局部，多个产品只有用户钱包单边记录

- **当前证据 / 符号**：
  - `migrations/0110_platform_financial_journal.sql:1-17` 建立平台 journal，但 context 注释只列 `new_coin_unlock_fee/loan_disbursement/loan_repayment/loan_liquidation`。
  - 新币解禁费已在 `src/modules/new_coin/infrastructure/unlock.rs:214-262` 同事务写用户扣款与 `user_unlock_fee_expense/platform_unlock_fee_revenue` 双腿；借贷放款/还款/清算也使用平台 journal，这是当前强项。
  - 理财申购/赎回仅写 `earn_subscribe`/`earn_redeem` 用户流水（`src/modules/earn/infrastructure.rs:682-716,743-775`），本金负债、收益费用和平台支出/收入没有不可变对手腿。
  - 新币申购/购买在 `src/modules/new_coin/infrastructure.rs:476-518,633-675` 扣 quote 钱包并发行 base 资产，但销售所得/发行义务未写平台 journal。
  - 返佣发放在 `src/modules/admin/application/agents.rs::settle_agent_commission_payout_in_tx`（586-613）只给代理钱包加 available/ledger。
- **影响**：用户子账内部能回放余额，但平台无法按业务解释理财负债和费用、新币销售所得/发行义务、佣金应付与实际支付；储备、收入、坏账和业务利润无法由同一权威分录对账。
- **建议**：先定义会计科目和每域 posting template，再让业务事务同时写用户钱包腿与平台腿；commission 在来源交易时确认 payable、发放时清 payable；每日 reconciliation 验证业务快照、钱包流水和 platform journal。
- **兼容**：先 shadow-write 和离线比较，不立即改现有钱包权威性；历史记录只能从订单快照/钱包流水可证明地回填，无法拆出的费用标为 legacy-unknown，禁止臆造。
- **测试**：每 transaction key 唯一、每资产模板平衡、删除/重复一腿告警、业务回滚不留平台腿、历史回填可重跑、按日储备等式可解释。
- **工作量 / 依赖**：XL；依赖财务科目、统一 WalletPostingPort、reconciliation job、运营差异队列。
- **Runtime-evidence flag**：静态=是；测试=loan/unlock 有 journal 测试，其他域缺；运行时=否。

### BPB-P1-02：来源退款/作废不反冲代理佣金

- **当前证据 / 符号**：
  - 预测下单在同事务创建 pending 佣金：`src/modules/prediction/infrastructure.rs::create_order_in_tx`（422-434）。
  - 无效市场退款在 `settle_market_in_tx`（511-539）退 stake/fee 并把订单置 refunded，没有查询或迁移佣金。
  - 自动 worker 仅按佣金 `status='pending'` 和账龄扫描：`src/workers/agent_commission_settlement.rs::fetch_pending_commissions`（134-155）；结算入口 `apply_admin_agent_commission_status`（`src/modules/admin/application/agents.rs:541-583`）只要求 commission 为 pending，不复核 source order 状态。
  - 管理状态只允许 pending→settled/rejected（同文件 554-565），schema/代码没有 reversed/receivable 状态。
- **影响**：预测订单先生成返佣，之后市场 invalid 并退款时，pending 佣金仍可发放；若已 settled，用户获得退款而代理保留佣金，平台形成不可解释损失。相同模型也应核对其他可退款/撤销来源。
- **建议**：佣金增加 `eligible_at/source_status/reversed_at/reversal_of`；worker 在锁 commission 后以统一 source adapter 锁并验证终态。退款事务对 pending 佣金拒绝，对 settled 佣金写反向应收/平台分录；退款与佣金结算使用固定锁序或同一 source settlement coordinator。
- **兼容**：先停止自动发放来源不可验证的 legacy 记录并进入人工队列；已发放历史按 source_id 对账，反冲采用新交易而非篡改旧钱包流水。
- **测试**：退款发生在发放前/后、退款与发放并发、重复退款/反冲、多个层级差额佣金、无推荐用户以及 worker 重启。
- **工作量 / 依赖**：L；依赖佣金状态 migration、各业务 source adapter、platform journal、运营追收政策。
- **Runtime-evidence flag**：静态=是；测试=现有佣金原子/幂等测试存在但缺 reversal；运行时=否。

### BPB-P1-03：预测后台“隐藏/结算”控制不是持久权威，且资金操作缺审计治理

- **当前证据 / 符号**：
  - `src/modules/prediction/application.rs::update_admin_market`（459-507）接受 display/settlement/asset/cap/fee 覆盖，但 `_auth` 不解析 admin_id，请求 DTO `presentation.rs:95-102` 没有 reason/revision；更新与回读不在同一事务（466-467）。
  - `src/modules/prediction/infrastructure.rs::sync_polymarket_markets_inner`（701-795）upsert 时直接重写 `display_status`（748-753）；代码注释（707-708）明确下一轮同步会覆盖后台手工展示状态。
  - `settle_admin_market`（`application.rs:642-669`）同样忽略具体 admin 身份，请求仅有 result/refund policy（`presentation.rs:118-122`），没有 reason 或 admin audit。
  - 相比之下，全局设置和资产配置在 `application.rs:85-158,273-317` 有 revision、reason、锁和同事务 before/after 审计。
- **影响**：运营为风险事件手工隐藏市场后，下一轮上游同步可重新变为 active 并恢复 quote/order；人工派奖/退款无法从业务审计中证明操作者和原因。同步本身还无多实例锁（`application.rs:47-69` 明示部署侧单实例），会放大覆盖竞态。
- **建议**：拆分 `source_status`、`operator_visibility_override/risk_hold` 和 effective status；sync 永不清除 operator hold。市场更新和结算都要求 admin subject、reason、expected revision，在锁市场、资金结算和 audit 同一事务提交；提供显式解除 hold 动作。
- **兼容**：迁移时对 `hidden` 且无本地关盘证据的市场保守映射为 operator hold/manual review；不能根据当前上游 active 自动解除。保持现有 JSON 字段为 effective display 的兼容 façade。
- **测试**：后台 hide→上游仍 active→多次 sync 后仍拒绝 quote；旧 revision 409；结算写 actor/reason；提前/重复/相反结果结算冲突；双实例 sync 顺序颠倒。
- **工作量 / 依赖**：L；依赖 status migration、admin API/UI revision、审计与 sync lease/fence。
- **Runtime-evidence flag**：静态=是；测试=缺 hide-vs-sync/audit；运行时=否，需确认生产是否把 display_status 当风险暂停闸门及实例数。

### BPB-P1-04：预测市场把全部订单放进一个结算事务，重放也不核对原始意图

- **当前证据 / 符号**：
  - `src/modules/prediction/infrastructure.rs::settle_market_in_tx`（466-599）锁市场后以无 LIMIT 的 `SELECT ... WHERE market_id=? AND status='open' ... FOR UPDATE` 一次取出全部订单（494-504），逐单锁钱包/写流水，最后统一 commit（593）。
  - 任意一单钱包/数据异常会回滚整场；没有 settlement job、batch cursor、claim、逐单终态或恢复进度。
  - 市场已 settled/refunded 时直接 `changed=false`，不比较本次 result/refund policy（474-478）。
  - 下单幂等快速返回也不比较本次 `quote_id`：`create_order_in_tx`（286-319）；应用层注释在 `application.rs:524-526` 明确这一点。
- **影响**：大市场结算持锁时间和事务体量随订单数无界增长；一个坏钱包可阻塞全场派奖/退款。错误复用幂等键或相反结算命令会被静默解释成旧成功，削弱客户端和运营对意图的确认。
- **建议**：先在短事务固化 result/refund policy/actor/reason 和 `settling` job，再按 ID 分批 claim 订单、逐单幂等结算，最后仅在全部完成后 finalize market；持久化 cursor、attempt、error 和 reconciliation counts。所有命令保存 request fingerprint，异参重放返回 conflict。
- **兼容**：保留现有 settle endpoint，内部改为创建/查询 job；小市场可同步等待有限时间，旧客户端仍收到原响应结构。历史终态缺 fingerprint 时只允许同结果保守重放。
- **测试**：十万单分批、单个 poison wallet、任意批次崩溃重启、多 worker、结算与最后一笔下单竞争、相反结果/策略重放、每单与市场汇总对账。
- **工作量 / 依赖**：XL；依赖 settlement job migration、worker、平台 journal、admin 进度 UI。
- **Runtime-evidence flag**：静态=是；测试=缺大批量/崩溃恢复；运行时=否，需生产订单量分布和事务耗时。

### BPB-P1-05：多个资金 worker 的失败重试只在内存或固定前缀中处理

- **当前证据 / 符号**：
  - 理财 worker 会逐项继续，但 `fetch_due_subscriptions`（`src/workers/earn_auto_redemption.rs:186-203`）每轮始终取最早至多 500 条，没有 attempt/next_attempt/dead-letter；500 条永久失败可持续占满前缀。
  - 返佣 worker 的失败 guard 是进程内 `HashSet`，达到 10,000 整体清空且重启丢失：`src/workers/agent_commission_settlement.rs:32-52,106-130`；候选始终按最早 ID 限 1,000（134-155）。
  - 解禁 scanner 在循环中使用 `release_due_unlock_by_id(...).await?`：`src/workers/unlock_scanner.rs:170-195`，单条错误立即终止整轮；候选仍按最早到期限制 100（219-249）。
- **影响**：少量错误目前能继续或被内存跳过，但固定上限的坏前缀、worker 重启或单个解禁异常仍能让后续到期资金长期不处理；运营没有持久失败原因、下次重试时间、死信或重排动作。
- **建议**：每类任务增加持久 `attempt_count/next_attempt_at/last_error/lease_owner/lease_until`，用 `FOR UPDATE SKIP LOCKED` 领取；指数退避、最大次数、dead-letter、管理员重排和 backlog age 指标统一实现。业务终态继续承担资金幂等。
- **兼容**：字段 additive，现有 pending 行默认 attempt=0；先 shadow 记录失败再启用排程。内存 guard 可在持久策略稳定后移除。
- **测试**：坏前缀超过 scan limit、单项 SQL 错误、进程重启、多实例竞争、租约过期、dead-letter/requeue、后续健康项在 SLA 内完成。
- **工作量 / 依赖**：L；依赖通用 worker lease/retry 组件、migration、admin 运维页、指标告警。
- **Runtime-evidence flag**：静态=是；测试=earn 仅覆盖小坏前缀继续，缺持久/多实例；运行时=否。

### BPB-P1-06：代理停用不立即解除客服会话，未分配队列可能漏单

- **当前证据 / 符号**：
  - 权威 owner 解析会拒绝自身或祖先非 active：`src/modules/support/infrastructure.rs::resolve_active_support_agent_in_tx`（243-283）。
  - 但 `src/modules/admin/application/agents.rs::update_admin_agent_status`（124-157）只更新 agents、门户账号和 audit，没有调用客服归属同步。
  - 现有会话只在用户下一次 get/list/send/read/status 时通过 `synchronize_existing_user_conversation`（`src/modules/support/application.rs:63-77,203-339`）惰性同步。
  - admin queue 的 `list_admin_support_conversations`（521-557）直接读当前 snapshot；`unassigned=true` 最终只筛 `assigned_agent_id IS NULL`（`src/modules/support/infrastructure.rs:91-99`），不会主动把 inactive owner 清空。
- **影响**：代理或其祖先停用后，代理认证已失效，但既有 open 会话仍显示 assigned；它既不在代理可处理队列，也不在管理员未分配队列，直到用户再次触发同步，造成客服积压和 SLA 漏报。
- **建议**：代理状态迁移事务内，set-based 找出该代理及受影响后代 owner 的会话，立即置 NULL 或转入明确 fallback queue，清 staff cursor 并记录 affected count/audit；定义恢复 active 时是保持未分配还是显式重派。
- **兼容**：assigned_agent_id 已 nullable；上线 migration/job 先扫描 inactive ancestry 的陈旧分配并 fail-safe 清空，保留 referral 归属不变。
- **测试**：停用直属代理、停用祖先、管理员 unassigned queue 立即可见、旧代理访问拒绝、用户不发新消息也能接管、恢复/改派政策。
- **工作量 / 依赖**：M；依赖 agent status 事务、support set-based helper、运营通知/SLA 指标。
- **Runtime-evidence flag**：静态=是；测试=现有 exact-owner/reassignment 测试强，但缺 status-transition；运行时=否。

### BPB-P1-07：手动 K 线恢复没有独立 worker 与 fenced lease，超时重领可能并发执行

- **当前证据 / 符号**：
  - `src/modules/admin/application/market.rs::execute_admin_market_strategy_recovery`（403-575）创建/认领任务后，在同一 HTTP 请求内直接 await Mongo 恢复（511-517），然后才写 completed/failed。
  - 任务最多展开 10,080 根 1m：`recovery_open_times`（692-713）和 `src/workers/kline_recovery.rs:32`。
  - `claim_market_strategy_recovery_job`（`src/modules/admin/infrastructure/market.rs:829-897`）只比较 status/started_at；没有 lease owner、fencing token 或 heartbeat，15 分钟后可被另一请求重领。
  - terminal update 只要求 `status='running'`（同文件 931-989），旧执行者没有 claim token 条件，可能在新执行者重领后先行完成。
  - job 列表不暴露 token/hash且只读（`market.rs:715-741`、`infrastructure/market.rs:751-779`）；重试入口按 preview token hash 找任务（`application/market.rs:420-438`）。`src/main.rs:123-157` 只启动 realtime synthetic worker，没有 recovery-job worker。
- **影响**：请求超时/进程崩溃会留下 running；页面刷新后若原 preview token 不再可用，运营无法按 job id 恢复。合法长任务超过 15 分钟时可被并发重领，旧/新执行者同时写 Mongo，终态由竞争者决定；幂等 upsert 防重复文档，但不能保证任务所有权和准确进度。
- **建议**：HTTP 仅验证 token、创建 job 并返回；独立 worker 以 owner+lease+fencing token 领取，周期 heartbeat，所有进度/终态更新带 fence。提供 job-id retry/cancel/resume（RBAC+reason），preview token 只用于首次创建。
- **兼容**：Mongo `(interval,open_time)` upsert 可继续作为数据幂等；将现有 pending/running 迁移到新 lease 模型，超时旧任务先人工核对实际 Mongo 根数再重排。
- **测试**：HTTP 断开、进程 kill、UI 丢 token、任务运行超过 15 分钟、多 worker 重领、旧 fence terminal write 被拒、部分 Mongo 成功后的精确续跑。
- **工作量 / 依赖**：L；依赖 migration、worker role/supervision、admin job actions、Mongo reconciliation。
- **Runtime-evidence flag**：静态=是；测试=当前测试覆盖幂等 upsert/状态但缺真实崩溃与 fence；运行时=否。

### BPB-P1-08：借贷只有单笔限额，缺用户总敞口与可执行健康动作

- **当前证据 / 符号**：
  - `migrations/0071_user_loans.sql:1-25` 的 `min_amount/max_amount` 是产品单笔字段；订单表（28-77）没有 user/product/asset exposure 或 credit limit。
  - `create_loan_order_use_case`（`src/modules/loan/application.rs:295-506`）只校验当前 amount 在产品区间；不同幂等键可创建多笔 pending。`approve_loan_order_use_case`（850-1010）审批时也没有汇总 outstanding exposure。
  - 健康接口存在：`src/modules/loan/routes.rs:54-62` 的 `GET /loan/orders/:id/health`，但 PC `pc/src/api/loan.ts:147-209` 与 mobile `mobile/src/api/loan.ts:55-90` 只包含创建、列表、取消、全额还款，没有调用 health。
  - 用户路由也没有补充抵押、部分还款或风险通知动作（`routes.rs:50-62`）。
- **影响**：`max_amount` 可通过多订单规避；尤其信用贷完全依赖人工逐笔判断，代码没有并发安全的用户总敞口闸门。抵押贷即使进入 maintenance/margin-call，客户端也没有产品内自救路径，只能全额还款或等待清算。
- **建议**：明确并快照 per-user/product/asset credit/exposure limit；创建和审批时锁定统一 exposure row，终态原子释放；管理员显示 pending+disbursed+overdue 总敞口。把 health 接入 PC/mobile，并提供精度安全、幂等的 top-up collateral/partial repay 与风险通知。
- **兼容**：先新增 nullable limit 并对历史产品采用 manual-review；现有超限用户不自动取消，冻结新增审批直至运营处置。客户端保留旧全额 repay。
- **测试**：并发创建/审批跨限额、不同产品/资产隔离、取消/拒绝/还款/清算释放 exposure、健康阈值 UI、补抵押与清算竞争、部分还款账务。
- **工作量 / 依赖**：L/XL；依赖信用政策、exposure migration、loan state machine、客户端与通知。
- **Runtime-evidence flag**：静态=是；测试=现有单笔/LTV 测试有，聚合限额与客户端健康动作缺；运行时=否。产品若明确允许无限多笔，需将“总敞口缺失”降为已接受政策，但仍需平台级风险上限。

## Strengths

1. **秒合约外部行情的历史 P0 已实质修复**：`migrations/0114_event_time_price_snapshots.sql:1-30` 建 append-only tick 与结算证据列；人工/worker 都按事件窗口选快照，窗口缺失不读 Redis 最新价。当前问题是 synthetic producer 未接入，而不是旧的“所有秒合约都按处理时价”。
2. **借贷申请、审批和 LTV 清算有完整事务骨架**：`create_loan_order_use_case`（295-506）在写入前锁产品/资产、校验 KYC/精度/oracle/LTV并原子冻结抵押；审批再次估值；`src/modules/loan/oracle.rs:128-151` 同时拒绝陈旧与超过 5 秒未来偏差的 ticker；清算用 SKIP LOCKED、唯一记录和平台坏账分录。
3. **新币权威定价与供给防超发成立**：`create_subscription_order`（379-529）和 `create_purchase_order`（531-686）事务内重读项目/交易对、验证资产精度、用 issue price 计算 authoritative quote，并通过 conditional reserve/finalize 串行供给。
4. **新币解禁费已是真实动账**：`src/modules/new_coin/infrastructure/unlock.rs:168-263` 同事务扣 available、写 wallet ledger、平台双腿和 paid evidence；释放路径还复核钱包与平台证据（约 346-581）。
5. **预测本地关盘和报价消费边界已加强**：`create_order_in_tx`（321-435）在市场锁后取 DB 时间、复核 quote expiry、market_version/last_synced_at 和资产精度；`src/workers/prediction_market_close.rs` 使用本地 DB 时间关闭到期市场。
6. **代理差额返佣生成是原业务事务的一部分**：`insert_agent_business_commission_in_tx`（27-108）按 active ancestor、累计费率和资产精度计算正差额，以 `(agent_id,source_type,source_id)` 幂等写 pending；来源交易回滚会连同佣金回滚。
7. **客服是当前范围内最完整的 authority/read-model 样板**：`.trellis/spec/backend/online-support.md:73-110` 与 `support/infrastructure.rs:243-390` 一致，精确 owner 鉴权、消息幂等、游标、改派与 MySQL/REST 权威都已落地；进程 WS 明确只是可丢刷新提示。
8. **多数后台配置已有 revision/reason/audit**：秒合约产品、借贷产品、理财产品、预测全局设置/资产配置、新币生命周期与合成策略均保留快照或审计；本报告特别指出的是预测“单市场覆盖/结算”这两条旁路。

## Invalidated / Narrowed Prior Conclusions

| 2026-08-24 旧结论 | 当前复证结果 | 当前证据 |
| --- | --- | --- |
| P0-03 新币由客户端价格/金额驱动且不扣总供给 | **失效；已修复** | `migrations/0111_new_coin_authoritative_issuance.sql`；`new_coin/infrastructure.rs:379-686,1013-1061` 权威 quote、请求指纹和供给 reserve/finalize |
| P0-04 解禁费只改 paid、不扣钱包 | **失效；已修复** | `new_coin/infrastructure/unlock.rs:168-263` 钱包、流水、平台 journal、paid evidence 同事务 |
| P0-05 抵押贷无 LTV/oracle/清算 | **失效；已修复原问题** | `migrations/0112_loan_collateral_risk.sql`、`0113_loan_liquidation_accounting.sql`；`loan/oracle.rs`、`loan/liquidation.rs`；本轮新问题是“到期处置”而非 LTV |
| P0-06 秒合约统一按 worker 处理时最新价 | **大部分失效；缩窄为 synthetic 交叉缺口** | `0114_event_time_price_snapshots.sql` 和 `select_settlement_price_snapshot` 已采用 event time；只有 synthetic ingestion 未写该历史表 |
| P0-07 预测不在 end_at/陈旧同步时关闭下单 | **失效；已修复** | `prediction/service.rs::validate_market_trading_window`、`infrastructure.rs:321-350`、`prediction_market_close.rs`；本轮发现是 admin hold 被 sync 覆盖 |
| P1-08 返佣无反冲 | **仍成立并已重新证明** | 预测创建佣金 `prediction/infrastructure.rs:422-434`；invalid refund `:511-539`；worker 仅看 commission status `agent_commission_settlement.rs:134-155` |
| P1-09 完全没有平台总账 | **部分失效** | migration 0110、loan 和 unlock 已接入；理财、新币销售、返佣等仍缺，因此改写为“覆盖局部” |
| P1-11 任一坏理财记录立即阻断全批 | **部分失效** | `earn_auto_redemption.rs:145-160` 已逐项 continue；但固定 500 前缀与无持久 retry 仍会在大量永久失败时饿死后项 |
| 客服实时提示丢失即状态丢失 | **不成立（按当前合同）** | `.trellis/spec/backend/online-support.md:14-16,105-107` 明确 MySQL/REST 权威，WS 仅提示；当前缺口是 agent status 与 assignment snapshot 联动 |

## Files Found

- `.trellis/tasks/08-30-project-code-business-optimization-reaudit/research/baseline-and-p0-verification.md` — 当前提交、migration 与 CI 外部服务缺口基线。
- `docs/architecture/project-optimization-audit-2026-08-24.md` — 上一轮 P0/P1 与业务保护矩阵。
- `.trellis/tasks/archive/2026-08/08-24-project-architecture-business-flow-audit/research/business-flows.md` — 上一轮逐流程研究证据。
- `src/modules/seconds_contract/{application,infrastructure,service}.rs` — 秒合约配置、开仓、event-time 结算、钱包/审计。
- `src/workers/{seconds_contract_settlement,market_feed,synthetic_market}.rs` — 到期结算、外部 tick 归档与合成行情生产。
- `src/modules/market/infrastructure/adapters/ingestion.rs` — Redis CAS、订单触发与 WS 摄取路径。
- `src/modules/loan/{application,oracle,liquidation,service,routes}.rs` — 借贷申请/审批/还款、oracle、LTV 清算和路由。
- `src/workers/{loan_overdue,loan_health}.rs` — 逾期标记与抵押健康扫描。
- `src/modules/earn/{application,infrastructure,service,redemption}.rs` — 申购、赎回、费用/金额计算和钱包写入。
- `src/workers/earn_auto_redemption.rs` — 到期理财扫描与自动赎回。
- `src/modules/prediction/{application,infrastructure,presentation,routes}.rs` — 同步、后台覆盖、quote/order 和整场结算。
- `src/workers/prediction_market_close.rs` — 本地数据库时间关盘。
- `src/modules/new_coin/{application,infrastructure}.rs`、`src/modules/new_coin/infrastructure/unlock.rs` — 权威发行、购买、锁仓、缴费和释放。
- `src/modules/agent/infrastructure.rs` — 来源事务内的多级差额佣金生成。
- `src/modules/admin/application/agents.rs`、`src/workers/agent_commission_settlement.rs` — 佣金发放和自动 worker。
- `src/modules/support/{application,infrastructure}.rs` — 精确 owner、消息、游标和归属同步。
- `src/modules/admin/application/market.rs`、`src/modules/admin/infrastructure/market.rs` — 合成策略恢复任务及 claim/terminal 状态。
- `src/workers/unlock_scanner.rs` — 到期解禁批处理。
- `migrations/0071_user_loans.sql`、`0110_platform_financial_journal.sql`、`0111_new_coin_authoritative_issuance.sql`、`0112_loan_collateral_risk.sql`、`0113_loan_liquidation_accounting.sql`、`0114_event_time_price_snapshots.sql`、`0115_prediction_market_local_close.sql` — 本轮状态机与账务基线。
- `mobile/src/api/{seconds,loan,earn,prediction,newCoin,support}.ts`、`pc/src/api/{loan,prediction}.ts`、`web/src/admin/` — 用户/管理端读写入口与运营工作台。

## Related Specs

- `.trellis/spec/backend/wallet-amount-precision.md` — 钱包金额和费率计算精度权威合同。
- `.trellis/spec/backend/seconds-contracts.md` — 秒合约入场、周期、事件价格和客户端字段合同。
- `.trellis/spec/backend/loan-products.md` — 借贷产品/admin filter；尚未覆盖到期违约与总敞口。
- `.trellis/spec/backend/earn-products.md` — 费用快照和手工/自动赎回共同计算合同。
- `.trellis/spec/backend/prediction-markets.md` — quote、关盘、结算、覆盖配置和 PC read model。
- `.trellis/spec/backend/new-coin-mobile-contract.md` — 新币项目 authority 与 mobile 生命周期。
- `.trellis/spec/backend/agent-hierarchy.md` — 多业务累计差额佣金合同；尚未定义 reversal。
- `.trellis/spec/backend/online-support.md` — 精确直属代理、REST 权威和改派合同。
- `.trellis/spec/backend/synthetic-market-kline.md` — realtime lease 与显式手动恢复合同；当前同步 HTTP 恢复语义正是 BPB-P1-07 的改进对象。
- `.trellis/spec/backend/realtime-websockets.md` — 进程内提示与 REST 恢复边界。

## External References

- 本报告没有使用外部网页或供应商文档；结论全部来自当前仓库源码、migration、spec、测试与上一轮审计。
- MySQL/Redis/Mongo/RabbitMQ 的生产版本、拓扑、隔离级别、时钟同步、broker policy 和云端运营能力均未取得仓库外证据。

## Caveats / Not Found

1. 本轮是静态复审。没有连接生产数据库或消息基础设施，也没有运行会写 `target/` 或外部服务的测试；当前 CI 本身未配置 MySQL/Redis/Mongo/RabbitMQ services，关键集成测试会跳过，见 baseline 文件 23-33 行。
2. P0/P1 表示源码存在可达风险，不表示线上已经发生资金损失。运行时优先核查：strategy pair 秒合约与超龄 opened、earn 超精度余额、loan overdue 账龄/类型、invalid prediction 的已发佣金、inactive agent 的 assigned support 会话、stale recovery jobs。
3. 平台 journal 的科目、理财负债、新币发行会计属性、信用贷逾期政策和佣金追收方式需要产品/财务/法务确认；报告只证明当前代码没有这些闭环，不替代业务政策。
4. `earn_products.max_subscribe` 是否意图表示单笔还是用户累计上限在现有 spec 中不明确，因此未单独列缺陷；借贷总敞口项也保留了“若产品明确允许无限多笔则降级”的兼容说明。
5. 秒合约 Redis 入场价只检查超过 60 秒的过去陈旧值，未设置统一的未来时钟偏差上限：`src/modules/seconds_contract/infrastructure.rs:621-648`；Redis CAS 会让未来 tick 压住后续正常 tick（`src/modules/market/infrastructure/cache.rs:404-419`）。证据成立，但为遵守 5-12 条高置信度上限，本报告未另计 ID；建议随 BPB-P0-01 的统一时间权威修复一并处理。
6. 路径与行号基于本次读取到的当前工作树；后续修改时应优先按符号名定位。此次只写入本研究文件，未修改生产代码、spec、进度文件或其他任务目录。
