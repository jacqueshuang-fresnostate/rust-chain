# Research: data / async / realtime re-audit

- Query: 对当前代码中的 MySQL migrations/约束、MongoDB/Redis/RabbitMQ、outbox/inbox、worker 生命周期、行情时间可信度、WebSocket 多实例恢复、备份与对账做有界静态复审，并复核历史 P1-01、P1-10、P1-11、P1-12、P1-13、P1-18、P1-20。
- Scope: internal
- Date: 2026-08-30

## Findings

### 历史项状态摘要

| 历史项 | 当前状态 | 本轮结论 |
| --- | --- | --- |
| P1-01 | **仍存在，消费侧有明显增强** | inbox 已有持久 payload、租约、retry/dead-letter 与重放，但 publisher confirm/mandatory、队列拓扑、默认 consumer 和缺钱包补偿仍未闭环。 |
| P1-10 | **仍存在，mobile 部分收敛** | 服务端仍是进程内 lossy hub；mobile 私有杠杆路径已有 REST 对账，PC/admin 及协议级断档检测仍不完整。 |
| P1-11 | **部分完成** | market-feed generation 已有 CancellationToken、JoinSet 与 fencing；其他 worker 仍由 API 进程 fire-and-forget，缺运行角色、顶层监督和统一持久 retry。 |
| P1-12 | **部分完成** | generation fencing、上游 liveness 和事件时点价格快照已增强；future skew、本机时间回填和多实例配置 ACK 仍缺失。 |
| P1-13 | **部分完成** | 新增提现/报价等 CHECK 和 fresh/re-run 测试；event/充值核心约束及必跑 upgrade/old-app migration lane 仍缺失。 |
| P1-18 | **仍存在** | `/health` 仍恒成功，worker/依赖/read-model 停摆未进入 readiness 或可抓取指标。 |
| P1-20 | **仓库内仍存在；生产状态待补证** | 仓库仅见卷、Redis AOF 和 0099 的一次性人工恢复说明，未见持续跨存储备份/恢复演练实现。 |

### DAR-P1-01 — `user.created` 的 RabbitMQ 交付仍可能“数据库显示已发布、钱包却未初始化”

- **Severity / 历史映射**：P1；P1-01 仍存在。
- **当前证据**：
  - 注册把用户、邀请关系与 `user.created` outbox 放在同一事务并提交：`src/modules/auth/application.rs:507-565::register_user_with_email_code`。
  - publisher 声明 durable topic exchange 和持久消息，但使用 `BasicPublishOptions::default()`，未调用 `confirm_select`；`basic_publish` 返回后即视为成功：`src/modules/events/service/rabbitmq.rs:107-138::RabbitMqOutboxPublisher::publish`。
  - 上层随后直接把行标为 `published`：`src/modules/events/service/outbox.rs:134-178::EventOutboxService::publish_once`。
  - consumer 只对预先存在的 queue 执行 `basic_consume`，不声明 queue/binding/DLX：`src/modules/events/service/rabbitmq.rs:141-220::RabbitMqInboxConsumer`。
  - `EVENT_INBOX_QUEUE_NAME` 缺失即正常停用：`src/workers/event_inbox.rs:58-112::EventInboxWorkerConfig`；两份示例 Compose 的 API 环境均未传该变量：`docker-compose.example.yml:1-20`、`docker-compose.1panel.example.yml:7-31`。
  - 钱包初始化本身已是幂等 `INSERT IGNORE ... SELECT assets`：`src/modules/events/infrastructure.rs:260-303::create_wallet_accounts_for_user_in_tx`；仓库静态检索未找到 `users × assets` 缺口扫描/补偿 worker。
