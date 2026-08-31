# Research: 当前交付、安全、可观测性、灾备与质量复审

- Query: 审计当前 checkout 的 GitHub Actions、Docker/supervisor/Nginx/1Panel、Secret/配置边界、依赖与镜像来源、迁移发布/回滚、健康/指标/日志/告警、备份恢复、测试质量与 Trellis/仓库治理，并复核 2026-08-24 相关 P1/P2。
- Scope: mixed（仓库静态证据 + 官方外部文档；未执行可疑 PC 构建、联网 payload、真实部署或恢复演练）
- Date: 2026-08-30

## Findings

### 结论摘要

- **当前发布结论：HOLD。** `pc/postcss.config.js:8` 存在可在 Vite/PostCSS 构建期间运行的混淆远程代码加载器，属于本次新增 **P0 发布阻断/事件响应项**。
- 2026-08-24 的“CI 只 build/publish”与“会话枚举失败被 `unwrap_or_default` 吞掉”两项旧描述已被当前实现部分推翻；但 P1-01、03、11、13–15、18–21 与 P2-01、02、08 的核心风险仍未闭环。
- 当前仓库已有显著正向基础：发布前质量 job、锁文件安装、非 root 一体化镜像、独立迁移器、事务 outbox/inbox、部分管理员 request-id 与较广的测试资产；这些不足以抵消 P0 loader 与可跳过的集成门禁。

### DSQ-01 — PC PostCSS 配置含混淆远程代码加载器

- **优先级**：P0 / 立即停止 PC 构建和发布。
- **证据**：
  - `pc/postcss.config.js:8` 是 7,416 字节单行混淆顶层 IIFE；文件 SHA-256 为 `556812c8ec8177751aa22b8fa641a92e782f9e2564866887061c6626186bd5f0`。
  - 仅做静态解码、未执行代码：解码后的调用链动态取得 `https`/`JSON`/`Buffer`，从 Tron/Aptos 交易记录派生标识，再调用 BSC JSON-RPC `eth_getTransactionByHash` 读取交易 `input`，XOR 解密为 JavaScript；第一路直接 `eval`，第二路调用 `child_process.spawn("node", ["-e", ...], { detached: true, stdio: "ignore", windowsHide: true })`，spawn 报错时回退 `eval`。
  - `pc/package.json:6-12::scripts.build` 运行 `vite build`；`pc/src/main.ts:4` 导入 CSS。Vite 官方说明项目中的 `postcss.config.js` 会自动应用于导入的 CSS，因此配置顶层代码处于构建执行路径。
  - `.github/workflows/docker-image.yml:49-56` 安装 PC 依赖后运行质量脚本，但 `scripts/p0-release-gate.sh:22-24` 只执行 `type-check` 和单个 margin 测试，不执行 PC build，也无混淆/恶意 loader 扫描。
- **影响**：运行 PC dev/build 的开发机或 CI runner 可下载并执行随链上交易变化的任意代码，读取构建环境可访问的源码、令牌、SSH/registry/cloud 凭据，并启动脱离父进程的 Node 子进程；现有构建产物和凭据应视为待调查。
- **增量建议**：
  1. 隔离执行过 PC Vite/PostCSS 的主机与 runner；暂停 PC 构建、发布和缓存复用。
  2. 从可信来源恢复一个只含 Tailwind/Autoprefixer 的配置；不要在受影响主机上运行当前文件。
  3. 轮换可能暴露的 GitHub、npm/GHCR、云、SSH、数据库和部署凭据；清除 runner/cache 前先保全日志。
  4. 增加 source/config 长行与 `Function`/`eval`/`child_process`/网络加载组合扫描、依赖/Secret 扫描、CODEOWNERS 与安全审阅；在隔离、默认断网 runner 中补 PC build gate。
- **验证**：目标 hash 从所有分支、制品、缓存和镜像上下文消失；可信 clean config 可审阅；隔离 runner 的 PC build 无外联与脱离子进程；进程树、DNS/HTTP、shell history、CI run 与 Secret 使用日志完成回溯。
- **依赖/工作量**：代码清理 S；事件响应、凭据轮换与主机取证 M/L。
- **外部/运行时证据需求**：需要 Git 历史/PR provenance、哪些主机和 CI run 加载过该配置、网络/EDR/进程日志、当时链上 payload 内容、相关 Secret 的访问与使用记录。本研究按约束未运行 Git 操作或 payload。

### DSQ-02 — `user.created` 的 RabbitMQ 可靠交付和部署拓扑仍未闭环

