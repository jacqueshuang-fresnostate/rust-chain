# Research: 数据、异步任务与运维交付审计

- Query: 只读审计数据库迁移、数据约束、MQ/后台 workers、行情/WS、配置、密钥、Docker/1Panel、GitHub Actions、日志指标、备份恢复与质量门禁；重点检查启动顺序、重复 worker、迁移兼容、环境变量漂移、单点、重试/死信、健康检查、资源限制、供应链与发布回滚。
- Scope: mixed（仓库静态证据 + 官方文档行为核对）
- Date: 2026-08-24

## Findings

### 结论摘要

| 等级 | 数量 | 结论 |
|---|---:|---|
| P0 | 3 | 提现广播歧义失败后释放冻结资金；行情热重载遗留旧 provider 子任务；公开默认口令创建通配权限管理员。 |
| P1 | 11 | 行情时间可信度、MQ 可靠性、worker 所有权与监督、重试/死信、跨实例行情与 WS、配置漂移、可观测性、部署单点与密钥、备份恢复、CI/供应链/回滚、数据库约束均存在未闭环项。 |

下文的“已防护”表示代码中已有可验证机制，“未防护”表示本次证据确认仍存在的缺口；两者不相互抵消。

### P0-01 提现广播歧义失败达到次数上限后释放 frozen，存在链上付款与本地退款并存窗口

- **状态：未防护（已有幂等基础，但终态决策违反接口合同）。**
- **问题：** 网关 HTTP 超时、连接失败、非 2xx、响应 JSON 解析失败都可能发生在网关已经受理请求之后。接口合同明确要求这类错误不能据此释放 frozen，但 worker 将所有错误合并计数，达到上限后把 `broadcasting` 请求标记 `failed` 并全额解冻。
- **路径与行号/符号证据：**
  - `src/modules/wallet/infrastructure/withdrawals.rs:44-74`，`HttpWalletChainGateway::broadcast_withdrawal`；其中 `:47` 明确写明远端可能已受理，调用方不得因超时释放 frozen，`:54-74` 将传输、HTTP 状态与反序列化失败统一返回错误。
  - `src/modules/wallet/repository.rs:92-101`，`WalletChainGateway::broadcast_withdrawal`；`:95` 规定 `request_id` 是外部幂等身份，超时/失败应保留本地冻结。
  - `src/workers/wallet_chain.rs:111-120`，`run_once_with_gateway` 合同；`:168-183` 的实际分支在 `attempts >= max_attempts` 时调用 `release_withdrawal_in_tx(..., "failed", ...)`；默认上限见 `src/workers/wallet_chain.rs:43-52`。
  - `src/modules/wallet/infrastructure/withdrawals.rs:367-454`，`release_withdrawal_in_tx`；`:386-390` 允许从 `broadcasting` 释放，`:404-430` 把 `total_reserved` 加回 available 并扣减 frozen，`:431-452` 落 `failed/released_at`。
- **影响：** 网关已广播而客户端连续超时的情况下，用户既可收到链上转账，又重新获得平台可用余额并再次消费/提现，形成直接平台资金损失；本地 `failed` 状态还会掩盖链上待确认交易。
- **已防护：** `gateway_request_id` 稳定且唯一；网关合同要求外部幂等；认领使用状态条件与 30 秒可见性窗口（`src/workers/wallet_chain.rs:601-648`）；已取得 `tx_hash` 的申请不会走自动解冻。
- **未防护：** 没有“确定性拒绝”与“结果未知”的错误分类；没有按 `request_id` 查询网关结果的能力；没有保留 frozen 的 `unknown_broadcast/manual_review` 状态；达到次数上限自动释放没有人工门禁。
- **建议：** 仅对可证明请求未提交的确定性前置拒绝允许释放；传输、5xx、响应解析和确认丢失统一进入 `unknown_broadcast`，保持 frozen，并按 `gateway_request_id` 查询/对账；超过自动重试预算后转人工复核并告警，而不是退款。状态、查询结果和人工操作均保留审计记录。
- **验收：** 集成测试模拟“网关已受理并返回 tx_hash，但客户端每次读超时直至超过最大次数”：申请保持冻结且不产生 `withdrawal_release` 流水；后续按 request_id 查询到交易后只确认一次。另测确定性拒绝只能释放一次，进程崩溃/重放不重复动账。
- **依赖：** 链网关提供幂等查询契约；新增状态/约束迁移；管理端人工复核入口；告警与链上对账任务。
- **工作量：** M/L。

### P0-02 行情热重载/禁用只终止父任务，旧 provider 无限循环会脱离监督继续写入

- **状态：未防护。**
- **问题：** 每个 provider 被再次 `tokio::spawn` 为独立子任务；父级只等待第一个完成，不中止/等待其余任务。热重载和停用仅 `abort` 父级 JoinHandle。Tokio 的 JoinHandle 被丢弃时任务会 detach，父任务取消不会递归取消其另行 spawn 的子任务，因此旧 provider 连接和写入可持续存在。
- **路径与行号/符号证据：**
  - `src/workers/market_feed.rs:657-695`，`run_config_loop`；`:677-691` 为每个 provider 创建独立任务。
  - `src/workers/market_feed.rs:716-735`，`await_market_feed_provider_tasks`；`:718` 注释及 `:726-734` 实现均表明任一任务结束即返回，且不主动终止其余任务。
  - `src/workers/market_feed.rs:185-238`，`MarketFeedSupervisorHandle::{reload,stop}`；`:216-225` 启动新父任务后只 abort 旧父任务，`:231-235` 停用同样只 abort 父任务。
  - `src/workers/market_feed.rs:892-949`，`run_provider_reconnect_loop_with`；`:914-948` 是无正常退出分支的无限重连循环。
  - `src/modules/market/infrastructure/cache.rs:157-171,498-505,541-549` 显示 depth 无条件覆盖；ticker/K 线虽有 `src/modules/market/infrastructure/cache.rs:404-453` 的时间 CAS，但没有配置代际 fencing。
