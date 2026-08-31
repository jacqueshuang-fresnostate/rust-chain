# Research: 当前 Rust 后端架构与核心金融流复审

- Query: 审计当前 Rust 后端的认证/授权、用户创建与钱包初始化、充提币、现货、杠杆、闪兑、钱包过账/账本，以及事务、幂等、锁序和错误边界；对照 2026-08-24 审计与 P0 修复后现状，仅保留当前可达且证据充分的剩余/新增根因。
- Scope: internal
- Date: 2026-08-30

## 基线与方法

- 当前基线：`.git/HEAD` 指向 `main`，分支引用为 `fac1defff85d55d556949ec8b04b3c5a3f9e262a`；`.git/logs/HEAD:235` 记录 P0 修复提交 `5aa98f14f2bd8f0e7c3fb615e60d2e2a2a5a8cb4`（`fix: 完成 P0 发布阻断风险修复`）。本轮以当前符号和 HTTP/worker 可达链独立复核，没有仅沿用旧报告结论。
- 对照材料：`docs/architecture/project-optimization-audit-2026-08-24.md`、`.trellis/tasks/archive/2026-08/08-24-p0-remediation-program/prd.md`、`docs/superpowers/PROGRESS.md:7794`。
- 严重度是本轮**建议优先级**：P0 为发布阻断/立即修；P1 为近期资金或安全整改。未发现可证明 P0 修复回退的当前证据。

## Findings

### F-01 — P0：高价值资金命令仍允许无稳定客户端幂等身份

- **根因/当前证据**：后台人工充值 DTO 没有请求键（`src/modules/admin/presentation/users.rs:29-36::AdminUserRechargeRequest`），每次调用都生成新 UUID，源码明确承认不确定提交后重试会再次入账（`src/modules/admin/application/users.rs:150-205::recharge_admin_user_wallet`）；底层又把幂等责任留给上层（`src/modules/admin/infrastructure/wallet_assets.rs:883-917::credit_admin_wallet_available_in_tx`）。同一根因还出现在用户现货建单：`idempotency_key: Option<String>`（`src/modules/spot/presentation.rs:11-21`），缺键直接继续创建（`src/modules/spot/application/idempotency.rs:18-41::replay_spot_order_for_idempotency_key`）；杠杆划转也为可选键（`src/modules/margin/presentation.rs:88-96`），省略时服务端生成新 UUID（`src/modules/margin/application/account_settings.rs:621-640::normalize_transfer_idempotency_key`）。
- **影响**：人工充值在超时、代理重试或双击下可直接重复铸入可用余额；现货会重复建单/冻结/成交，杠杆会重复搬运资金。后台充值的直接增发路径决定本项建议为 P0，未再拆成三条重复发现。
- **增量修复**：所有高价值命令强制接受稳定 `request_id/idempotency_key`，以“主体 + 操作 + key”唯一占位并保存请求指纹和首次响应；同键同参返回原结果，同键异参 409。禁止用服务端临时 UUID 冒充跨重试幂等。
- **迁移/兼容**：先升级 admin/PC/mobile 客户端，再将字段由可选转必填；为人工充值增加命令/收据表唯一键，现货唯一键宜从全局键迁为 `(user_id, idempotency_key)`；存量 NULL 保留只读兼容。
- **验证**：并发 20 次、事务提交后模拟断连再重试、同键异参、客户端超时重放；每种场景断言仅一条业务记录、一组钱包变动和一组流水。
- **工作量/依赖**：M，约 4–7 天；依赖三端请求合同及 migration。**运行时证据**：静态缺口成立；另需查历史重复充值/订单/划转以决定数据修复范围。

### F-02 — P1：用户访问令牌不回查账号活跃状态，停用后的阻断依赖提交后会话清理