- **优先级**：P1（复核 P1-01：仍存在）。
- **证据**：
  - `src/modules/events/service/rabbitmq.rs:107-138::RabbitMqOutboxPublisher::publish` 只声明 durable exchange 并调用默认 `basic_publish`；注释明确未 `confirm_select`，结果不是 broker ACK。
  - 同文件 `:141-221::RabbitMqInboxConsumer` 要求队列预先声明/绑定，代码不做 `queue_declare`、`queue_bind` 或 QoS/prefetch。
  - `src/workers/event_inbox.rs:66-106::EventInboxWorkerConfig::from_env_values` 将缺失/空 `EVENT_INBOX_QUEUE_NAME` 视为正常停用。
  - `docker-compose.example.yml:1-20` 与 `docker-compose.1panel.example.yml:7-31` 均未传入 `EVENT_INBOX_QUEUE_NAME`；仓库未发现 RabbitMQ definitions/topology 文件。
  - `src/modules/events/service/production_dispatch.rs:15-21,90-98` 只有 `user.created` 产生钱包初始化副作用，其余白名单事件成功 ACK 但无业务动作。
- **影响**：无 binding 时消息可因默认 `mandatory=false` 而不可路由，publisher 仍可能把 outbox 推进为已发布；标准 Compose 可默认不启动 consumer，注册用户永久缺少资产钱包。
- **增量建议**：publisher confirm + mandatory/return；版本化 durable exchange/queue/binding/DLX definitions；生产角色强制配置 consumer；readiness 检查 topology；增加 `users × active_assets` 幂等补偿。
- **验证**：Nack、unroutable、断线不标 published；全新 Compose 注册后在 SLA 内账户齐全；空账户删除后补偿可重建；缺队列/binding 时 readiness 失败。
- **依赖/工作量**：RabbitMQ IaC、集成环境、worker role 和监控，M/L（5–8 天）。
- **外部/运行时证据需求**：实际 RabbitMQ definitions、queue policy/DLX、publisher return/confirm 指标和生产 users×assets 差集。

### DSQ-03 — worker 无顶层监督，liveness 被当作 readiness，业务停摆不可机器判定

- **优先级**：P1（复核 P1-11/P1-18：仍存在）。
- **证据**：
  - `src/main.rs:1-4` 明确所有 worker 使用 fire-and-forget；`:59-313` 对行情、outbox、结算、强平、计息、佣金、贷款、钱包链、prediction、inbox 等分别 `tokio::spawn`，错误只写日志，没有 `JoinSet`/registry/required-worker 状态。
  - `src/lib.rs:76-98::health` 恒定返回 `{"status":"ok"}`，并明确忽略 MySQL/Redis/worker；没有 `/ready`。
  - `Dockerfile:77-78` 与 `docker/nginx.conf:53-55` 仅把该 `/health` 用作容器健康检查。
  - `src/workers/event_inbox.rs:206-217` 有批次计数和 alert 分类，但只 emit 日志；仓库未发现 Prometheus exporter、OpenTelemetry pipeline、指标抓取端点或告警规则。
  - `src/main.rs:34-37` 使用默认文本 fmt layer；全局 HTTP 只有 `TraceLayer`。request-id 仅覆盖后台路由：`src/infra/admin_request_context.rs:31-57`。
- **影响**：结算、强平、链任务或 inbox 永久退出后容器仍 healthy 并继续收流；多 API 副本同时启动相同 worker；日志无法形成可靠 SLO、积压、死信和价格陈旧告警。
- **增量建议**：拆分 `APP_ROLE=api|worker|all`；WorkerRegistry/JoinSet 管理 required/optional、心跳、panic/restart；新增 `/live` 与依赖/worker `/ready`；输出 JSON tracing、全局 request-id、Prometheus/OTel 指标及 SLO 告警。
- **验证**：杀 required worker/断依赖后两轮或 120 秒内 readiness 503，恢复后自动转绿；2 API + 1 worker 只有一个 owner；积压/死信/price-age 可抓取并触发测试告警。
- **依赖/工作量**：部署角色、租约/fencing、监控平台，L（2–4 周）。
- **外部/运行时证据需求**：1Panel 探针路由、实例数、日志采集、监控/告警平台、worker SLO 与 on-call 记录。

### DSQ-04 — 质量 job 已加入，但关键集成测试可静默跳过，PC/mobile build 与供应链 gate 缺失