- **可达影响**：unroutable、broker 在未确认窗口断连、或默认部署未启 consumer 时，outbox 可成为 `published`，但用户缺少部分/全部 `wallet_accounts`；该 false-success 不会再被普通 outbox retry 扫描捕获。
- **增量修复**：publisher channel 启用 confirms，只有 broker ACK 才 `mark_published`；开启 `mandatory` 并处理 returned message；以版本化 IaC 声明 durable queue、binding、DLX/policy；启用 `user.*.created` consumer 的部署角色和 readiness；增加按 `users CROSS JOIN assets LEFT JOIN wallet_accounts` 的幂等补偿与缺口指标。
- **兼容/迁移**：保留现有 exchange、routing key、message/idempotency key；先并行建立新 queue/binding 并验证消费，再将 consumer 配置改为 required；补偿只插缺行，不重置现有余额。
- **验证**：RabbitMQ 集成场景覆盖 ACK/NACK、unroutable、publish 后断线；断言未确认消息不标 `published`；fresh Compose 注册后在 SLA 内形成完整钱包矩阵；删除一个零余额账户后补偿只恢复该行且重复运行无副作用。
- **工作量/依赖**：M/L，约 5–8 天；依赖 RabbitMQ topology/policy 管理、集成环境、部署角色与 backlog 告警。
- **运行时证据 caveat**：未读取生产 RabbitMQ bindings、policy 或外部 consumer；生产可能有手工拓扑，但当前仓库默认部署和应用自身不能证明这条链闭环。

### DAR-P1-02 — WebSocket 仍无跨实例 fan-out/断档信号，PC 与 admin 不能可靠收敛

- **Severity / 历史映射**：P1；P1-10 仍存在，mobile 私有杠杆恢复仅算部分完成。
- **当前证据**：
  - `AppState` 明示 hub 只在当前进程有效：`src/state.rs:86-90::with_event_broadcast_hub`。
  - hub 只有 `channel + payload`，内存容量满、无订阅者或重启均可丢；lag 分支被静默跳过且无 cursor/sequence：`src/modules/events/service/websocket.rs:438-446::EventBroadcastMessage`、`:499-537::EventBroadcastHub`、`:548-589::{EventBroadcastMultiSubscription,EventBroadcastSubscription}::recv`。
  - PC 有固定延迟重连和重订阅，但 open/message 路径无 heartbeat watchdog、sequence 缺口或权威 REST 对账：`pc/src/api/stomp.ts:103-147::openSocket/openPrivateSocket`、`:327-373::handleMessage/handlePrivateMessage`、`:426-453::scheduleReconnect`。
  - admin ticker 只创建一次 socket，未注册 open/close/error 恢复或 REST fallback：`web/src/api/marketTickerSocket.ts:51-69::subscribeMarketTicker`。
  - mobile 杠杆已把 open/reconnect/事件归入串行 REST 对账和周期轮询：`mobile/src/core/marginAccountReconciliation.ts:48-223`，与 `.trellis/spec/backend/realtime-websockets.md:175-220` 一致。
- **可达影响**：请求落在实例 A、客户端连实例 B 时会漏私有/公共提示；慢消费者 lag 或 API 重启也无法判断漏了多少。已有 REST 对账的 mobile 财务状态最终可恢复，但 PC/admin 可能持续展示陈旧行情/状态，其他私有事件也没有统一收敛证明。
- **增量修复**：短期给所有客户端统一“WS 仅提示”合同：open、reconnect、`resync_required` 和有界周期都拉取带版本的 REST snapshot；协议增加单调 `sequence`/`snapshot_version`。中期用共享 bus/stream 做跨实例 fan-out，并在 lag/gap 时显式发 `resync_required` 和计数。
- **兼容/迁移**：新增 envelope 字段保持 additive，旧客户端可忽略；保留现有 `/ws/public|spot|margin|seconds|private` 路径；先为 PC/admin 加 REST 恢复，再切共享 fan-out，避免把消息可靠性误当业务事实。
- **验证**：2 个 API 实例中在 A 产生事件、连接 B；再注入 lag、断线、重启、重复和乱序，断言 mobile/PC/admin 最终与 MySQL/REST snapshot 一致；最后订阅释放后 socket/timer/watch 均为零。
- **工作量/依赖**：L，约 2–4 周；依赖共享总线、读模型版本、三端 lifecycle 测试与负载均衡测试环境。
- **运行时证据 caveat**：未观察生产副本数、sticky-session 或外部 pub/sub；这些设施可能降低命中概率，但当前进程内 hub 自身没有跨实例恢复能力。

### DAR-P1-03 — market-feed 内部生命周期已修复，但全局 worker 仍无运行角色和顶层监督