- **根因/当前证据**：统一令牌校验明确“不查询账号表”（`src/modules/auth/mod.rs:529-550::claims_from_bearer_token`），`UserAuth` 只校验 token/scope（`src/modules/auth/mod.rs:619-633`）；与之相比 `AdminAuth` 会回查状态、会话代际及 RBAC（`:637-659`）。管理员停用用户先提交状态/刷新令牌/审计，再在提交后撤销在线会话（`src/modules/admin/application/users.rs:103-147::update_admin_user_status`）。若该副作用失败，数据库已停用但旧访问会话可保留。现货、提现、杠杆、闪兑资金入口均只从 `UserAuth` 提取主体，例如 `src/modules/spot/routes.rs:79-95`、`src/modules/wallet/routes.rs:228-239`、`src/modules/margin/routes.rs:356-376`、`src/modules/convert/routes.rs:49-82`。
- **影响**：会话撤销部分失败并恢复后，已停用用户仍可能用旧 token 发起资金写操作直至会话到期或人工重试成功。
- **增量修复**：先在 `UserAuth` 集中回查 `users.status='active'`（资金事务可再做最终状态闸门）；随后可引入用户 `auth_session_version`，停用/改密同事务递增，token 版本不一致即拒绝，把 Redis 清理降为补偿而非唯一安全闸门。
- **迁移/兼容**：仅状态回查无需迁移；会话代际列可默认 0，先双读兼容旧 token，再切换签发与强校验。
- **验证**：故障注入会话枚举/logout 失败，停用提交后用旧 token 调四类资金路由，均须 401/403 且零资金副作用。
- **工作量/依赖**：S/M，2–5 天；依赖认证缓存/数据库负载策略。**运行时证据**：静态可达链成立；需统计 token TTL 与历史撤销失败确定暴露窗口。

### F-03 — P1：刷新令牌可重复兑换，未形成消费、轮换和重放检测

- **根因/当前证据**：两种刷新实现都只查询旧 token 后签发新会话，不消费或撤销传入 token（`src/modules/auth/service.rs:224-268::refresh`、`:270-301::refresh_sa_token`）。Sa-Token 签发还先创建访问会话、再单独保存刷新令牌，两个后端不原子且失败无补偿（`:402-465::issue_sa_tokens`）。
- **影响**：被窃取的刷新令牌在过期或主体级撤销前可持续兑换新访问会话；并发刷新会形成多个有效会话，重放无法被识别。
- **增量修复**：实现单次消费 + rotate，持久化 token family、父子关系、used/revoked 时间及请求指纹；Redis 路径用 Lua/CAS 保证同一旧 token 只有一个成功，检测到已消费 token 重放时撤销整个 family。创建访问会话后刷新存储失败应补偿 logout。
- **迁移/兼容**：为 MySQL/Redis 记录引入 family/version 状态；短期兼容旧 token 一次换新，换出的新 token 全部执行轮换合同。
- **验证**：同 token 并发刷新仅一次成功；再次重放触发 family 撤销；刷新存储失败不得遗留可用访问会话。
- **工作量/依赖**：M，4–7 天；依赖 Sa-Token 存储原语和 Redis 原子脚本。**运行时证据**：静态缺口成立，不依赖生产数据。

### F-04 — P1：新用户钱包初始化仍依赖未闭环的异步投递链