- **影响：** 每次后台 reload 都可能新增一组 provider 网络连接和写入者；禁用配置也不能真正停流。旧 symbol/provider 继续写 Redis/Mongo/事件，产生行情竞争、资源泄漏、供应商限流和错误深度覆盖；运维显示“success/skipped”与实际运行态不一致。
- **已防护：** ticker/K 线使用 Redis Lua 防倒退；provider 循环有上限 60 秒的退避；新配置在替换前做 adapter 校验。
- **未防护：** 无取消令牌、无 `JoinSet/abort_all`、无对子任务的 join；无 config generation/owner fencing；状态只观察父任务。
- **建议：** 采用结构化并发：监督器保存并管理所有子任务，使用 `CancellationToken + JoinSet/TaskTracker` 传播取消，`abort_all` 后等待退出；旧代际完全停止或被写入端 generation fence 拒绝后再发布新配置成功。任何 provider 异常应取消同组兄弟并让 readiness/状态失败。
- **验收：** 记录 provider 活跃连接数和写入代际，连续 N 次 reload 后每个期望 provider 仍恰好一个循环；disable 后在限定时间内连接和写入归零；旧 generation 的写入被拒；注入子任务 panic 时状态失败且能被监督器重建或触发进程重启。
- **依赖：** Tokio 任务生命周期改造；配置版本传播到 ingestion/cache；连接/写入指标与故障注入测试。
- **工作量：** M。

### P0-03 全新部署可用公开固定口令创建 active 的 `*` 权限管理员

- **状态：未防护（文档告警不属于强制控制）。**
- **问题：** 用户名或密码环境变量缺失/空白会回退到仓库公开常量；迁移器在管理员表为空时创建 active 管理员及 `JSON_ARRAY('*')` 角色。两份 Compose 示例也显式回退到同一公开口令，没有生产环境 fail-closed 检查。
- **路径与行号/符号证据：**
  - `src/bootstrap.rs:14-20,33-51`，`DEFAULT_BOOTSTRAP_ADMIN_PASSWORD`、`BootstrapAdminConfig::{built_in_defaults,from_env}`；`:34` 直接标注固定公开弱口令，`:43-50` 缺失/空值回退。
  - `src/bootstrap.rs:154-203`，`bootstrap_default_admin_while_locked`；`:183` 创建 `JSON_ARRAY('*')` 角色，`:192-199` 创建 active 管理员。
  - `docker-compose.example.yml:93-102` 与 `docker-compose.1panel.example.yml:39-49` 为 migrator 注入公开默认账号/密码。
  - `docs/deployment/docker.md:229-252` 仅在文档中要求生产覆盖，且说明数据库已有管理员后改环境变量不会重置密码。
- **影响：** 未正确覆盖环境变量的首发生产环境具备已知口令的通配权限账号，可直接接管权限、配置、资金和密钥；错误一旦发生，后续修正部署变量不会自动消除该账号风险。
- **已防护：** 只在 `admin_users` 为空时创建；MySQL 命名锁与事务防止多实例重复；口令经 Argon2 哈希；敏感值不写日志。
- **未防护：** 无生产模式识别；无已知默认值拒绝；无首次登录强制轮换/过期；Compose 默认值使误配置静默成功。
- **建议：** 生产迁移必须显式提供非默认一次性 secret，缺失、空值或等于已知默认值立即非零退出；更优方案是从 secret manager 注入随机一次性口令并设置首次登录强制轮换与短期过期。开发默认值只能由显式 local profile 开启。
- **验收：** production 模式下缺失、空白或公开默认口令的迁移均失败且 API 不启动；随机 secret 可成功创建一次；首次登录必须轮换；集成测试证明生产路径无法创建公开默认口令的 `*` 管理员。
- **依赖：** 明确 `APP_ENV/BOOTSTRAP_MODE`；1Panel/Compose secrets 流程；管理员首次登录轮换能力。
- **工作量：** S/M。

### P1-01 外部行情时间戳无未来偏移上限，可毒化 freshness CAS；REST 缺时间时把旧数据标成本机“现在”

- **状态：部分防护。**
- **问题：** provider 时间原样进入快照和 Redis CAS；CAS 只比较“是否更大”，不限制未来时间。资金消费者只检查低于当前时间的陈旧下界，未来时间会长期被判新鲜并阻止后续正常快照。REST 兜底缺 `ts` 时使用 `Utc::now()`，无法表达上游数据真实年龄。
- **路径与行号/符号证据：** `src/modules/market/domain.rs:247-288`（`MarketTickerSnapshot::with_24h` 不检查 freshness）；`src/modules/market/infrastructure/cache.rs:404-419`（`SAVE_TICKER_IF_FRESH_SCRIPT` 仅按时间递增）；`src/modules/market/infrastructure/adapters/provider.rs:968-1000,1024-1032`（外部时间解析及缺失时回退本机时间）；资金读取只做下界检查见 `src/modules/spot/infrastructure/market_prices.rs:46-53`、`src/modules/margin/infrastructure/market_data.rs:92-122`、`src/workers/seconds_contract_settlement.rs:392-423`、`src/workers/margin_liquidation.rs:552-573`。
- **影响：** 单个远未来时间戳可让错误价格长期驻留并被订单、秒合约结算和强平读取；REST 缓存旧响应可被不断包装成“最新”，造成错误动账或风控决策。
- **已防护：** 价格正数/交易对格式校验；ticker/K 线原子防倒退；资金路径有约 60 秒陈旧下界。
- **未防护：** 无未来偏移上限、receive time、来源可信度标记和 provider 隔离；金融消费者不拒绝未来时间。
- **建议：** 分开持久化 `provider_observed_at` 与 `received_at`，按 provider 合同限制 `[received_at-max_age, received_at+max_future_skew]`；异常来源隔离并告警；缺上游时间的 REST 数据标记 untrusted，不能默认参与资金动作。
- **验收：** 注入远未来快照不会修改 Redis，随后正常快照可写入；所有资金消费者拒绝未来偏移超限数据；缺时间 REST 数据的可用策略有显式测试和指标。
- **依赖：** provider 时间合同/SLO；缓存 DTO 兼容迁移；四条资金消费路径统一 freshness policy。
- **工作量：** M。

### P1-02 MQ 的发布确认、拓扑、坏消息隔离与 inbox 重放未形成闭环