- **Severity / 历史映射**：P1；P1-11 部分完成。
- **当前证据**：
  - 主入口明确说明所有后台协程 fire-and-forget，退出只记日志：`src/main.rs:1-4`；行情、outbox、结算、强平、利息、链任务、prediction、inbox 等均由同一 API 进程 `tokio::spawn`：`src/main.rs:59-314::main`。
  - market-feed 已改为 generation task + CancellationToken + join + fence：`src/workers/market_feed.rs:303-518::MarketFeedSupervisorHandle`，provider 子任务由 JoinSet 收集：`:954-1040::run_config_loop_with_generation`。这关闭了历史旧 generation 泄漏，但没有覆盖其他 worker。
  - prediction 定时同步的到期判断仍不加锁，并明确要求部署侧单实例：`src/modules/prediction/application.rs:36-69::run_sync_loop/run_due_sync_once`。
  - 理财失败项会继续处理，但仍固定扫描最老 500 条且无持久 `next_retry_at`：`src/workers/earn_auto_redemption.rs:131-203::run_once_with_broadcast/fetch_due_subscriptions`；佣金 worker 仍用进程内失败集合改变重试语义：`src/workers/agent_commission_settlement.rs:32-52::AgentCommissionSettlementGuard`、`:60-100::run_once_with_dependencies`。
- **可达影响**：扩容 API 会同步扩容所有扫描/同步任务；无锁任务会重复调用上游或并发推进。worker panic/意外 return 后 HTTP 继续服务；瞬态佣金失败需重启才再尝试，固定 poison 理财记录可持续占据扫描首页。
- **增量修复**：引入兼容的 `PROCESS_ROLE=all|api|worker`；以 `WorkerRegistry + JoinSet/TaskTracker + CancellationToken` 管理 required/optional worker、panic 策略和优雅停机；singleton 类 worker 增加 lease+fencing；将 attempt/error_class/next_retry_at/dead_letter 持久化并按到期游标公平分页。
- **兼容/迁移**：首个版本默认 `all` 保持单实例行为，再由部署切成 API/worker；lease/job 表 additive；资金业务继续复用现有事务与幂等键，不在同一发布中重写结算逻辑。
- **验证**：2 API + 1 worker 仅一个 prediction/行情 owner；注入 panic/return 后 registry 在两轮内变红并按策略重启或退出；SIGTERM 后停止领取、等待在途事务并释放 lease；瞬态失败无需重启恢复，poison 达预算后不阻塞后项。
- **工作量/依赖**：L，约 2–4 周；依赖部署拓扑、租约存储、job schema、错误分类及 DAR-P1-06 readiness。
- **运行时证据 caveat**：若生产严格单 API，重复 owner 暂不触发；当前仓库没有可验证的单实例部署约束，且任务退出不可见问题与副本数无关。

### DAR-P1-04 — 行情仍可被未来时间戳锁死，缺源时间的 REST 又会被包装成本机“现在”

- **Severity / 历史映射**：P1；P1-12 部分完成。
- **当前证据**：
  - ticker 构造只规范化 symbol，不校验正价、过期或 future skew：`src/modules/market/domain.rs:247-287::MarketTickerSnapshot::with_24h`。
  - Redis Lua 只接受严格更大的 `observed_at`，不与接收时钟比较：`src/modules/market/infrastructure/cache.rs:404-418::SAVE_TICKER_IF_FRESH_SCRIPT`。
  - REST 响应缺 `ts` 时直接使用 `Utc::now()`，注释也承认该值无法衡量真实延迟：`src/modules/market/infrastructure/adapters/provider.rs:1024-1032::rest_payload_observed_millis`。
  - 启动读取 DB 行情配置失败时回落 env 配置：`src/main.rs:59-93`；后台 reload 仅操作命中实例的进程内 supervisor，随后把共享 DB 版本标 success：`src/modules/admin/application/market_feed.rs:168-239::reload_admin_market_feed_config`。
  - 已完成的增强包括 generation fencing/status：`src/workers/market_feed.rs:303-518`，以及 append-only 事件时点价格的 source/observed_at/generation/version：`migrations/0114_event_time_price_snapshots.sql:1-30`；二者仍未提供上游时钟可信度或每实例版本 ACK。