- **根因/当前证据**：用户、邀请码和 `user.created` outbox 同事务，但钱包不在注册事务创建（`src/modules/auth/application.rs:507-567::register_user_with_email_code`；后台创建同样见 `src/modules/admin/application/users.rs:49-100`）。钱包只由 inbox handler 执行 `INSERT IGNORE ... SELECT assets`（`src/modules/events/infrastructure.rs:260-304::create_wallet_accounts_for_user_in_tx`）。publisher 未启用 confirm，`basic_publish` 完成后即视为成功（`src/modules/events/application.rs:190-197`、`src/modules/events/service/rabbitmq.rs:107-138::publish`）；消费者要求队列预先声明/绑定（`:141-195`），且缺少 `EVENT_INBOX_QUEUE_NAME` 会被视为正常停用（`src/workers/event_inbox.rs:66-113`）。现货资金路径遇到账户缺失会直接失败（`src/modules/spot/infrastructure/wallet_accounts.rs:37-56::lock_wallet_row`）。
- **影响**：broker 未确认持久化、拓扑缺失或 consumer 被静默关闭时，注册成功用户长期没有钱包，资金功能出现数据依赖型失败；当前仓库未形成“用户 × 资产”补偿闭环。
- **增量修复**：首选在用户创建的同一 MySQL 事务经 wallet provisioning port 创建零余额账户，事件仅承载非关键副作用；同时为历史/新增资产保留幂等 reconciler。若继续异步，则至少启用 publisher confirms、应用声明/校验 durable topology，并把 consumer 配置纳入 readiness。
- **迁移/兼容**：无需破坏 API；先跑幂等回填 `users × assets - wallet_accounts`，再切同步创建，保留 inbox 重放兼容。
- **验证**：断 RabbitMQ 注册、publish 后崩溃、无队列/错绑定、重复事件、多实例并发；SLA 内钱包覆盖率必须 100%。
- **工作量/依赖**：M，5–8 天；依赖上下文端口和部署拓扑。**运行时证据**：需核对生产 RabbitMQ topology、publisher confirm、缺失钱包数量；静态交付缺口已成立。

### F-05 — P1：充值手续费配置仅展示，入账与冲正未快照/执行

- **根因/当前证据**：资产有 `deposit_fee`（`migrations/0063_asset_deposit_withdraw_fee_settings.sql:1-4`），列表也返回该字段（`src/modules/wallet/infrastructure/deposits.rs:748-763`）；但充值目标只读取精度、最小额和确认数（`:386-425::observe_deposit_event`），事件表只有单一 `amount`（`migrations/0087_p0_financial_safety.sql:83-107`），确认按全额 `event.amount` 入账（`src/modules/wallet/infrastructure/deposits.rs:697-745::credit_deposit_event_in_tx`），冲正也按同一全额（`:486-561::reverse_deposit_event`）。
- **影响**：非零手续费配置不会收取；历史入账缺少 gross/fee/net 与规则版本，之后无法按原始政策精确冲正或对账。
- **增量修复**：事件首次观测时固化 `gross_amount/fee_amount/net_amount/fee_rule_version`；确认贷记 net 并记平台 fee 腿，冲正严格复用原快照。若产品决定充值费仅用于展示，应删除/重命名配置，消除错误合同。
- **迁移/兼容**：新增列先可空，存量事件按“fee=0、net=amount、legacy version”回填；读模型兼容旧 `amount`。
- **验证**：固定费为 0/非 0、边界精度、重复确认、规则变更后确认、链重组冲正，断言 `gross=fee+net` 且按原版本逆向。
- **工作量/依赖**：M，4–7 天；依赖产品收费政策和平台费用科目。**运行时证据**：需查询生产非零 `deposit_fee` 与历史事件后才能定暴露金额。

### F-06 — P1：提现创建明确绕过 Redis 限频执行面

- **根因/当前证据**：提现创建在冻结前调用风控，但源码明确说明该用例没有 Redis 句柄并把 `None` 传给 `enforce_risk_control`（`src/modules/wallet/application.rs:805-822::create_withdrawal_request`）。资产/网络、权威 quote、资金密码/2FA 和数据库规则仍执行，本项仅归因于跨实例限频执行面缺失。
- **影响**：已配置的用户/资产频率规则不能在提现路径生效，可被高频请求绕过；数据库规则若不覆盖速率语义，将增加盗号资金外流和资源滥用风险。
- **增量修复**：把 `ConnectionManager` 从路由/AppState 显式传入提现用例，并为 Redis 故障定义资金操作的 fail-closed 或受控降级政策；限频 key 必须跨实例共享。
- **迁移/兼容**：无数据库迁移；函数签名和路由装配变更，不改变 HTTP JSON。
- **验证**：跨两个实例发 N+1 请求、Redis 故障、规则禁用/启用、幂等重放；拒绝路径不得消耗安全凭据、quote 或写资金记录。
- **工作量/依赖**：S，1–3 天；依赖明确的 Redis 故障政策。**运行时证据**：需核对生产风险规则是否启用及历史命中率，静态绕过成立。