- **状态：部分防护。**
- **问题：** outbox 发布未启用 publisher confirms，`basic_publish` 返回后即标记 `published`；默认 Compose 开启 outbox，却不配置 inbox queue，代码也不声明 queue/binding。坏 JSON 在进入 inbox 前直接 ACK，仅留日志；inbox `dead_letter` 被永久视为 duplicate，管理 API 只有 outbox 重排；consumer 没有显式 QoS/prefetch。
- **路径与行号/符号证据：**
  - `src/modules/events/service/rabbitmq.rs:1-8,107-139`，`RabbitMqOutboxPublisher::publish` 明确未调用 `confirm_select`；`src/modules/events/service/outbox.rs:134-177` 发布返回 `Ok` 后 `mark_published`；`src/modules/events/infrastructure.rs:367-382` 再次注明不能解释为 broker 已持久接收。
  - `src/modules/events/service/rabbitmq.rs:141-155,189-211` 要求队列预先声明，但代码只 `basic_consume`；仓库未找到 queue declare/bind 实现。`src/workers/event_inbox.rs:66-89` 在 `EVENT_INBOX_QUEUE_NAME` 缺失时静默停用；`src/config.rs:247-256` 的 outbox 默认开启，而 `docker-compose.example.yml:1-20`、`docker-compose.1panel.example.yml:7-31` 未传 inbox queue。
  - `src/modules/events/service/rabbitmq.rs:224-258` 中 malformed delivery 走 ACK；`src/modules/events/infrastructure.rs:781-818` 把 dead letter 终态视为 duplicate；`src/modules/events/routes.rs:47-64` 与 `src/modules/events/application.rs:250-272` 只提供 outbox requeue。
- **影响：** broker 未实际接管或消息无法路由时，数据库仍永久标记已发布；全新 Compose 可持续产生无消费者/无绑定事件。坏消息无持久取证，inbox 死信无法受控恢复；无限 prefetch/串行慢消息会放大内存和队首阻塞。
- **已防护：** 业务事务与 outbox 原子提交；消息 `delivery_mode=2`、durable exchange；inbox 唯一键、处理租约、ACK-after-state 和 5 次重试；用户钱包初始化 handler 本身幂等。
- **未防护：** broker ACK、mandatory/return、代码或 IaC 中的 queue/binding/DLX、坏消息持久隔离、inbox 审计重放、显式 prefetch、指数退避/jitter。
- **建议：** channel 开启 confirms，仅收到 Ack 才标记 published，Nack/超时进入重试；使用 mandatory 并处理 unroutable；代码或 IaC 声明 durable queue/binding/DLX；坏消息写 quarantine 表/队列；补 inbox 带审计重放；设置有界 prefetch 和按错误分类的退避。
- **验收：** broker kill/断网/无 binding 测试中没有任何未获 Ack 的行进入 `published`；fresh Compose 注册用户后事件可达绑定队列并完成 handler；坏消息可查询；inbox 死信可审计重放且只产生一次副作用；压力测试内存有界。
- **依赖：** RabbitMQ topology/IaC；lapin confirm/return 处理；运维告警和管理端重放 API。
- **工作量：** M/L。

### P1-03 所有 worker 与 API 同进程、随每个副本启动且无顶层监督，prediction 明确没有多实例锁

- **状态：部分防护。**
- **问题：** `main` 以 fire-and-forget 方式 spawn 全部任务，JoinHandle 不受监督；HTTP 成功与 worker 存活无关。扩容 API 会同步复制所有轮询和上游连接。prediction 在每个 MySQL 可用的副本中无条件启动轮询，代码明确要求部署侧单实例，但同步包含真实资金结算副作用。
- **路径与行号/符号证据：** `src/main.rs:1-4,59-311`（所有任务均 `tokio::spawn`，错误只记日志），`src/main.rs:275-281`（每个实例启动 prediction loop）；`src/modules/prediction/application.rs:47-69`，`run_due_sync_once:54` 明确无锁且要求部署单实例；`src/modules/prediction/infrastructure.rs:568-579,668-678` 明确同步无锁/判重、逐条提交并可能自动结算真实动账。
- **影响：** API 水平扩容会放大 DB 扫描、Rabbit 发布、供应商连接和同步请求；prediction 可并发拉取/结算；关键任务 panic 或返回后 HTTP `/health` 仍正常，结算、强平、解禁、链任务可长期停摆而不被编排器发现。
- **已防护：** 多数资金 worker 在数据库使用状态条件、行锁或唯一键；wallet chain 有 claim 可见性，synthetic market 有租约/fencing；终态重放通常幂等。
- **未防护：** API/worker 角色隔离；全局 owner lease；顶层 `JoinSet`/重启策略；worker heartbeat/readiness；prediction 并发互斥。
- **建议：** 拆分 `api` 与 `worker` 运行角色/二进制，按队列命名 worker set；资金/上游同步任务使用数据库或 Redis 租约并带 fencing；用结构化监督收集 JoinHandle，关键任务退出即 readiness 失败或进程退出；优雅停机等待租约和任务释放。
- **验收：** 2 个 API + 1 个 worker 部署中只出现一个 prediction/market owner，API 扩容不增加任务调用；注入 worker panic 后 readiness 失败并自动恢复/重启；owner 宕机后租约在 SLO 内转移且不重复动账。
- **依赖：** 部署拓扑、worker role 配置、租约表/Redis key、健康检查和指标。
- **工作量：** L。

### P1-04 worker 重试模型不一致：代理佣金瞬态错误被进程内永久屏蔽，理财坏记录可持续占据有界首页