- **优先级**：P1（复核 P1-14/P1-15：部分改善，未关闭）。
- **证据**：
  - **旧结论已失效部分**：`.github/workflows/docker-image.yml:16-56` 现有 `quality-gate`，PR/publish jobs 在 `:58-159` 通过 `needs` 依赖它；使用 `npm ci`，这是明确进步。
  - `scripts/p0-release-gate.sh:8-28` 跑 Rust fmt/clippy/all-targets、web lint/type/test、PC type-check+单个 `test:margin`、mobile type/test；没有 PC/mobile build、Compose smoke、migration upgrade matrix、coverage、SBOM、签名/attestation、依赖/镜像漏洞扫描。
  - 当前静态统计：`tests/*.rs` 58 个 integration targets 中至少 39 个包含显式 “skipping”；37 个依赖 `DATABASE_URL`、11 个 Redis、3 个 Mongo、1 个 RabbitMQ。workflow 没有 service containers。
  - 代表例：`tests/withdrawal_quote_migration.rs:36-63::fresh_database_runs_the_complete_migration_chain_idempotently` 在缺 URL 或无 CREATE DATABASE 权限时直接返回成功。
  - `pc/package.json:6-12` 只有单文件 `test:margin`；PC 当前有 21 个 test 文件/95 个 test 调用，其中 16 个读取源码文本，无组件 harness。Mobile 有 80 个 test 文件/511 个调用，其中 68 个读取源码文本，且 `mobile/tsconfig.json` 明确排除 `tests`。
  - `src/openapi.rs:1-5,18-36` 明示手工同步，模块只覆盖 auth/user/wallet/quick-recharge/agent/news/support/system-config，未形成 spot/margin/seconds/earn/loan/prediction 等核心资金 DTO 的生成合同。
- **影响**：绿色 release gate 不能证明真实 MySQL/Redis/Mongo/RabbitMQ、完整迁移链、PC 构建或关键 UI 行为；源码字符串测试容易在行为已坏时继续通过；破坏性 schema/DTO 漂移无 required gate。
- **增量建议**：CI 增 MySQL 8.4/Redis/Mongo/RabbitMQ services，禁止 required 测试 skip；PC/mobile 都执行全测试+build；fresh/上一生产快照 upgrade/re-run/旧应用 smoke；按 wallet→margin→seconds 生成 OpenAPI DTO/golden fixtures；加行为/E2E、coverage 阈值和 flake 记录。
- **验证**：故意破坏每 lane 均阻断 publish；required 日志 skip=0；核心资金请求/交互不是源码文本匹配；schema diff 和覆盖率下降失败。
- **依赖/工作量**：CI services、测试账号/fixture、schema 版本策略，M/L（1–4 周分批）。
- **外部/运行时证据需求**：required check/branch protection 配置、真实 Actions run 日志、flake 历史、覆盖率基线和发布制品关联。

### DSQ-05 — Action、基础镜像和 Compose 仍为可变引用，发布物无可验证 provenance

- **优先级**：P1（P1-14/P1-19 供应链部分）。
- **证据**：
  - `.github/workflows/docker-image.yml:25,35,40,79,84,87,117,122,125,133,148,167,174,177,185` 全部使用浮动 major tag，而非完整 commit SHA。
  - `Dockerfile:3-6,19,38` 固定 Rust/Node 版本号但未 pin OCI digest；apt 包也未版本锁定。CI 的 Rust `stable`/Node 22（workflow `:29-42`）与镜像 Rust 1.92/Node 24 不一致。
  - `docker-compose.example.yml:23-94,109-110` 的 MySQL/Mongo/Redis/RabbitMQ 和 app image 都是可变 tag；1Panel example 默认 `latest`（`docker-compose.1panel.example.yml:39-57`）。
  - workflow 没有 SBOM、artifact attestation、签名或部署前 attestation 验证。
  - lockfile 有 integrity 且 `npm ci` 是强项；但 web/mobile 大多数 tarball 固定到 `registry.npmmirror.com`（唯一 resolved URL 分别 460/463、472/475），这个额外来源边界未在交付文档说明。
- **影响**：相同源码/标签可能在不同时间解析成不同 Action、base image 或依赖来源；质量 job 验证的依赖环境与实际镜像构建环境不完全相同，无法从已部署 digest证明“由哪次已验证构建产生”。
- **增量建议**：Action pin 完整 SHA；base/service/app image pin digest；统一 rust-toolchain/Node 版本；生成并签署 SBOM/provenance attestation；发布一次构建的 digest，不在验证后重新解析可变输入；建立依赖升级 bot 与来源策略。
- **验证**：部署只接受 immutable digest 且 attestation subject digest 匹配当前 workflow/source；重建输入清单一致；策略拒绝浮动 Action/tag。
- **依赖/工作量**：registry/组织策略/签名身份，M（3–7 天）。
- **外部/运行时证据需求**：GHCR digest、Actions org policy、环境审批、registry retention/signing 和实际部署 digest。