### F-07 — P1：资产精度合同仍未在共享过账边界统一执行

- **根因/当前证据**：规范要求所有用户输入与计算金额服从 `assets.precision_scale`，且 ledger amount 必须等于真实账户变动（`.trellis/spec/backend/wallet-amount-precision.md:10-22`）。当前后台现货 fill 只校验价格/数量为正（`src/modules/spot/application/settlement.rs:237-250::validate_fill_spot_order_request`），交易对结算只取资产 ID、不取资产精度（`src/modules/spot/infrastructure/trade_settlement.rs:431-447::pair_assets_in_tx`），随后把 `price * quantity` 原值写四条资金腿（`src/modules/spot/application/settlement.rs:61-73,133-153::settle_spot_fill`）。杠杆开仓锁定的产品规则没有保证金币种精度（`src/modules/margin/infrastructure/positions.rs:20-45::MarginOpenProductRule`、`:296-317::lock_active_open_product`），`validate_product_margin` 不校验资产精度且原值扣抵押（`src/modules/margin/application/open_position.rs:175-245,359-385`）。人工充值也只验正数（`src/modules/admin/service/users.rs:9-19`）。闪兑虽正确量化 quote，但结算通过截断“加款后总余额”而非直接应用已量化增量，ledger 仍写原 `to_amount`（`src/modules/convert/infrastructure.rs:772-835::settle_convert_order_in_tx`），遇到既有 dust 时账户增量与流水金额可能不一致。
- **影响**：可生成超过资产业务精度的余额/流水 dust，并使流水 amount 与账户实际 delta 漂移，破坏跨业务重放与对账。
- **增量修复**：建立 transaction-aware `WalletPostingPort`：锁资产规则与钱包，用户输入超精度拒绝，计算值向零截断，ledger 取真实 delta；现货 fill 同时验证 pair 精度与 base/quote 资产精度，杠杆加载 margin asset 精度，人工充值拒绝超精度。
- **迁移/兼容**：先画像并清理存量 dust，再逐上下文收紧；交易对需校验 `qty_precision <= base asset precision` 等配置不变量。避免直接对动态资产精度加不可表达的静态 CHECK。
- **验证**：precision 0/2/8/18，正负/尾零、部分成交、杠杆开仓、人工充值、带既有 dust 的闪兑；逐笔断言 `ledger.amount = account_after-account_before`。
- **工作量/依赖**：L，1–2 周；依赖共享过账端口和数据清理。**运行时证据**：需扫描存量余额/流水 dust；当前写边界缺失已由静态路径证实。

### F-08 — P1：平台总账只覆盖少数 P0 场景，核心资金流仍无平衡对手腿

- **根因/当前证据**：`wallet_ledger` 是单一用户账户变动及 after snapshot（`migrations/0003_assets_wallet_ledger_locks.sql:25-42`），不是平台双边总账。P0 新增的 `platform_financial_journal` 具备稳定 transaction key 与科目腿（`migrations/0110_platform_financial_journal.sql:1-17`），但当前生产写入仅见新币解禁费与借贷放款/还款/清算（`src/modules/new_coin/infrastructure/unlock.rs:229`、`src/modules/loan/infrastructure.rs:1438-1538`、`src/modules/loan/liquidation.rs:544-642`）；充值、提现、闪兑和杠杆仍只写用户/杠杆钱包流水，核心现货只记录用户/做市账户腿。
- **影响**：无法从统一账本证明托管资产、清算库存、手续费收入、应收和坏账按资产守恒；漏腿/重复腿只能靠跨表定制查询发现。
- **增量修复**：先以 shadow 模式扩展现有 journal：建立 custody/clearing/treasury/fee/insurance/bad-debt 系统科目，每笔 posting 使用唯一 transaction key，并在同一业务事务写用户腿映射与平台腿；对账稳定后再提升为权威过账入口。
- **迁移/兼容**：按 deposit→withdrawal→convert→margin 分批；存量通过业务 ref 回填并保留来源版本，禁止猜测无法重建的历史腿，差异进入 reconciliation queue。
- **验证**：每 transaction/asset 的分录和为 0；钱包余额可由 posting 重演；人为删除/重复一腿必须触发对账告警。
- **工作量/依赖**：XL，3–6 周；依赖科目表、托管/财务政策及 F-07 过账端口。**运行时证据**：需取得托管余额与历史业务数据做真实 reconciliation；静态覆盖缺口成立。