- **状态：部分防护。**
- **问题：** 代理佣金任意 Conflict/基础设施错误都会加入内存 `failed_ids`，直到重启或集合整体清空才再尝试；理财自动赎回按最老 500 条有界扫描，失败不改变状态/next retry，足量 poison 记录会每轮占满首页并饿死后续到期项。
- **路径与行号/符号证据：** `src/workers/agent_commission_settlement.rs:32-52`（`AgentCommissionSettlementGuard`）、`:79-100`（所有错误 `record_failure`）、`:106-131`（同一 guard 贯穿进程生命周期）；`src/workers/earn_auto_redemption.rs:131-163`（失败只日志并继续）、`:186-203`（`status='subscribed' ORDER BY ... LIMIT <=500`）。
- **影响：** 短暂 DB/锁错误可能使合法佣金在进程生命周期内不再结算；重启后失败项突发重试。大量固定失败的理财记录会让更新记录无法进入扫描，造成资金长期滞留；没有统一死信/人工处置视图。
- **已防护：** 权威结算用例以数据库状态/事务保证幂等；代理自动结算默认关闭（`src/config.rs:367-370`）；理财每项独立事务且提交后才广播。
- **未防护：** 持久化 attempt/next_retry_at/error_class/dead_letter；指数退避与 jitter；公平分页/跳过未到期重试项；统一 worker 运维面板。
- **建议：** 使用持久化作业状态；瞬态错误指数退避，确定性错误进入 dead letter；按 `next_retry_at,id` 扫描并支持审计重排；删除永久内存 guard，或只把它用作短 TTL 熔断而不改变持久语义。
- **验收：** 注入一次瞬态错误可在无重启情况下自动恢复；固定 poison 达预算后进入 dead letter 且不阻塞后续记录；重放只结算一次；积压/最老年龄可告警。
- **依赖：** worker job schema/迁移、错误分类、管理端重放和指标。
- **工作量：** M。

### P1-05 行情配置与 WS 都是进程内状态，多副本下配置成功与实时推送不具备全局语义

- **状态：部分防护。**
- **问题：** 行情 supervisor 的 `applied_version` 和任务句柄只在当前进程；管理请求只 reload 命中的实例却把数据库版本标为 success。启动读取数据库配置失败时回退环境配置。WS hub 也只在当前进程，lag 分支跳过消息；连接到其他副本的客户端收不到本副本外产生的事件。
- **路径与行号/符号证据：** `src/workers/market_feed.rs:144-227`（进程内 status/task）；`src/modules/admin/application/market_feed.rs:168-239`（当前 supervisor reload 后写成功）；`src/modules/admin/infrastructure/market_feed.rs:265-283`（全局版本状态）；`src/main.rs:59-93`（DB 读取错误降级到 env）；`src/state.rs:86-97` 明确 broadcast hub 仅当前进程有效；`src/modules/events/service/websocket.rs:499-538,548-589` 为有界 broadcast 和 lag 跳过。
- **影响：** 多副本可能分别运行不同 provider/symbol 版本，但后台显示全局 success；DB 暂时故障会使实例以旧 env 配置写共享 Redis。用户 WS 会随机漏实时行情/私有事件，lag 没有持久补发或可见指标。
- **已防护：** WS 明确定义为 best-effort，客户端规范要求 REST 对账；hub 有界避免无限内存；配置版本写库并可查询。
- **未防护：** 每实例版本 ACK、配置广播/watch、全局 singleton/fencing、跨实例 pub/sub、lag 指标和客户端断档信号。
- **建议：** 行情服务独立单例或让每实例 watch 配置并上报 instance/version ACK；版本未全量收敛前不显示 success。WS 使用共享 broker/Redis Streams 等跨实例 fan-out，或在协议中提供 sequence/resume；lag 时发送 resync-required 并计数。
- **验收：** 两实例同时确认同一版本，禁用后全部停止；DB 配置读取失败时 readiness 失败而非回退不同配置；在任意实例产生事件，连接任意副本均可收到或收到明确 resync 信号；lag 指标可告警。
- **依赖：** 实例身份与配置 ACK 存储、共享实时总线、前端重连/对账协议。
- **工作量：** L。

### P1-06 环境变量存在多入口和 Compose 透传漂移，关键 worker 开关可被静默默认值覆盖

- **状态：部分防护。**
- **问题：** 一部分配置由 typed `Settings` 解析，另一部分 worker 直接 `std::env` 读取并在非法值时静默回退。Compose 使用显式 environment map，但没有 `env_file` 透传所有源码消费项；例如 inbox、wallet chain、loan 等关键开关不在示例 map。历史 `KLINE_RECOVERY_*` 又被复用为 synthetic market 语义。
- **路径与行号/符号证据：** `src/config.rs:15-123`（中心 Settings），`src/workers/event_inbox.rs:66-75` 与 `src/workers/wallet_chain.rs:43-52,748-773`（直接 env 且非法值回默认）；`src/config.rs:277-292`、`src/main.rs:123-156`（KLINE_RECOVERY 语义复用）；`docker-compose.example.yml:1-20` 与 `docker-compose.1panel.example.yml:7-31` 只显式传递部分变量。Docker Compose 的 `.env` 插值不会自动使未列入 service environment 的键进入容器。
- **影响：** 运维以为在 env 文件关闭/调整 worker，实际容器未收到变量而按默认开启；拼写错误/非法布尔值不报错；inbox 可静默关闭、wallet chain 可默认开启；旧变量名使容量与职责判断错误。
- **已防护：** 强制连接串由 `Settings` 解析失败即退出；部分数值有 clamp；SecretString 避免 Debug 泄漏。
- **未防护：** 单一配置 schema、未知/非法变量校验、源码—env example—Compose 一致性测试、弃用迁移和完整透传。
- **建议：** 所有生产配置收敛到 typed schema；关键值非法必须启动失败；从 schema 生成 env example/Compose map 和文档；CI 检查每个消费键都有声明、默认、敏感级别和部署映射；为旧 KLINE 名提供有期限告警与新变量迁移。
- **验收：** 合同测试枚举生产源码消费键并与两份 Compose/schema 对齐；任一关键变量拼写/类型错误均非零退出；env 文件可实际控制全部 worker；弃用变量使用时有明确告警。
- **依赖：** 配置模块重构、部署模板、兼容期策略。
- **工作量：** M。

### P1-07 健康检查、日志指标和错误边界不足，依赖/worker 失效仍显示健康且 5xx 可能泄漏底层细节