- **可达影响**：远未来帧先写 Redis 后，正常 provider 帧会长期被 CAS 拒绝；缺真实时间的旧 REST 快照会被视为最新并触发撮合/风控派生路径。多副本还可能分别运行 DB 版和 env 版配置，却把一次本地 reload 表示成全局成功。
- **增量修复**：分离 `provider_observed_at`、`received_at` 与 `time_trust`；配置 provider-specific 最大 future skew，异常帧隔离且不得触发资金副作用；REST 缺源时间标 `untrusted` 并限制为展示/恢复候选；行情改独立 singleton，或由每个实例持久 ACK `instance_id + config_version + generation`，全量收敛后才标成功。
- **兼容/迁移**：缓存/事件 DTO 先 additive 双写新字段，消费者在兼容窗口内优先新字段；上线前扫描并清除/隔离 future-poisoned key；保留旧 env 只作显式 bootstrap，不在 DB 读取错误时静默回退。
- **验证**：注入 `now + skew + 1` 的 ticker 必须拒写，随后正常帧可写；无 `ts` REST 不得成为资金价格；双实例保存/禁用配置后必须全部 ACK 同版，任一实例失败时全局状态不显示 success。
- **工作量/依赖**：M/L，约 5–10 天；依赖 provider 时间 SLO、缓存 DTO 兼容、实例身份与配置 ACK 存储。
- **运行时证据 caveat**：未测量三家 provider 的实际时钟偏差或生产缓存中是否已有未来值；阈值需以运行数据定标，静态代码只证明当前没有上限。

### DAR-P1-05 — 迁移门禁和数据库最后一层约束仍不完整，新增 fresh 测试在 CI 中可静默跳过

- **Severity / 历史映射**：P1；P1-13 部分完成。
- **当前证据**：
  - 当前迁移链到 `0117`；`0108` 已为提现状态/链证据加 CHECK：`migrations/0108_withdrawal_broadcast_reconciliation.sql:56-82`，`0109` 已为报价金额守恒/过期/消费加 CHECK：`migrations/0109_withdrawal_quotes.sql:25-35`。
  - event outbox/inbox 的 `status` 和 `retry_count` 仍是无 CHECK 的自由文本/有符号 INT：`migrations/0008_events_risk_audit.sql:1-33`、`migrations/0009_event_inbox_retry_count.sql:1-2`。
  - `wallet_deposit_events` 的 amount、status、confirmations 与 required_confirmations 仍无 CHECK：`migrations/0087_p0_financial_safety.sql:83-108`；withdrawal quote 有金额约束，但既有 `wallet_withdrawal_requests` 本体仍未获得同等金额关系约束：`migrations/0109_withdrawal_quotes.sql:38-44`。
  - CI 已新增 quality gate：`.github/workflows/docker-image.yml:15-56` 调用 `scripts/p0-release-gate.sh:8-28`；但 fresh/re-run migration 测试在缺 `DATABASE_URL` 或无建库权限时直接成功返回：`tests/withdrawal_quote_migration.rs:36-63::fresh_database_runs_the_complete_migration_chain_idempotently`，workflow 未定义 MySQL service/`DATABASE_URL`，也未见上一生产快照 upgrade 或旧应用/新 schema lane。
- **可达影响**：应用 bug、人工 SQL 或后续脚本仍可落入非法 event/deposit 状态，导致扫描遗漏或错误重试；CI 表面执行全量 `cargo test`，却不能证明 fresh/upgrade/re-run/rollback compatibility，迁移缺陷仍可进入镜像。
- **增量修复**：只新增 follow-up migration，不编辑已发布文件；先画像/清洗，再补 event status、retry>=0、deposit amount/status/确认数关系和 withdrawal request 守恒等可表达 CHECK。CI 提供隔离 MySQL 8.4，integration lane 缺依赖必须失败；加入 fresh、上一生产快照 upgrade、re-run、旧应用/新 schema smoke 和大 DDL 中断恢复。
- **兼容/迁移**：采用 expand-contract：先部署写端校验和异常报表，再清洗存量，最后加 CHECK；保留旧列/值兼容窗口，旧应用 smoke 通过后才收紧；SQLx 既有 migration 保持不可变。
- **验证**：直接 SQL 注入每类非法状态/金额/确认关系均被拒；四类 migration lane 无 skip；模拟 0099 类 DDL 中断后按 runbook 恢复；声明窗口内旧镜像能读写新 schema。
- **工作量/依赖**：M/L，约 1–3 周；依赖生产数据画像、上一版 schema/应用 fixture、MySQL 8.4 service 与发布权限配置。
- **运行时证据 caveat**：未读取生产异常数据、SQL mode、已应用 migration checksum 或 GitHub branch protection；约束上线前必须先做真实数据画像。