### F-09 — P1：杠杆计息口径与逐仓坏账仍未闭环

- **根因/当前证据**：计息按 `floor(now-checkpoint)` 的完整小时计算后把 checkpoint 直接写成 `now`，源码明确当前利率来自产品表且立即影响历史未计提窗口（`src/workers/margin_interest.rs:195-306::accrue_position_interest`、`:303-327::lock_position`、`:329-349::margin_interest_delta/full_elapsed_hours`）。这使调用分片和改价时点影响结果。逐仓强平把负 equity 截零并明确不登记缺口（`src/workers/margin_liquidation.rs:588-697::liquidate_position_by_id`、`:1101-1109::non_negative_amount`）；全仓则已把 `bad_debt` 原子写入账户（`:821-914::liquidate_cross_account`），证明缺口只剩逐仓路径。
- **影响**：同一持仓时长因 worker 调度分片不同可能少计利息，产品改率可追溯作用于未计提小时；逐仓穿仓损失对平台账务不可见。
- **增量修复**：计息保存 rate/version 有效期或开仓/变更快照，并把 checkpoint 推进为“旧 checkpoint + 已计完整时长”以保留余数；逐仓在同一强平事务写 `bad_debt` 快照及平台保险/坏账 journal 腿。
- **迁移/兼容**：新增利率版本/计息游标与 isolated bad-debt 字段；存量从当前 checkpoint 起采用新口径，不追溯重算，除非业务确认历史规则。
- **验证**：同一时间区间按 60/90/随机分钟分片结果相同；窗口内改率按版本分段；负 equity 逐仓强平只执行一次且坏账与 journal 可对账。
- **工作量/依赖**：L，2–4 周；依赖计息政策、保险/坏账科目与 F-08。**运行时证据**：需核对 worker 调度、历史改率与逐仓负 equity 数量以量化金额。

### F-10 — P1：全局 5xx 错误仍向客户端回显底层错误原文

- **根因/当前证据**：`AppError` 的数据库、Redis、RabbitMQ、Mongo、配置和 Internal 变体的 `Display` 包含底层文本（`src/error.rs:15-55`），`IntoResponse` 虽记录 5xx 日志，却把 `self.to_string()` 直接放入响应（`:131-148::into_response`）。
- **影响**：SQL/连接地址、表列名、缓存键、上游 URL 或序列化内容可能泄露给终端；错误文案不稳定也会把基础设施细节变成客户端隐式合同。
- **增量修复**：5xx 响应只返回稳定公共 code/message 与 `error_id/request_id`；完整 error chain 仅结构化日志记录。4xx 仍可保留经审查的业务语义。
- **迁移/兼容**：HTTP 状态和现有码值可保留；客户端不得依赖 5xx message，先发布固定文案再清理前端匹配。
- **验证**：注入带 secret/SQL marker 的各类基础设施错误，响应不得包含 marker，日志须能按 error_id 找回完整上下文。
- **工作量/依赖**：S，1–2 天；依赖统一 request/error ID。**运行时证据**：不需要，当前响应映射直接证明。

### F-11 — P1：手工资金操作有 RBAC，但部分路径不记录操作者/原因的原子审计