### DSQ-06 — 本地 1Panel Secret 边界和生产容器硬化未闭环

- **优先级**：P1（复核 P1-19：仍存在）。
- **证据**：
  - `.gitignore:2-7`、`.dockerignore:5-11` 排除了实际 env/`docker-compose.1panel.yml`，但忽略并不等于 Secret 管理。
  - 当前本地 `docker-compose.1panel.yml:3,20,22,24,26` 含连接凭据/JWT/加密 key 的字面量；文件权限为 `0644`，且 JWT 与凭据加密 key 复用同一值。`:96` 将 API 端口发布到所有宿主接口，偏离 example 的 loopback 默认。**本报告不记录任何 Secret 值。** 该文件是忽略的本地运行证据，不代表已提交内容。
  - `docker-compose.yml:3-39` 的开发依赖使用固定弱默认凭据并把 MySQL/Mongo/Redis/RabbitMQ/management 端口发布到所有接口；文件没有显式 local-only 防护。
  - 两份生产 example 均没有 CPU/memory/pids、`read_only`、`cap_drop`、`no-new-privileges` 或 tmpfs；`docker/nginx.conf:26-27` 允许无限 body size。
  - `src/config.rs:15-123::Settings` 与 worker 自行读 env 并存；关键 `EVENT_INBOX_QUEUE_NAME` 不在任何 Compose/env example 中。`src/infra/secrets.rs:29-75` 密文仅为 `base64(nonce||ciphertext||tag)`，无 version/key_id；`src/config.rs:155-162` 明确换 key 后历史密文不可读。
- **影响**：同机用户可读部署 Secret；密钥复用扩大单点泄露面；端口误暴露；配置漏传可静默停用 consumer；资源耗尽影响整台 1Panel 主机；主密钥遗失/轮换会造成凭据不可恢复。
- **增量建议**：立刻轮换本地暴露/复用 Secret 并改 `0600` 或平台 Secret/file mount；端口回 loopback；从 typed schema 生成 env/Compose 并对 required worker 配置 fail-fast；加资源/能力/只读 FS 限制；envelope 加 version/key_id、双读单写轮换与离线 escrow。
- **验证**：源码消费 key 与部署模板 100% 对齐；权限/Secret 扫描无明文和复用；非法/缺关键值启动失败；容器限制和只读 FS smoke；旧新 key 共存迁移后可撤旧 key。
- **依赖/工作量**：1Panel Secret manager、容量基线、KMS/escrow，代码 M/L + 运维项目。
- **外部/运行时证据需求**：1Panel 网络/防火墙、Secret 注入方式、宿主用户权限、资源配额、Redis/数据库 HA 与连接预算。

### DSQ-07 — 迁移器基础改善，但 release 没有 upgrade/rollback 证据链

- **优先级**：P1（复核 P1-13：部分改善，未关闭）。
- **证据**：
  - `src/bin/exchange-migrate.rs:13-39` 使用编译期嵌入 SQLx migrator、单连接、失败非零退出；Compose 在迁移成功后才启动 API（`docker-compose.example.yml:93-123`），是当前强项。
  - 当前 migration 共 114 个，末尾 `0117_margin_partial_close.sql`；最近迁移包含多项 `ALTER/UPDATE/CHECK`。
  - `tests/withdrawal_quote_migration.rs:36-174` 新增 fresh 0001–0117 与 re-run 测试，但如 DSQ-04 所述 CI 无 MySQL，测试可跳过。
  - `docs/deployment/docker.md:223-227` 只有 0099 专项人工恢复且无 down migration；`:334-336` 明示应用回滚不回 schema。仓库未发现上一生产快照 upgrade、旧应用+新 schema smoke 或通用 expand-contract gate。
  - 文档漂移：`docs/deployment/docker.md:3-7` 仍称表空就创建管理员，实际 `exchange-migrate.rs:41-48` 只有显式 `BOOTSTRAP_MODE=create_admin` 才创建。
- **影响**：fresh schema 可用不代表存量数据升级、部分 DDL 失败恢复或旧应用回滚兼容；大 ALTER 可能锁表或留下部分提交状态。
- **增量建议**：required migration matrix：fresh、当前生产脱敏快照 upgrade、re-run、旧应用/new schema smoke；每次 migration 写兼容/锁时预算和恢复步骤；默认 expand-contract，收紧/删列延后一个兼容窗口。
- **验证**：四 lane 全绿且不可 skip；故意中断迁移按 runbook 恢复；旧镜像在新 schema 上完成核心只读/写 smoke；记录实际锁时。
- **依赖/工作量**：脱敏生产 fixture、上一版镜像、MySQL 权限，M/L（1–3 周）。
- **外部/运行时证据需求**：生产 MySQL 版本/数据量/异常画像、在线 DDL 能力、维护窗口、实际迁移日志。