### DAR-P1-06 — liveness 恒绿，依赖/worker/积压停摆没有 readiness 和机器可抓取指标

- **Severity / 历史映射**：P1；P1-18 仍存在。
- **当前证据**：
  - 唯一 `/health` 忽略 AppState 并恒定返回 `ok`：`src/lib.rs:75-97::health`。
  - Mongo connect 只构造句柄、不 ping：`src/infra/mongo.rs:15-20::connect`。
  - worker 退出只在 detached task 中写日志，不改变服务状态：`src/main.rs:97-314::main`。
  - inbox 的 total/consumed/retried/dead-letter 只是每轮结构化日志与日志告警：`src/workers/event_inbox.rs:200-220::run_retry_scanner_once`；路由树未见 `/ready` 或 `/metrics`。
  - market-feed 已有进程内 `ready/last_reload_status`：`src/workers/market_feed.rs:303-313::MarketFeedRuntimeStatus`，但没有接到 HTTP readiness。
- **可达影响**：MySQL/Mongo/Redis/Rabbit 断开、结算/强平/outbox/inbox worker 退出、价格长期陈旧或死信积压时，容器仍 healthy 且继续接流量；运维无法用统一指标判定 oldest pending、owner、lag 和价格 age 是否越过 SLO。
- **增量修复**：保留 `/health` 为纯 liveness，新增有界超时 `/ready`；`WorkerRegistry` 暴露 required worker heartbeat、最近成功、owner/generation、backlog oldest-age；导出 Prometheus/OTel 指标与稳定低基数标签，并配置 dead-letter、unknown withdrawal、陈旧价格、WS lag 告警。
- **兼容/迁移**：不改变现有 `/health`；先只观测 `/ready`，稳定后把 Compose/1Panel/Kubernetes 流量探针切到它；按业务角色配置 required worker，避免纯 API 实例因未运行 worker 被误判。
- **验证**：逐个断依赖、杀 required worker、制造 backlog/陈旧价格，readiness 在两轮或 120 秒内失败且恢复后自动转绿；指标端点可抓取并用测试规则触发/解除告警。
- **工作量/依赖**：M/L，约 1–2 周；依赖 DAR-P1-03 registry、监控后端、探针配置与 SLO 口径。
- **运行时证据 caveat**：生产可能已有日志采集或外部 TCP 探针，未在仓库中补证；静态上可以确定应用自身的健康响应无法表达这些故障。

### DAR-P1-07 — Redis 先成功、Mongo 后失败时，相同 K 线重试会被 Redis CAS 拒绝，缺少自动对账修复

- **Severity / 历史映射**：P1；本轮新增的跨存储一致性发现，与 P1-12/P1-20 的恢复边界相关但不重复计入其根因。
- **当前证据**：
  - K 线摄取固定先写 Redis，只有 accepted 才 upsert Mongo：`src/modules/market/infrastructure/adapters/ingestion.rs:201-214::ingest_kline`；synthetic 路径同样先 Redis 后 Mongo：`:217-238::ingest_and_publish_synthetic_kline`。
  - Redis 时序脚本对相同 `(open_time, observed_at)` 返回 stale/rejected：`src/modules/market/infrastructure/cache.rs:422-453::SAVE_KLINE_IF_FRESH_SCRIPT`。
  - Mongo 具备 `(interval, open_time)` 唯一索引和幂等 upsert 基础：`src/infra/mongo.rs:35-55::ensure_kline_indexes/kline_unique_index_model`，但仓库未见 Redis→Mongo half-commit 的持久 repair record 或周期 gap reconciler。