- **根因/当前证据**：后台现货 fill 是真实双边结算入口，路由丢弃 `AdminAuth` claims（`src/modules/spot/routes.rs:63-73,207-219::fill_orders`），请求只有订单、价格、数量和幂等键，没有 reason（`src/modules/spot/presentation.rs:35-44`），结算用例也不接收 admin actor（`src/modules/spot/application/settlement.rs:182-250`）。后台手工充值观测和链重组冲正同样使用 `AdminAuth(_claims)`，不向资金事务传 admin ID（`src/modules/wallet/routes.rs:369-395`）；其持久化只写事件、钱包和流水（`src/modules/wallet/infrastructure/deposits.rs:386-561`）。对照之下，强制撤单已在事务内写管理员审计（`src/modules/spot/infrastructure/order_repository.rs:125-127,463-485`）。
- **影响**：有写权限的人可以手工触发成交、充值入账或冲正，但事后无法从同一原子审计链证明具体管理员和业务理由；共享账号或凭据滥用时追责与回滚困难。
- **增量修复**：为敏感命令引入 `CommandActor::{Admin(id), SystemWorker}` 与必填 reason/request_id；在同一资金事务写 `admin_audit_logs` 或不可变 command receipt，worker 明确记 system actor。
- **迁移/兼容**：HTTP 增加 reason 可先兼容可选并告警，admin UI 上线后转必填；历史记录 actor 置 unknown/system，不伪造归属。
- **验证**：管理员与 worker 两种入口、重复幂等、事务回滚、审计写失败；断言资金与 actor receipt 要么同时提交、要么同时回滚。
- **工作量/依赖**：M，3–5 天；依赖统一后台请求上下文。**运行时证据**：静态缺口成立；需核对网关访问日志才能补充历史操作者线索。

## 当前强项

1. **生产依赖 fail-fast，认证作用域分离**：`src/main.rs:39-57` 启动必须连接 MySQL/Mongo/Redis/Sa-Token/RabbitMQ；`src/modules/auth/mod.rs:529-550,619-676` 分离 user/admin/agent scope。后台额外实时回查状态、会话代际与 RBAC，未映射路由默认需要 `admin.unmapped`/`*`（`src/modules/admin/application/access_control.rs:17-42`、`src/modules/admin/service/access_control.rs:88-117`）。
2. **用户创建核心数据原子**：注册及后台创建均把用户、邀请码/推荐关系、outbox 放在同一 MySQL 事务（`src/modules/auth/application.rs:507-562`、`src/modules/admin/application/users.rs:49-100`）。
3. **充值主链幂等和锁序扎实**：外部事件唯一键 `(network, tx_hash, event_index)`，确认/钱包/流水同事务，冲正不足转人工审核（`migrations/0087_p0_financial_safety.sql:83-107`、`src/modules/wallet/infrastructure/deposits.rs:386-561`）。
4. **提现 P0 状态机已形成保守闭环**：权威 quote、配置版本、指纹、一次消费和冻结同事务（`src/modules/wallet/application.rs:675-867`、`src/modules/wallet/infrastructure/withdrawals.rs:479-650`）；歧义广播保留 frozen，只有权威未受理且无受理证据才释放（`:760-930`），确认只从 frozen 核销（`:1138-1215`）。
5. **现货核心结算具备稳定锁序和成交幂等**：订单稳定加锁、trade key 占位、双方 base/quote 钱包稳定排序、四腿和订单状态同事务（`src/modules/spot/application/settlement.rs:35-179`、`src/modules/spot/infrastructure/wallet_accounts.rs:143-223`）。
6. **全仓杠杆转出 P0 闸门已落地**：预取同批新鲜 ticker，事务内复核账户 version/仓位集合/价格年龄，按“账户→仓位→spot wallet→margin wallet”一致顺序校验转后维持保证金（`src/modules/margin/application/account_settings.rs:175-245,270-325`、`src/modules/margin/infrastructure/transfers.rs:1-71`）。
7. **闪兑 P0 权威边界已落地**：quote 先落 MySQL，市场价来自 append-only MySQL 历史；确认锁 quote、校验 owner/指纹/数据库时间/当前配置，并按资产 ID 稳定锁钱包、一次消费（`src/modules/convert/application.rs:76-215,245-290`、`src/modules/convert/infrastructure.rs:427-488,491-625`）。

## 已失效或被收窄的旧发现