- **状态：部分防护。**
- **问题：** `/health` 恒定返回 ok；Mongo `connect` 只构造句柄不 ping；worker 无心跳。日志为文本 tracing，没有指标端点/Prometheus/OTel；event “metrics”仅日志。`AppError::IntoResponse` 将包含底层错误的 Display 直接返回客户端。
- **路径与行号/符号证据：** `src/lib.rs:84-94`（恒定 health）；`src/infra/mongo.rs:15-20`（不执行 ping）；`src/main.rs:1-4,313-317`（worker 退出不影响监听）；`src/main.rs:34-37`、`src/bin/exchange-migrate.rs:22-25`（文本日志）；`src/workers/event_inbox.rs:203-218`（计数只写日志）；`src/error.rs:15-48,131-147`（底层错误参与 Display 并作为响应 body）。
- **影响：** Docker/1Panel 无法发现 DB、Redis、Rabbit、行情或结算任务故障；积压、死信、重试年龄、租约、WS lag、价格陈旧度和提现未知态没有可告警指标；客户端可看到 schema/约束/拓扑/上游错误文本，增加信息泄漏风险。
- **已防护：** 启动时 MySQL/Redis/Rabbit 初连失败会退出；HTTP 有 TraceLayer；1Panel 示例有 json-file rotation；日志多处使用结构化字段。
- **未防护：** liveness/readiness 分离；依赖有界检查；worker heartbeat；统一 JSON/request_id；指标、SLO 和告警；通用 5xx 脱敏。
- **建议：** 保留轻量 liveness，新增 readiness 检查依赖、迁移版本、关键 worker heartbeat/行情 owner；导出 Prometheus/OTel 指标和 JSON 日志；所有 5xx 对外返回稳定错误码+request_id，底层 cause 仅内部日志记录并脱敏。
- **验收：** 断开各依赖/杀死 worker 后 readiness 在 SLO 内失败，liveness 仍反映进程；关键 backlog/oldest-age/dead-letter/lag 指标可抓取并触发测试告警；API 测试断言 5xx 不含 SQL、URL、token 或供应商原文且可按 request_id 定位内部日志。
- **依赖：** metrics/tracing stack、编排器探针、告警平台和错误码合同。
- **工作量：** M/L。

### P1-08 Docker/1Panel 缺资源与高可用约束，Redis/单实例依赖形成大故障域；密钥无版本化轮换

- **状态：部分防护。**
- **问题：** Compose 服务为单实例且未声明 CPU/memory/pids 等限制，默认镜像标签含 `latest`；Redis 同时承载会话、行情、限频和 worker 协调。MySQL pool 固定 10 连接。凭据 AES-GCM 主密钥为单一 32 字节值，更换后旧密文不可读；本地实际 1Panel 文件存在内联敏感配置和非 loopback 端口绑定（报告不抄录值）。
- **路径与行号/符号证据：** `docker-compose.example.yml:22-132`、`docker-compose.1panel.example.yml:39-73`（无资源/副本/安全约束且默认 latest）；`src/config.rs:143-157`（Redis 共享故障域及密钥更换不可逆说明）；`src/infra/mysql.rs:9-24`（pool 固定 10/5s）；`src/infra/secrets.rs:29-75,103-114`（单 key AES-GCM 与精确 32 字节）；`docker-compose.1panel.yml:90-94`（本地实际端口绑定范围，敏感值未引用）。
- **影响：** 任一容器可耗尽宿主资源；Redis 故障同时导致会话、价格、协调和风控路径失效；API 扩容按每实例 10 连接放大 MySQL 压力；主密钥遗失/轮换会使 SMTP/provider 凭据不可恢复；实际端口若缺防火墙可扩大暴露面。
- **已防护：** Dockerfile 使用非 root、tini/supervisor 和镜像内 healthcheck（`Dockerfile:49-80`）；1Panel 示例默认 loopback 并有日志轮转；secrets 使用 AES-GCM 且不明文回退；敏感部署文件在 `.gitignore/.dockerignore` 中。
- **未防护：** 资源 limits/reservations、HA/failover、Redis 职责隔离、连接池配置化、key id/双读轮换/托管 secret、生产部署自动安全校验。
- **建议：** 设置 memory/CPU/pids、只读根文件系统、cap drop 和容量基线；生产使用 HA MySQL/Redis/Rabbit/Mongo 并拆分会话/行情/锁域；连接池按实例数预算；密文 envelope 加 `key_id/version`，支持双读单写轮换和 escrow；1Panel 用 secrets 注入并强制 loopback/防火墙。
- **验收：** 资源压力下单容器被限制且宿主保持可用；Redis/数据库节点故障演练满足 RTO；API 扩容不超过连接预算；旧新 key 共存迁移后所有密文可读且可撤旧 key；部署 lint 拒绝 latest、广域绑定和明文 secret。
- **依赖：** 生产基础设施/1Panel 能力、secret manager、容量压测和密钥恢复流程。
- **工作量：** L（代码部分 M，基础设施另计）。

### P1-09 仓库没有多存储一致性备份/恢复实现或演练门禁

- **状态：未防护（0099 迁移有一次性人工说明，但不是持续备份体系）。**
- **问题：** 仓库未找到 MySQL/Mongo/Rabbit/Redis/uploads/加密主密钥的定时备份脚本、保留策略、校验、恢复编排或常态演练记录。Compose 仅声明数据卷；文档只针对 0099 维护窗口要求人工全库备份和隔离恢复。
- **路径与行号/符号证据：** 数据卷见 `docker-compose.example.yml:40-41,62-63,75-76,90-91,124-132`；0099 人工备份/恢复要求见 `docs/deployment/docker.md:98-120,210-227`；应用镜像回滚不回滚 schema 见 `docs/deployment/docker.md:331-338`。仓库静态检索未发现通用 backup/restore/PITR/runbook 服务或脚本。
- **影响：** RPO/RTO 未定义；只恢复 MySQL 可能与 Mongo K 线、Rabbit 队列、Redis 协调状态和 uploads 时间点错位；丢失 `CREDENTIAL_ENCRYPTION_KEY` 即使恢复数据库也无法解密运营凭据；事故中只能临时手工操作。
- **已防护：** 持久服务使用 named volumes；Redis 示例启用 AOF；0099 文档要求停写、验证备份恢复并禁止手工逆 ALTER。
- **未防护：** 自动备份、异地/不可变保留、PITR、密钥 escrow、跨存储恢复顺序/对账、定期恢复演练和 RPO/RTO 证据。
- **建议：** 定义分层 RPO/RTO；MySQL 全量+binlog PITR、Mongo 一致性备份、Rabbit topology/策略与必要队列保护、uploads 快照、密钥 escrow；在隔离环境自动恢复并运行资金、outbox/inbox、K 线和文件引用对账。
- **验收：** 从一套指定时间点备份在空环境恢复，记录实际 RPO/RTO；钱包总账/余额、事件状态、K 线唯一键、文件引用和密钥解密检查全部通过；恢复演练按周期自动执行并保留报告。
- **依赖：** 存储提供商快照/PITR、对象存储/KMS、恢复环境和业务对账脚本。
- **工作量：** L/运维项目。