- **可达影响**：Redis 写成功后 Mongo 短暂失败，当前调用返回错误；若重试完全相同快照，Redis 会先判 stale 并短路，Mongo 不再执行。形成中 K 线可能被下一版自然修复，但关闭帧/一次性帧可能永久缺失，影响历史图表、历史估值和恢复完整性。
- **增量修复**：在 Redis accepted/Mongo failed 时写持久 repair job（symbol、interval、open_time、observed_at、payload hash）；repair worker 直接执行 Mongo 幂等 upsert，不再经过 Redis CAS；增加按时间窗对账和 unresolved oldest-age 指标。
- **兼容/迁移**：API/缓存合同不变；additive 增加 repair 表或专用 outbox 类型；先 shadow 记录差异并回填历史缺口，再启自动修复。
- **验证**：故障注入精确卡在 Redis 成功、Mongo 失败之后；重放同一快照最终在 Mongo 恰有一条正确文档，Redis 不倒退，repair job 进入终态；重复 repair 无副作用。
- **工作量/依赖**：M，约 3–5 天；依赖持久作业载荷、Mongo 幂等写入口、历史源保留和差异指标。
- **运行时证据 caveat**：未测量生产 Mongo 失败率；活跃 candle 的后续帧会缩短不一致窗口，但不能证明每个关闭/低频 candle 都一定有下一版。

### DAR-P1-08 — 仓库仍没有可验证的 MySQL/Mongo/Redis/Rabbit/uploads/密钥联合恢复体系

- **Severity / 历史映射**：P1；P1-20 在仓库范围内仍存在，生产需运行时补证。
- **当前证据**：
  - 完整 Compose 仅给 MySQL、Mongo、Redis、RabbitMQ、uploads 声明 named volumes；Redis 仅启用 AOF：`docker-compose.example.yml:40-41,62-63,65-76,78-91,124-134`。
  - 1Panel 示例只管理 uploads volume，其余存储均为外部依赖：`docker-compose.1panel.example.yml:56-83`。
  - 部署文档只为 0099 要求停写、人工全库备份并恢复到隔离库验证：`docs/deployment/docker.md:98-120`；DDL 部分提交后的恢复仍依赖该备份：`:200-227`；应用回滚明确不回滚 schema：`:334-340`。
  - 对仓库脚本、workflow、Compose 和部署文档的静态检索未找到通用 PITR、定时备份、异地/不可变保留、Rabbit topology export、密钥 escrow 或周期 restore drill 实现。
- **可达影响**：RPO/RTO 无仓库证据；单独恢复 MySQL 可能与 Mongo K 线、Rabbit outbox/inbox 消费位置、Redis 协调/会话、uploads 引用处于不同时间点；丢失 `CREDENTIAL_ENCRYPTION_KEY` 时数据库恢复后运营凭据仍不可解密。
- **增量修复**：定义经业务批准的分层 RPO/RTO；MySQL full+binlog PITR、Mongo 一致性快照、Rabbit definitions/policy 与必要消息保护、uploads 对象/卷快照、版本化密钥 escrow；编排隔离恢复并依次执行 schema、钱包/平台 journal、outbox/inbox、K 线、文件引用和解密 canary 对账。
- **兼容/迁移**：不改在线 API；先建立 runbook 和只读备份，再做隔离 restore drill；Redis 明确区分可重建缓存与需保护的会话/协调状态，避免盲目恢复过期锁；密钥采用 `key_id/version` 双读单写后再轮换。
- **验证**：从指定时间点在空环境恢复全部存储，记录实际 RPO/RTO；钱包三桶与 journal、event 状态、Mongo 唯一键/时间范围、uploads 引用、Rabbit bindings 和密文解密全部通过；按季度保存演练报告并验证备份不可变/过期策略。
- **工作量/依赖**：L/运维项目，约 2–6 周起；依赖云/1Panel 快照与 PITR、对象存储/KMS、隔离恢复环境、业务对账脚本和批准的 RPO/RTO。
- **运行时证据 caveat**：未访问云厂商、1Panel、KMS 或生产备份控制台；结论仅是“仓库无法证明”，不能据此断言生产完全没有外部备份。

## Files Found