### DSQ-08 — 多存储备份、PITR 和恢复演练仍不可验证

- **优先级**：P1（复核 P1-20：仍存在）。
- **证据**：
  - `docker-compose.example.yml:40-41,62-63,75-76,90-91,126-134` 只有 named volumes；Redis 开 AOF 是局部耐久性，不是跨存储备份。
  - `docs/deployment/docker.md:456-460` 仅要求“定期备份”卷，没有工具、频率、保留、加密、校验、RPO/RTO、跨存储恢复顺序或演练记录。
  - 仓库未找到通用 backup/restore/PITR/runbook 脚本；`docs/deployment/docker.md:223-227` 只有 0099 维护窗口备份恢复说明。
- **影响**：只恢复 MySQL 会与 Mongo K 线、Rabbit topology/积压、Redis 会话/协调、uploads 及 `CREDENTIAL_ENCRYPTION_KEY` 产生错点；密钥未恢复时数据库密文不可解；RPO/RTO 只有假设没有证明。
- **增量建议**：MySQL full + binlog PITR；Mongo 一致性 snapshot/oplog；Rabbit definitions/policies 与必要消息策略；uploads snapshot；Secret/key escrow；编排冻结/恢复顺序及恢复后资金、事件、K 线、文件对账；隔离环境定期自动演练。
- **验证**：从空环境恢复到指定时间点；对账 invariant 通过；季度 drill 的测得 RPO/RTO 不超业务批准目标；备份加密、异地、保留和删除可审计。
- **依赖/工作量**：存储商/1Panel/KMS/对象存储/隔离环境，L 运维项目。
- **外部/运行时证据需求**：云/1Panel 备份策略、binlog/oplog 保留、备份成功率、最近 restore drill 报告、密钥 escrow 和责任人。

### DSQ-09 — 5xx 仍泄露内部原文；CORS 与邮件交付状态仍是旧缺口

- **优先级**：P1（5xx）；P2（CORS、邮件）。
- **证据**：
  - `src/error.rs:15-48::AppError` 的 Database/Mongo/Redis/Rabbit/Internal `Display` 带底层文本；`:131-147::IntoResponse` 将 `self.to_string()` 直接作为响应 message。
  - `tests/unit_src/src_modules_auth_routes_tests.rs:98-113` 明确断言 500 message 包含 MySQL persistence 内部文案，说明泄露行为已被测试固化。P1-03 仍存在。
  - `src/lib.rs:76-85::build_router` 仍为 `CorsLayer::permissive()`；`APP_ENV` 不切换策略。P2-01 仍存在。
  - `src/modules/auth/application.rs:768-853::{send_registration_email_code,send_email_code_for_purpose}` 与 `src/modules/user/application.rs:333-404::send_user_email_bind_code` 都先提交验证码/冷却，再同步 SMTP；发送失败不回滚且无 delivery_failed/outbox。P2-02 仍存在。
- **影响**：SQL/约束/主机/provider 原文可能暴露；生产 origin 边界依赖仓库外网关；SMTP 瞬态失败会消耗冷却并让 API 报错，但系统没有可重试、可查询的交付状态。
- **增量建议**：5xx 只返回稳定 public code/message + error/request id，完整 chain 仅进脱敏日志；生产 CORS allowlist fail-fast；邮件记录 delivery state，并逐步引入不保存验证码明文的受控 outbox/重试模型。
- **验证**：注入带 Secret marker 的 SQL/Redis/SMTP 错误，响应不含 marker 且日志可按 id 关联；非法 Origin 被拒；SMTP 故障有明确 failed/retry 状态且不会错误宣称 sent。
- **依赖/工作量**：错误/OpenAPI 合同、前端 message 盘点、邮件安全模型，S/M（2–5 天）。
- **外部/运行时证据需求**：1Panel/Nginx/WAF CORS 与 TLS 配置、SMTP provider delivery/bounce 能力、集中日志脱敏规则。

### DSQ-10 — 会话撤销旧 bug 已修正，但跨存储提交和 user/agent 代际校验仍有窗口