### P1-10 CI 只构建/发布镜像，缺测试与迁移兼容门禁；依赖和发布引用可变且没有可验证回滚链

- **状态：部分防护。**
- **问题：** 唯一 GitHub workflow 在 PR 只 build，push/manual 可发布；没有 fmt/clippy/unit/integration、前端质量、fresh/upgrade migration matrix、Compose smoke、漏洞扫描、SBOM/签名/attestation。Actions 仅固定 major tag，基础镜像和默认应用镜像使用可变 tag；没有 environment approval/concurrency。schema 迁移后应用回滚依赖人工判断兼容。
- **路径与行号/符号证据：** `.github/workflows/docker-image.yml:15-50`（PR build）、`:51-168`（push/manual publish）、`:35,40,43,71,76,79,87,102,121,128,131,139`（action major tags）；`Dockerfile:3-6,19,38`（可变基础镜像）、`:10-17,27-36`（只执行 npm build 与 `cargo build --locked --release`）；`docker-compose.example.yml:94,108` 与 `docker-compose.1panel.example.yml:41,55` 默认 `latest`；`docs/deployment/docker.md:331-338` 明确应用回滚不回滚 schema。
- **影响：** 业务测试、数据约束或迁移错误仍可发布；action/base tag 被重指会改变供应链输入；无法证明镜像来源/SBOM；并行发布可互相覆盖；旧应用与新 schema 不兼容时回滚失败，扩大停机窗口。
- **已防护：** Rust 使用 `--locked`；npm 使用 `npm ci`；workflow 生成 SHA tag 并核验 manifest digest（`.github/workflows/docker-image.yml:85-105,137-165`）；runtime 非 root；0099 有人工维护窗口说明。
- **未防护：** 必需质量 job、迁移兼容矩阵、full-SHA action pin/base digest、依赖审计、SBOM/签名/来源证明、受保护 release environment、发布 concurrency、自动 canary/rollback 验证。
- **建议：** PR 必须通过 fmt/clippy/test、后端集成、Web/PC/mobile 对应门禁、fresh DB 与“上一生产版本 schema -> head”迁移、Compose smoke；加 cargo deny/audit 与 npm audit 策略；actions pin full SHA、base pin digest；生成 SBOM/provenance/signature；只允许受保护 tag/environment 发布 immutable digest，并演练 expand-contract 回滚。
- **验收：** 任一测试/迁移失败阻止镜像发布；全新库和上一生产快照均可迁移；旧应用在新 schema 兼容窗口内 smoke 通过；镜像可验证签名/attestation/SBOM；同环境发布串行；canary 失败自动回到已验证 digest。
- **依赖：** CI 数据库服务与生产前一版 fixture、GitHub environment/branch rules、registry 签名能力和发布编排。
- **工作量：** M/L。

### P1-11 迁移执行顺序有门禁，但关键异步/资金状态机的数据库约束不完整，兼容验证主要靠人工

- **状态：部分防护。**
- **问题：** 独立 migrator 和 Compose 启动顺序是正确方向，但 103 个 migration 没有自动 fresh/upgrade/旧应用兼容测试。0099 包含大量 DDL，MySQL 隐式提交导致部分失败无法事务回滚。关键 event 与 wallet chain 表的 status、retry_count、金额/确认数关系仍主要靠应用代码约束。
- **路径与行号/符号证据：** `src/bin/exchange-migrate.rs:13-56`（嵌入并先执行 migrations，随后 bootstrap）；`docker-compose.example.yml:93-121`（MySQL healthy -> migrate complete -> API）；`docs/deployment/docker.md:98-120,200-227`（0099 的 96 张表 ALTER、隐式提交、dirty 修复与恢复要求）。约束缺口见 `migrations/0008_events_risk_audit.sql:1-33`（event status/retry 无 CHECK）、`migrations/0009_event_inbox_retry_count.sql:1-2`；`migrations/0087_p0_financial_safety.sql:11-63,65-107`（提现/充值唯一键与 FK 齐全，但 status、amount>0、`total_reserved=amount+fee`、确认数关系无 CHECK）。
- **影响：** 应用 bug、人工 SQL 或未来脚本可写入负 retry、非法状态、非正金额、预留额不守恒等状态；worker 扫描可能遗漏或错误推进。大迁移失败/回滚时只能依赖人工 runbook，发布前无法证明新旧版本兼容。
- **已防护：** wallet_accounts 有非负 CHECK、唯一键和 FK（`migrations/0003_assets_wallet_ledger_locks.sql:11-23`）；锁仓金额有 CHECK（`:45-77`）；提现/充值有幂等唯一键/FK；链事件死信 kind 有 CHECK（`migrations/0089_wallet_chain_event_dead_letters.sql:1-18`）；API 本身不自动迁移。
- **未防护：** 状态枚举/转移及关键数值关系的 DB 最后一层约束；自动 schema compatibility gate；通用 migration rollback/restore rehearsal。
- **建议：** 先做数据审计，再分阶段增加 status CHECK、retry_count>=0、amount/fee/required_confirmations 合法、`total_reserved=amount+fee` 等可表达约束；复杂转移继续由事务应用层负责。CI 执行 fresh、上一生产快照 upgrade、重复运行和旧应用/新 schema smoke；大 DDL 走 expand-contract/online DDL 评审。
- **验收：** 直接 SQL 注入每类非法状态/金额均被 DB 拒绝；历史数据清洗报告为零异常；fresh/upgrade/re-run 测试通过；大迁移中断后的恢复演练按 runbook 成功；旧应用在声明兼容窗口可运行。
- **依赖：** 生产数据画像、MySQL 版本/online DDL 能力、上一生产快照 fixture、应用状态常量清单。
- **工作量：** M/L。