- **旧 P0-01 默认管理员：已失效。** 当前引导缺省关闭、必须显式 `create_admin`，口令只来自 Secret/环境且拒绝已知公开默认值，新管理员强制改密（`src/bootstrap.rs:1-4,27-109,192-240`；`migrations/0107_bootstrap_admin_password_gate.sql:1-12`）。
- **旧 P0-02 提现歧义失败自动解冻：已失效。** `unknown_broadcast/manual_review` 不走普通 release；只有权威未受理且无 tx/evidence 才能退冻（`src/modules/wallet/infrastructure/withdrawals.rs:760-930`；`migrations/0108_withdrawal_broadcast_reconciliation.sql:1-82`）。
- **旧 P0-08 闪兑缓存新鲜度/TOCTOU/反向锁序：已失效。** 当前 Redis 仅展示缓存，MySQL quote 是唯一权威且锁行消费；钱包按 asset ID 排序（`migrations/0116_convert_quote_authority.sql:1-49`、`src/modules/convert/infrastructure.rs:427-488,754-760`）。
- **旧 P0-09 全仓转出不看转后风险：已失效。** 当前在锁后复算前后风险并阻止低于维持保证金的提交（`src/modules/margin/application/account_settings.rs:175-245`）。
- **旧 P1-02 “会话撤销失败仍返回成功”：该具体表述已失效。** `revoke_actor_auth_sessions` 的枚举/单次 logout 错误会上抛（`src/modules/auth/mod.rs:368-403`）；但数据库状态先提交且 UserAuth 不回查，剩余风险已去重归入 F-02。
- **旧 P1-04 的资产-网络绑定部分已失效，限频部分仍在。** quote 和提交都会校验活动网络/资产及配置变化（`src/modules/wallet/application.rs:675-714,784-803`）；Redis 限频缺口单列 F-06。
- **旧 P1-05/P1-07/P1-09 被 P0 部分修复但未整体失效。** 提现、杠杆划转和闪兑 quote 已执行资产精度，全仓坏账与借贷/新币 platform journal 已落地；剩余分别归入 F-05/F-07、F-09、F-08。

## Files Found

- `src/modules/auth/mod.rs` — token scope、主体提取、会话撤销及 User/Admin/Agent 授权边界。
- `src/modules/auth/service.rs` — 登录、刷新和 Sa-Token 签发流程。
- `src/modules/auth/application.rs` — 用户注册事务及 `user.created` outbox。
- `src/modules/admin/application/users.rs` — 后台建用户、停用用户、人工充值及审计事务。
- `src/modules/events/{application.rs,infrastructure.rs,service/rabbitmq.rs,service/production_dispatch.rs}` — outbox/inbox、RabbitMQ 投递和钱包初始化链。
- `src/workers/event_inbox.rs` — inbox 配置、重连和补偿扫描。
- `src/modules/wallet/{application.rs,routes.rs}` — 用户/后台充提币路由与应用编排。
- `src/modules/wallet/infrastructure/{deposits.rs,withdrawals.rs,shared.rs}` — 充值、提现、钱包和流水事务。
- `src/workers/wallet_chain.rs` — 链网关广播/查询状态机。
- `src/modules/spot/{routes.rs,presentation.rs}` — 用户下单和后台填单 HTTP 合同。
- `src/modules/spot/application/{order_creation.rs,idempotency.rs,settlement.rs}` — 现货建单、重放和四腿结算。
- `src/modules/spot/infrastructure/{trade_settlement.rs,wallet_accounts.rs}` — 交易对资产解析、钱包锁和资金腿。
- `src/modules/margin/application/{open_position.rs,account_settings.rs}` — 开仓与双向划转/全仓风险闸门。
- `src/modules/margin/infrastructure/{positions.rs,transfers.rs}` — 产品/仓位及 spot-margin 钱包事务。
- `src/workers/{margin_interest.rs,margin_liquidation.rs}` — 计息与逐仓/全仓强平。
- `src/modules/convert/{application.rs,infrastructure.rs}` — MySQL 权威 quote 与双钱包结算。
- `src/error.rs` — 全局错误分类和 HTTP 响应边界。
- `migrations/0003_assets_wallet_ledger_locks.sql` — 资产、钱包三桶和用户流水基础结构。
- `migrations/0063_asset_deposit_withdraw_fee_settings.sql`、`0087_p0_financial_safety.sql` — 充值费配置与链事件幂等表。
- `migrations/0107`–`0110`、`0116`、`0117` — P0 管理员/提现/平台 journal/闪兑修复及后续杠杆部分平仓。
- `tests/{events_inbox.rs,events_outbox.rs,margin_routes.rs,margin_liquidation_worker.rs}` — 相关集成测试与当前行为合同。