- **优先级**：P1（复核 P1-02：部分完成）。
- **证据**：
  - **旧结论已失效部分**：`src/modules/auth/mod.rs:373-402::revoke_actor_auth_sessions` 仅把 `SessionNotFound` 当空列表，其他枚举/登出错误均上抛，不再 `unwrap_or_default`。
  - `src/modules/auth/mod.rs:529-549::claims_from_bearer_token` 不查账号状态；`:619-633::UserAuth` 与 `:663-675::AgentAuth` 明示不做实时状态回查。Admin 已通过 `auth_session_version` 回查（`:637-658`），这是局部改进。
  - `src/modules/user/application.rs:493-527::change_user_password` 与 `src/modules/auth/application.rs:1084-1098::reset_password_with_email_code` 先提交 MySQL 改密/刷新令牌撤销，再调用外部 Sa-Token/Redis；外部失败时新密码已生效但旧访问会话状态不确定。
  - `src/modules/admin/application/users.rs:103-147::update_admin_user_status` 与 `src/modules/admin/application/agents.rs:160-216::reset_admin_agent_password` 也依赖提交后撤销，没有持久补偿任务。
- **影响**：Redis/Sa-Token 故障窗口中 user/agent 旧 access token 可能继续有效；API 返回失败也不能证明撤销最终完成。
- **增量建议**：为 user/agent 引入事务内递增的 session/credential version 并在每次鉴权比较；提交同事务写 durable revocation job，外部清理幂等重试；定义 Redis 不可用时的 fail-closed 策略。
- **验证**：故障注入后旧 user/admin/agent token 下一请求均拒绝；恢复后补偿清理完成；重复 job 不影响新会话。
- **依赖/工作量**：auth migration、Sa-Token/Redis 故障注入、三端重新登录合同，M/L（4–7 天）。
- **外部/运行时证据需求**：真实 token TTL、Redis HA/持久化、停用/改密期间访问审计。

### DSQ-11 — Trellis 任务真相、发布元数据和规范文档仍漂移

- **优先级**：P1（P1-21）；P2（P2-08 与仓库治理）。
- **证据**：
  - 当前 `.trellis/tasks/*/task.json` 静态统计为 124 个 active：67 `in_progress`、31 `done`、23 `completed`、3 `planning`；96 个至少 30 天、80 个至少 60 天，54 个已完成语义任务仍在 active 目录。旧报告的 66 `in_progress` 数值已过时，但风险未改善。
  - 当前任务 `task.json:6` 仍是 `planning`；`implement.jsonl:1` 与 `check.jsonl:1` 只有 `_example`，但 `task.py validate` 报 0 entries 且成功，门禁不能证明上下文已配置。
  - 5 个 08-24 P0 archive task 的 `task.json:15-24` 均没有 branch/commit/PR，notes 仍写“等待用户决定提交与推送”；但 `docs/superpowers/PROGRESS.md:7801-7803` 记录提交 `5aa98f1`、`8dd9c89` 已推送，元数据相互矛盾。
  - `.trellis/spec/backend/logging-guidelines.md:7-51` 仍全是 `(To be filled by the team)`；`error-handling.md:7-35` 与 `database-guidelines.md:7-27` 的核心章节仍为模板。P2-08 仍存在。
  - 未发现 `SECURITY.md`、`CODEOWNERS`、`CONTRIBUTING.md`、Dependabot/Renovate 或 Rust deny/audit 配置；对 DSQ-01 这类配置供应链异常缺少 owner 和响应入口。
  - 部分旧 P2 已改善：`.gitignore:33-36` 已忽略 `.codegraph/`（P2-06 部分关闭）；`web/src/admin/routes.tsx:9-18,56-155` 已路由级 lazy load（P2-07 部分改善），但 `resourceConfigs` 仍是共享 1,469 行 chunk 并静态导入所有 action（`web/src/admin/resources/resourceConfigs.tsx:1-34`）。
- **影响**：任务状态、提交与发布证据不可追溯；新 agent 可在空上下文下“校验通过”；关键错误/日志/数据库合同不可执行；敏感构建配置缺明确 reviewer。
- **增量建议**：统一 `completed` 语义并归档 active 完成任务；7/30 天 stale 报告+owner/复核日；archive 写 commit/branch/PR 并与 progress 对账；validator 要求复杂任务至少一个真实 context；补 SECURITY/CODEOWNERS/依赖更新与供应链配置 owner；把实际日志/error/DB 合同写回 spec。
- **验证**：30 天以上无 owner 活动任务为 0；完成任务 active=0；空 context 不能 start/validate；archive commit 与发布制品 digest 可追；spec 不再含核心模板占位。
- **依赖/工作量**：团队 owner、Trellis validator/报告脚本、GitHub governance，S/M（2–5 天）。
- **外部/运行时证据需求**：branch protection、review rules、release/PR 链接与组织级 Action/Secret policy。