- `src/main.rs` — 外部依赖装配及全部后台 worker 的 fire-and-forget 启动入口。
- `src/lib.rs`、`src/state.rs` — 恒成功 liveness、共享依赖及进程内广播 hub 装配。
- `src/modules/events/service/{rabbitmq,outbox,inbox,websocket}.rs` — Rabbit 发布/消费确认、outbox/inbox 重试和 lossy WS 语义。
- `src/modules/events/infrastructure.rs` — outbox/inbox MySQL 状态推进及幂等用户钱包初始化。
- `src/workers/{event_inbox,event_outbox,market_feed,earn_auto_redemption,agent_commission_settlement}.rs` — 消费补偿、行情 generation 生命周期及不一致的 worker retry 模型。
- `src/modules/market/{domain.rs,infrastructure/cache.rs,infrastructure/adapters/provider.rs,infrastructure/adapters/ingestion.rs}` — provider 时间、Redis CAS、Mongo K 线写入顺序与跨存储窗口。
- `migrations/0008_events_risk_audit.sql`、`0009_event_inbox_retry_count.sql`、`0087_p0_financial_safety.sql` — event/充提基础 schema 与剩余 CHECK 缺口。
- `migrations/0108_withdrawal_broadcast_reconciliation.sql`、`0109_withdrawal_quotes.sql`、`0114_event_time_price_snapshots.sql` — 本轮确认已补强的状态、报价和事件时点价格约束。
- `tests/withdrawal_quote_migration.rs`、`.github/workflows/docker-image.yml`、`scripts/p0-release-gate.sh` — fresh/re-run 测试及当前 CI 调用链与可跳过边界。
- `pc/src/api/stomp.ts`、`web/src/api/marketTickerSocket.ts`、`mobile/src/core/marginAccountReconciliation.ts` — 三端实时连接与 REST 恢复能力差异。
- `docker-compose.example.yml`、`docker-compose.1panel.example.yml`、`docs/deployment/docker.md` — 默认服务拓扑、卷、健康检查和现有人工迁移恢复说明。

## Code Patterns

- **事务 outbox + 幂等 inbox 骨架已成立**：`src/modules/auth/application.rs:507-565`、`src/modules/events/infrastructure.rs:260-303`；broker confirm/topology 是剩余断点。
- **局部结构化并发、全局 detached task**：market feed 使用 generation/fence/JoinSet（`src/workers/market_feed.rs:303-518,954-1040`），主入口仍丢弃所有顶层 JoinHandle（`src/main.rs:59-314`）。
- **Redis CAS 作为派生副作用总闸**：`src/modules/market/infrastructure/cache.rs:404-453`；能防倒退，但 future skew 和 Redis→Mongo half-commit 需要额外恢复通道。
- **WS 明确是提示而非事实，但执行不均衡**：backend/mobile 规范要求 REST 对账，mobile 已实现；PC/admin 尚未形成同等 lifecycle。
- **迁移保护逐项增强但 CI 环境未强制**：新 CHECK 与 fresh test 已出现，缺数据库时 integration test 仍可返回成功。

## External References

- 本次按“5 分钟内结束”的要求未联网检索或引用外部文档。
- 仓库声明的运行版本基线：MySQL 8.4、MongoDB 7、Redis 7、RabbitMQ 3（`docker-compose.example.yml:23-91`）；部署文档记录 SQLx 0.8.6 的 dirty migration 行为（`docs/deployment/docker.md:200-208`）。

## Related Specs

- `.trellis/spec/backend/database-guidelines.md` — immutable migration、expand/backfill 和真实 MySQL 验证合同。
- `.trellis/spec/backend/realtime-websockets.md` — process-local lossy hint、重连/周期 REST 对账及 market-feed liveness 合同。
- `.trellis/spec/backend/synthetic-market-kline.md` — generation lease/fencing、1m 权威聚合和手工历史恢复边界。
- `.trellis/spec/backend/container-delivery.md` — migrator、Compose 启动门禁和镜像运行合同。

## Caveats / Not Found

- 本轮是有界静态审计；按用户要求未运行 build、test、数据库迁移、容器或故障注入。
- 未读取生产 MySQL/Mongo/Redis/RabbitMQ 数据、Rabbit topology/policy、真实副本/负载均衡、监控告警、云备份/PITR、KMS 或 1Panel 外部配置。
- “未找到”均限定为当前仓库静态检索结果；外部基础设施可能已有补偿能力，需以运行证据复核。
- 行号为 2026-08-30 当前工作树快照；后续优先按符号名定位。