## 启动顺序与运行拓扑

| 阶段 | 当前行为 | 防护判断 | 主要证据 |
|---|---|---|---|
| 完整 Compose | MySQL/Mongo/Redis/Rabbit health 后运行 migrator；migrator 成功后启动 API | **已防护**，但使用可变镜像，health 只覆盖启动时依赖 | `docker-compose.example.yml:22-121` |
| 1Panel | 只保证 migrator 完成后 API 启动；外部 MySQL/Mongo/Redis/Rabbit 不在编排内 | **未防护**，外部 readiness/顺序由人工负责 | `docker-compose.1panel.example.yml:39-73`; `docs/deployment/docker.md:335-338` |
| API 进程 | 顺序建立 MySQL、Mongo handle、Redis、auth、Rabbit，再 spawn workers，最后监听 | **部分防护**；Mongo 未 ping，spawn 不等于 ready | `src/main.rs:25-57,313-317`; `src/infra/mongo.rs:15-20` |
| 行情 | DB 配置在 detached task 中异步加载，失败回退 env；HTTP 不等待行情 ready | **未防护** | `src/main.rs:59-94` |
| MQ inbox | 仅设置 queue name 才启动；缺失视为正常停用 | **未防护**，与默认开启 outbox 不成对 | `src/main.rs:284-311`; `src/workers/event_inbox.rs:66-89` |
| 健康 | Docker 探测恒定 `/health` | **仅 liveness 已防护**，无 readiness | `Dockerfile:77-78`; `src/lib.rs:89-94` |

## Worker 重复执行、幂等与重试概览

| Worker/链路 | 多副本现状 | 已防护 | 残余风险 |
|---|---|---|---|
| wallet chain | 每 API 副本启动 | 条件 claim、可见性窗口、request/event 唯一键 | P0-01 歧义结果释放；默认开启且配置漂移 |
| market feed | 每副本 supervisor，每 provider 再 spawn | ticker/Kline CAS、重连退避 | P0-02 子任务泄漏；无全局 owner/config fencing |
| prediction sync | 每副本 30 秒轮询 | market external id upsert | 明确无锁，可能触发资金结算 |
| outbox | 每副本扫描同一批，允许重复 publish | idempotency/message_id、下游 inbox 去重 | 无 claim 和 publisher confirms，存在重复与丢失窗口 |
| inbox | 配置 queue 后每副本消费 | broker 分发、DB 唯一键、租约、ACK-after-state | topology、坏消息、死信重放、QoS 缺口 |
| seconds/earn/unlock/margin/loan | 每副本扫描 | 主要使用行锁、状态条件、唯一引用，资金事务原子 | retry/dead-letter 不统一，poison 可能阻塞有界首页 |
| synthetic market | 每副本扫描 | Redis lease/fencing、Redis CAS | Redis 单点、旧配置/owner 可观测性 |
| agent commission | 默认关闭；开启后每副本扫描 | 权威事务和状态幂等 | 进程内失败 guard 抑制瞬态重试 |

## 数据约束与迁移兼容概览

- 截至审计日，`migrations/` 有 103 个 SQL 文件，版本范围 `0001`–`0106`；编号空档本身不是 SQLx 错误，不应据此重编号历史 migration。
- **已防护：** 独立迁移二进制；API 不隐式迁移；完整 Compose 使用 `service_completed_successfully`；钱包余额非负、业务唯一键、主要 FK、链事件死信 kind 等已有约束。
- **未防护：** CI 未验证 fresh/upgrade/re-run/旧应用兼容；0099 大 DDL 只能人工维护；event/充提状态与关键数值关系的 CHECK 不完整；没有通用 down migration，应用回滚必须保持 schema 向后兼容。

## 行情与 WebSocket 已有设计边界

- `src/modules/events/mod.rs:3-14` 已明确区分持久 outbox/inbox 与进程内 best-effort WS；事务提交后再广播是正确保护。
- `.trellis/spec/backend/realtime-websockets.md:109-119` 要求客户端 REST 对账，能降低漏推造成的最终一致性风险。
- 这些保护不解决多实例 fan-out、lag 可观测性、配置代际和金融价格来源可信度，相关缺口见 P0-02、P1-01、P1-05。

## Files Found

- `src/main.rs` — 依赖连接、所有 worker 启动顺序和 detached task 生命周期。
- `src/config.rs` — typed 环境配置、worker 默认值、Redis/密钥故障域说明。
- `src/bootstrap.rs` — 初始管理员默认值、命名锁、通配角色和 active 账号创建。
- `src/bin/exchange-migrate.rs` — SQLx migrator 与 bootstrap 的独立启动入口。
- `src/workers/wallet_chain.rs` — 提现 claim、广播重试、失败解冻和链事件游标/死信。
- `src/modules/wallet/infrastructure/withdrawals.rs` — 网关不确定性合同与资金释放事务。
- `src/workers/market_feed.rs` — provider 子任务、重连、热重载与 supervisor 状态。
- `src/modules/market/infrastructure/{cache.rs,adapters/provider.rs}` — Redis freshness CAS、depth 覆盖和 provider 时间解析。
- `src/modules/events/service/{rabbitmq.rs,outbox.rs}` — RabbitMQ 发布/消费确认语义。
- `src/modules/events/{application.rs,infrastructure.rs,routes.rs}` — inbox/outbox 重试、去重、死信和管理入口。
- `src/workers/{event_inbox.rs,agent_commission_settlement.rs,earn_auto_redemption.rs}` — 消费启动、补偿扫描及 worker 重试差异。
- `src/modules/prediction/{application.rs,infrastructure.rs}` — 无锁同步与可能的自动资金结算。
- `src/lib.rs`, `src/error.rs`, `src/infra/{mysql.rs,mongo.rs,redis.rs,secrets.rs}` — 健康、错误响应、连接与密钥边界。
- `migrations/0003_assets_wallet_ledger_locks.sql` — 钱包/锁仓核心 CHECK、唯一键和 FK。
- `migrations/0008_events_risk_audit.sql`、`0009_event_inbox_retry_count.sql` — event 状态与 retry schema。
- `migrations/0087_p0_financial_safety.sql`、`0089_wallet_chain_event_dead_letters.sql` — 充提安全字段、幂等键、FK 与链死信。
- `migrations/0099_schema_wide_text_metadata.sql` — 大规模 schema 文本元数据修复 DDL。
- `Dockerfile`, `docker/supervise.sh`, `docker/nginx.conf` — 镜像构建、进程监督和入口代理。
- `docker-compose.example.yml`, `docker-compose.1panel.example.yml`, `docker-compose.1panel.yml` — 完整/外部依赖部署拓扑及本地实际配置快照。
- `.github/workflows/docker-image.yml` — 唯一镜像 build/publish workflow。
- `docs/deployment/docker.md` — 0099 维护窗口、管理员默认值、1Panel 与回滚说明。