## 2026-08-24 P1/P2 复核摘要

| 旧项 | 当前判定 | 高置信度变化 |
| --- | --- | --- |
| P1-01 MQ 钱包初始化 | 仍存在 | outbox/inbox 幂等与持久 retry 是强项，但 confirm/mandatory/topology/consumer 默认配置未补 |
| P1-02 会话撤销 | 部分完成 | 枚举错误吞掉已修；admin 有 session version；user/agent 和提交后副作用仍有窗口 |
| P1-03 5xx 泄露 | 仍存在 | `IntoResponse` 仍返回 `Display`，测试仍锁定内部文案 |
| P1-11/P1-18 worker/readiness/观测 | 仍存在 | worker 仍 fire-and-forget，只有恒成功 `/health`，无 metrics/alerts pipeline |
| P1-13 migration | 部分完成 | 103→114 个 migration；新增 fresh/re-run 测试，但 CI 可跳过且无 upgrade/old-app lane |
| P1-14 CI/发布 | 部分完成 | “只 build/publish”已失效；现有 quality job 是实质进步，但集成 skip、PC build、supply-chain gate 未补 |
| P1-15 契约/行为测试 | 仍存在 | OpenAPI 仍手工且核心资金域不完整；PC/mobile 大量源码文本测试 |
| P1-19 配置/Secret/HA | 仍存在 | examples 有进步，但实际本地 Secret、配置漏传、key 无版本、资源/HA 未闭环 |
| P1-20 DR | 仍存在 | 只有卷/AOF/文字提醒，无 PITR/跨存储 restore drill |
| P1-21 Trellis 真相 | 仍存在 | active 总数仍 124，`in_progress` 66→67，完成状态和 archive 元数据继续漂移 |
| P2-01/P2-02 | 仍存在 | permissive CORS；验证码状态先提交后 SMTP，无交付状态/outbox |
| P2-06/P2-07 | 部分改善 | `.codegraph` 已 ignore；admin routes 已 lazy，但共享 mega chunk 仍在 |
| P2-08 | 仍存在 | logging/error/database 核心 spec 仍保留模板占位 |

> P1-04–10、12、16–17 与 P2-03–05 的业务/客户端深审不在本文件有限复核范围内；本文件不据缺少证据宣称其已关闭。

## Current Strengths

- `.github/workflows/docker-image.yml:16-159` 已让 PR/publish 依赖 quality gate，并采用最小 job 权限、native amd64/arm64 和 digest 合并；比 2026-08-24 基线明显改善。
- `Dockerfile:31-36,49-80` 使用 Cargo `--locked`、npm `ci`、非 root UID 10001、Tini、healthcheck；`docker/supervise.sh:9-43` 能在 Rust/Nginx 任一退出时关闭 sibling 并退出容器。
- `docker-compose.example.yml:22-134` 有四依赖 healthcheck、migration 完成后启动 API、持久卷；1Panel example 默认 loopback 绑定并有日志轮转（`:33-37,67-71`）。
- `src/bin/exchange-migrate.rs:13-39` 的独立嵌入式 migrator 与单连接失败退出边界清晰；fresh/re-run 测试已存在，只需进入不可 skip 的 CI。
- 事件链已有事务 outbox、持久 message id、inbox 幂等/租约、retry/dead-letter 和管理员重排基础；缺口主要在 broker confirm/topology 与部署闭环。
- Web 管理端测试质量相对好：52 个 test 文件/296 个 test 调用，37 个文件使用组件测试工具；路由已按页面 lazy load。
- `src/infra/admin_request_context.rs:31-57` 已为 admin 审计提供 request-id；`src/modules/auth/mod.rs:373-402` 已修正会话枚举伪成功；admin session version 是可扩展到 user/agent 的现成模式。

## Files Found