## Code Patterns

- **安全模式**：先业务单据/幂等占位，再按稳定键排序锁钱包，余额、流水与状态同一 MySQL 事务；代表：`src/modules/spot/application/settlement.rs:35-179`、`src/modules/convert/infrastructure.rs:427-488`。
- **危险模式**：可选/服务端临时幂等键用于可重试资金命令；代表：`src/modules/spot/application/idempotency.rs:18-29`、`src/modules/margin/application/account_settings.rs:621-628`。
- **危险模式**：跨提交副作用被当作安全阻断的唯一机制；代表：`src/modules/admin/application/users.rs:103-147`。
- **危险模式**：业务上下文直接更新钱包并各自拼流水，缺少共享精度与平台 posting invariant；代表：`src/modules/convert/infrastructure.rs:772-835`、`src/modules/spot/infrastructure/wallet_accounts.rs:161-223`。

## External References / Versions

- 本轮未联网检索外部文档；结论来自当前仓库代码、migrations、测试和项目 specs。
- `Cargo.lock` 当前关键版本：Axum 0.7.9、SQLx 0.8.6、Lapin 2.5.5、Redis 0.27.6（项目直接依赖；Sa-Token storage 另带 Redis 1.2.3）、Sa-Token crates 0.1.18、BigDecimal 0.4.10、Tokio 1.52.3、Reqwest 0.12.28。
- 生产 MySQL、RabbitMQ、Redis 服务端版本与 RabbitMQ policy/topology 不在仓库内，需运行环境补证。

## Related Specs

- `.trellis/spec/backend/auth-sessions.md` — 会话存储、作用域和撤销合同。
- `.trellis/spec/backend/user-authentication.md` — 用户登录/注册安全边界。
- `.trellis/spec/backend/deposit-addresses.md` — 充值地址与网络配置合同。
- `.trellis/spec/backend/wallet-amount-precision.md` — `assets.precision_scale` 与 ledger delta 合同。
- `.trellis/spec/backend/spot-orders.md` — 现货订单、ticker、冻结和成交合同。
- `.trellis/spec/backend/margin-trading-actions.md` — 杠杆幂等、wallet scope、全仓风险及锁序。
- `.trellis/spec/backend/error-handling.md` — 公共错误响应边界。
- `.trellis/spec/backend/database-guidelines.md`、`quality-guidelines.md` — 事务、migration 与验证要求。

## Caveats / Not Found

- 这是静态复审，未连接生产 MySQL/Mongo/Redis/RabbitMQ，未执行链上对账、故障注入、容量压测或历史余额重演；每条发现已标明是否需要运行时证据量化。
- 本轮为只写 research 的子代理；未运行会写 `target/` 的 Cargo 命令，也未修改生产代码、spec、progress 或其他任务文件。
- `src/modules/wallet/service.rs:147-216` 的通用跨资产 `settle`/锁仓方法自身非原子，且 `accounts_ledger.rs:381-435` 是无行锁绝对值更新；当前 `src/` 搜索只找到其兼容 `SpotService` 定义，没有找到 HTTP/worker 生产装配，因此未把它计为当前可达资金漏洞，但未来接入前必须封禁或重构。
- 旧报告的“现货应为中央订单簿还是系统做市柜台”仍缺唯一产品合同；当前实现的系统流动性模式本身未被当作缺陷，避免把产品选择误报为资金 bug。
- 未发现当前 P0 修复的明显回退；F-01 是本轮最强的新 P0 建议，其余为高置信 P1 剩余项。