## Code Patterns

- **资金状态机事务 + 行锁/条件更新：** `src/modules/wallet/infrastructure/withdrawals.rs:367-454`；基础正确，但外部调用结果分类错误可绕过安全前提。
- **fire-and-forget worker：** `src/main.rs:59-311`；任务退出只记录日志，不参与进程生命周期和 readiness。
- **嵌套 spawn 后丢弃 JoinHandle：** `src/workers/market_feed.rs:657-735`；是热重载泄漏的直接模式。
- **DB outbox + inbox 去重：** `src/modules/events/service/outbox.rs:134-177`、`src/modules/events/infrastructure.rs:781-818`；具备应用级至少一次骨架，但 broker publish 端缺 confirm。
- **直接环境读取并静默默认：** `src/workers/wallet_chain.rs:748-773`；与 `Settings::from_env` 的 fail-fast 语义不一致。
- **进程内 best-effort hub：** `src/state.rs:86-97`；单实例简单，多实例需共享 fan-out 或明确 sequence/reconcile。

## External References

- [Tokio `JoinHandle` documentation](https://docs.rs/tokio/latest/tokio/task/struct.JoinHandle.html) — JoinHandle 被 drop 后任务继续后台运行；支持 P0-02 的 detach 判断。
- [RabbitMQ Consumer Acknowledgements and Publisher Confirms](https://www.rabbitmq.com/docs/confirms) — publisher confirms 是发布端数据安全机制；prefetch 为 0 表示无上限并可能造成消费者内存增长。
- [RabbitMQ Dead Letter Exchanges](https://www.rabbitmq.com/docs/dlx) — DLX 触发条件、策略配置和 dead-letter 安全边界。
- [RabbitMQ Reliability Guide](https://www.rabbitmq.com/docs/reliability) — 网络歧义、publisher confirm 与重复处理要求。
- [Docker Compose startup order](https://docs.docker.com/compose/how-tos/startup-order/) — `service_healthy` 与 `service_completed_successfully` 的启动门禁语义。
- [Docker Compose environment variables](https://docs.docker.com/compose/how-tos/environment-variables/set-environment-variables/) — `.env`/shell 值必须通过 service `environment` 或 `env_file` 显式进入容器，敏感信息宜用 secrets。
- [Docker resource constraints](https://docs.docker.com/engine/containers/resource_constraints/) — 容器默认没有资源限制。
- [Docker volumes backup/restore](https://docs.docker.com/engine/storage/volumes/#back-up-restore-or-migrate-data-volumes) — volume 备份、恢复和自动演练基础方法。
- [MySQL 8.4 implicit commits](https://dev.mysql.com/doc/refman/8.4/en/implicit-commit.html) — `ALTER TABLE` 等 DDL 会隐式提交，不能依赖事务整体回滚。
- [MySQL point-in-time recovery](https://dev.mysql.com/doc/refman/8.4/en/point-in-time-recovery.html) — 全量备份后通过 binlog 做 PITR。
- [GitHub Actions secure use](https://docs.github.com/en/actions/reference/security/secure-use) — 推荐第三方 action 固定完整 commit SHA。
- [GitHub artifact attestations](https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/use-artifact-attestations) — 容器/二进制 build provenance 与 SBOM attestation。
- [GitHub deployment environments](https://docs.github.com/en/actions/concepts/workflows-and-actions/deployment-environments) 与 [concurrency](https://docs.github.com/en/actions/concepts/workflows-and-actions/concurrency) — 审批、分支限制、secret gate 和串行发布。

## Related Specs

- `.trellis/spec/backend/database-guidelines.md:35-73,93-200` — migration、约束、事务和兼容要求。
- `.trellis/spec/backend/container-delivery.md:92-93,186-229` — 1Panel 外部依赖责任与容器交付验证。
- `.trellis/spec/backend/quality-guidelines.md:102-115` — Rust 质量门禁要求。
- `.trellis/spec/backend/realtime-websockets.md:109-119` — WS best-effort 与 REST 对账合同。
- `.trellis/spec/backend/logging-guidelines.md:7-51` — 当前日志规范仍为空模板，尚不能提供项目级字段/脱敏/指标合同。
- `.trellis/spec/backend/directory-structure.md`、`.trellis/spec/backend/error-handling.md` — 基础设施边界与错误处理相关约定。

## Caveats / Not Found

- 本次是静态只读审计；未连接生产 MySQL/Mongo/Redis/RabbitMQ，未启动 Docker/1Panel，未执行故障注入、迁移或恢复演练，也未运行 GitHub Actions。
- 仓库外可能已有 1Panel 健康检查、云备份、WAF、KMS、告警、RabbitMQ policy、GitHub branch/environment protection；本报告只能把“仓库未声明”标为缺口，不能断言外部平台一定不存在。
- `docker-compose.1panel.yml` 是本地实际配置快照且被忽略；其风险用于提示部署核查，不等同于版本库公共模板。报告未复制任何敏感值。
- P0-02 的任务泄漏由代码控制流和 Tokio 官方 JoinHandle 语义直接推出，仍应通过连接数/写入代际集成测试量化现网影响。
- 迁移文件数量与版本范围是 2026-08-24 快照；编号空档未被当作缺陷。未对 103 个 SQL 文件逐条做数据分布验证，约束结论聚焦异步与资金关键表。
- 未发现通用备份/恢复脚本、metrics exporter、worker heartbeat、RabbitMQ queue/binding IaC、SBOM/签名/attestation workflow；如果这些能力由仓库外平台提供，应补充可审计链接和恢复/告警证据。