- `pc/postcss.config.js` — PC PostCSS 配置及新增 P0 混淆 loader。
- `pc/package.json`、`pc/src/main.ts` — Vite 构建与 CSS 导入执行入口。
- `.github/workflows/docker-image.yml`、`scripts/p0-release-gate.sh` — 当前质量、构建与发布门禁。
- `Dockerfile`、`docker/supervise.sh`、`docker/nginx.conf` — 一体化镜像、进程监管和反向代理边界。
- `docker-compose.yml`、`docker-compose.example.yml`、`docker-compose.1panel.example.yml`、本地忽略的 `docker-compose.1panel.yml` — 开发/生产/1Panel 编排与当前本地部署证据。
- `src/config.rs`、`src/infra/secrets.rs` — typed settings、共享 Secret 和加密 envelope。
- `src/main.rs`、`src/lib.rs`、`src/infra/admin_request_context.rs` — worker 生命周期、健康接口和 request-id 范围。
- `src/modules/events/service/rabbitmq.rs`、`production_dispatch.rs`、`src/workers/event_inbox.rs` — RabbitMQ 发布/消费/topology 与钱包初始化链。
- `src/error.rs`、`tests/unit_src/src_modules_auth_routes_tests.rs` — 5xx public response 与测试固化。
- `src/bin/exchange-migrate.rs`、`tests/withdrawal_quote_migration.rs`、`docs/deployment/docker.md` — migration 运行、fresh/re-run 测试与回滚说明。
- `src/modules/auth/mod.rs`、`src/modules/auth/application.rs`、`src/modules/user/application.rs`、`src/modules/admin/application/{users,agents}.rs` — 会话撤销与跨存储提交窗口。
- `.trellis/workflow.md`、`.trellis/tasks/**/task.json`、当前任务 `implement.jsonl`/`check.jsonl`、`docs/superpowers/PROGRESS.md` — Trellis 生命周期与交付真相漂移。
- `.trellis/spec/backend/{container-delivery,quality-guidelines,database-guidelines,error-handling,logging-guidelines}.md` — 相关交付/质量合同及模板缺口。

## External References

- [Vite CSS/PostCSS](https://vite.dev/guide/features.html#postcss)：存在有效 PostCSS config 时会自动应用到导入 CSS，支持 DSQ-01 执行路径判断。
- [GitHub Actions secure use](https://docs.github.com/en/actions/reference/security/secure-use)：完整 commit SHA 是 Action 不可变 pin 的方式。
- [GitHub artifact attestations](https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/use-artifact-attestations)：容器 digest 的 build provenance/SBOM attestation 与验证。
- [Docker Compose trust model](https://docs.docker.com/compose/trust-model/) 与 [Compose secrets](https://docs.docker.com/reference/compose-file/secrets/)：digest pin 与显式 Secret grant。
- [Docker resource constraints](https://docs.docker.com/engine/containers/resource_constraints/)：容器默认无 CPU/内存限制。
- [RabbitMQ publishers](https://www.rabbitmq.com/docs/4.2/publishers)、[reliability](https://www.rabbitmq.com/docs/reliability) 与 [definitions](https://www.rabbitmq.com/docs/definitions)：confirm、mandatory return、topology 导入/备份。
- [MySQL 8.4 point-in-time recovery](https://dev.mysql.com/doc/refman/8.4/en/point-in-time-recovery-binlog.html)：full backup 后应用 binlog 的 PITR 基线。
- [MongoDB self-managed backup methods](https://www.mongodb.com/docs/manual/core/backups/) 与 [backup/restore tools](https://www.mongodb.com/docs/manual/tutorial/backup-and-restore-tools/)：一致性 snapshot/oplog 与恢复验证要求。

## Related Specs

- `.trellis/spec/backend/container-delivery.md`
- `.trellis/spec/backend/quality-guidelines.md`
- `.trellis/spec/backend/database-guidelines.md`
- `.trellis/spec/backend/error-handling.md`
- `.trellis/spec/backend/logging-guidelines.md`
- `.trellis/spec/backend/auth-sessions.md`
- `.trellis/spec/backend/realtime-websockets.md`

## Caveats / Not Found

- 本轮按用户要求停止扩展扫描；只保留上述 11 条高置信度结论，没有执行应用代码、PC Vite/PostCSS、测试、Docker、数据库、消息队列或任何可疑 payload。
- 研究 agent 禁止 Git 操作，因此结论针对 **当前 checkout**；DSQ-01 的 commit/author/PR/远端分支 provenance 需要主会话或事件响应人员另行取得。
- 本地 `docker-compose.1panel.yml` 被 ignore；它证明当前工作区存在运行 Secret 边界问题，但不能据此断言该文件在 Git HEAD 或其他环境存在。
- 未取得 GitHub branch protection/environment approval、GHCR attestation、1Panel/云 HA、WAF/CORS、监控告警、备份作业、生产数据与 restore drill 的运行时证据；相关验收状态均保持未证实。
- 未发现通用 backup/restore/PITR runbook、Prometheus/OTel pipeline、SBOM/签名/attestation、dependency update bot、`SECURITY.md`、`CODEOWNERS` 或 `CONTRIBUTING.md`。
