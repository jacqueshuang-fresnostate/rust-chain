# Research: Rust 后端架构与代码结构审计

- Query: 只读审计 Rust 后端架构与代码结构，覆盖 `src/modules`、`src/workers`、`src/api`、application/domain/infrastructure 边界、超大文件/函数、跨域依赖、重复模式、错误/事务边界与测试组织。
- Scope: internal
- Date: 2026-08-24

## Findings

### 1. 审计口径与结论摘要

- **事实**：均来自当前工作区源码、规范、测试和部署文件的静态读取；路径、行号或符号可直接复核。
- **推断**：基于事实对运行影响的判断；均明确标注，并给出成立条件与限制。
- **规模**：`src/` 共有 331 个 Rust 文件、109,602 行；其中 `src/modules/` 289 个文件、95,299 行，`src/workers/` 14 个文件、7,789 行，`src/infra/` 9 个文件、641 行，`src/openapi/` 9 个文件、4,011 行。
- **API 位置**：仓库中不存在 `src/api/`。API 组合根实际位于 `src/lib.rs:23-82` 的 `build_router`，各限界上下文使用 `routes.rs`，OpenAPI 位于 `src/openapi/`。

| 优先级 | 结论 | 主要边界 |
|---|---|---|
| P0 | 固定公开默认管理员会在全新数据库迁移时自动创建，并获得 `*` 权限 | 启动/部署/认证 |
| P0 | 注册后的钱包初始化依赖一条可能被终态误标为已发布、且消费端默认停用的消息链 | 事务/outbox/RabbitMQ/钱包 |
| P1 | 全局错误响应把底层基础设施原文回显给客户端 | 错误/HTTP |
| P1 | 会话撤销在令牌枚举失败时按空列表成功返回，旧访问会话可继续有效 | 认证/跨存储一致性 |
| P1 | 钱包账户与流水由 11 个非钱包上下文及 workers 直接写 SQL | 资金域/事务/跨域依赖 |
| P1 | `admin` 成为 25K 行“影子业务域”，直接依赖多域基础设施和传输 DTO | 限界上下文/分层 |
| P1 | 架构守卫未覆盖 application/infra/presentation 与跨域依赖，已有反向依赖不会被拦截 | 架构门禁 |
| P1 | 14 个后台任务多为 fire-and-forget，健康检查恒定成功，无统一就绪度/积压状态 | workers/运维 |
| P1 | 现有 CI 只构建镜像；数据库/Redis 集成测试会在环境缺失时成功跳过 | 测试/交付门禁 |
| P1 | 21 个生产文件达到 1,000 行，30 个函数达到 100 行；高风险资金函数含 10～20 次 await | 可维护性/变更风险 |

---

### 2. P0-01：固定默认超级管理员自动进入生产迁移链

**事实**

- `src/bootstrap.rs:14-20` 固定内置角色 `super_admin`、用户名 `admin`、口令 `Qaz123456@`；`BootstrapAdminConfig::built_in_defaults` 位于 `src/bootstrap.rs:32-41`。
- `BootstrapAdminConfig::from_env` 在用户名或口令缺失/为空时回落到上述固定值：`src/bootstrap.rs:43-51`。
- `bootstrap_default_admin_while_locked` 在管理员表为空时创建 `JSON_ARRAY('*')` 角色并创建 active 管理员：`src/bootstrap.rs:154-202`，关键 SQL 在 `src/bootstrap.rs:183`、`src/bootstrap.rs:192-199`。
- 迁移二进制每次执行 migrations 后都调用该引导逻辑：`src/bin/exchange-migrate.rs:35-52`。
- 三份 Compose 配置继续提供同一固定回退值：`docker-compose.example.yml:97-102`、`docker-compose.1panel.example.yml:44-49`、`docker-compose.1panel.yml:67-72`。

**推断**

- 条件是“全新数据库没有管理员，且部署方没有显式覆盖口令”。满足条件时，部署会产生一组公开可预测、具有通配权限的登录凭据。已有任意管理员时逻辑会跳过，因此风险集中在首次部署、灾备恢复和新环境复制。

**影响**

- 外部调用者可在运维改密前取得后台全部权限；资产配置、人工充值、风控、用户和系统配置均处于同一控制面。该问题具备直接权限接管路径，定为 P0。

**增量优化方案**

1. 在 `APP_ENV=production` 时要求显式设置 `BOOTSTRAP_ADMIN_ENABLED=true` 和非默认高强度口令；缺失或命中已知默认值时迁移进程失败关闭。
2. Compose 将 `${BOOTSTRAP_ADMIN_PASSWORD:-...}` 改为 `${BOOTSTRAP_ADMIN_PASSWORD:?...}`，开发环境通过单独 fixture 显式注入。
3. 给首个账号增加 `must_rotate_password`/一次性引导状态；首次成功登录后才能执行其他后台操作。
4. 增加存量巡检：识别默认用户名与通配角色组合，要求人工确认和轮换；日志只记录结果，不记录秘密。

**验收方法**

- production 模式下，未配置、空值或固定默认口令均使 `exchange-migrate` 非零退出。
- 仓库生产 Compose 与 `src/` 不再出现固定默认口令字面量。
- 使用显式一次性秘密可创建首个管理员；固定默认凭据登录失败；首次登录前除改密外的后台路由均被拒绝。
- 已有管理员数据库重复迁移仍保持幂等，不创建第二个账号。

**工作量 / 依赖**

- 工作量：S，约 1～2 天。
- 依赖：部署 Secret 管理、Compose/1Panel 参数迁移、认证表字段或一次性状态设计、迁移集成测试。

---

### 3. P0-02：`user.created` 钱包初始化消息存在“未确认即终态 + 消费默认停用”窗口

**事实**

- 注册用例在同一 MySQL 事务中创建用户、邀请码/推荐关系和 outbox，提交后立即签发令牌；未同步创建钱包：`register_user_with_email_code`，`src/modules/auth/application.rs:461-520`，outbox 写入在 `:514`、提交在 `:516`。
- `user.created` 是生产 dispatch 中唯一有业务副作用的事件，副作用是调用钱包初始化端口：`src/modules/events/service/production_dispatch.rs:138-172`。
- 钱包初始化适配器以 `INSERT IGNORE ... SELECT assets` 创建全资产零余额账户：`src/modules/events/infrastructure.rs:260-303`。
- RabbitMQ publisher 使用 `BasicPublishOptions::default()`，未调用 `confirm_select`，源码注释明确结果为 `NotRequested`、不能证明 broker 已持久接收：`src/modules/events/service/rabbitmq.rs:107-137`。
- publisher 返回 `Ok(())` 后，outbox 服务立即调用 `mark_published`：`src/modules/events/service/outbox.rs:137-156`；基础设施注释也明确该终态不代表 broker 已持久接收：`src/modules/events/infrastructure.rs:367-385`。
- consumer 要求队列由外部预先声明和绑定，本类型不负责拓扑：`src/modules/events/service/rabbitmq.rs:141-155`。
- `EVENT_INBOX_QUEUE_NAME` 缺失或空白时 worker 视为显式停用：`src/workers/event_inbox.rs:67-106`；`src/main.rs:284-311` 只有配置存在才启动实时消费和补偿扫描。当前样本 `.env:22` 为空。
- 静态检索未在仓库生产代码或 Compose 中找到 `queue_declare`、`queue_bind`、`confirm_select`，也未找到 inbox 队列环境变量的部署注入。
- 本地锁定依赖 Lapin 2.5.5；其 `BasicPublishOptions::default()` 中 `mandatory` 默认为 false，且未请求 confirm 时返回 `Confirmation::NotRequested`：`$HOME/.cargo/registry/src/.../lapin-2.5.5/src/generated.rs:28-34`、`publisher_confirm.rs:15-20`。

**推断**

- 若外部平台没有额外预建并绑定队列，或 broker 在发送/持久化窗口发生故障，publisher 仍可能返回成功，数据库随后把 outbox 置为 `published`，该事件不再进入重试；注册请求已返回令牌，但基线钱包账户未创建。
- 某些业务会按需 `INSERT IGNORE` 钱包账户，因此影响不一定表现为所有接口立即失败；但“新用户拥有全部资产账户”的初始化不变量已失去可靠保证，且不同用户会因首次业务路径产生不同账户集合。

**影响**

- 账户初始化事件可能永久丢失，且数据库状态错误地显示已发布，普通重试无法修复。它跨越认证、事件和资金域，属于数据完整性与可恢复性 P0。

**增量优化方案**

1. publisher 启用 `confirm_select`，显式处理 Ack/Nack；启用 mandatory publish 并处理 returned message，只有 Ack 且无 return 才允许 `mark_published`。
2. 由服务启动或独立、版本化的基础设施脚本声明 durable exchange、queue、binding；production 下 publisher 开启而 inbox/topology 缺失时 readiness 失败。
3. 将 inbox 是否启用从“空值静默停用”改为按部署角色显式声明；API+worker 单体部署默认要求 consumer，纯 API 部署则要求独立 worker 的可观测证明。
4. 增加修复任务：按 `users × assets` 与 `wallet_accounts` 反连接补齐账户，并对受影响 outbox 状态做审计而非盲目重发。

**验收方法**

- 新用户注册后最终且幂等地拥有每个资产的一条钱包账户。
- 未绑定 routing key、broker Nack、连接在 publish 后断开时，outbox 保持 pending/retry，不进入 published。
- 在“broker Ack 后、数据库标记前”注入崩溃，允许重复投递但由 inbox 幂等吸收，钱包账户不重复。
- Compose 启动后自动断言 exchange/queue/binding、consumer 与补偿扫描存在；反连接巡检结果为 0。

**工作量 / 依赖**

- 工作量：M，约 3～5 天；存量修复另计 1～2 天。
- 依赖：RabbitMQ 拓扑所有权、Lapin publisher confirm/return 测试、MySQL+RabbitMQ 集成环境、部署角色定义。

---

### 4. P1-01：基础设施错误原文进入统一 HTTP 响应

**事实**

- `AppError` 的 Config/SQLx/Mongo/Redis/RabbitMQ/Internal 变体保存并通过 `Display` 输出底层消息：`src/error.rs:12-55`。
- `IntoResponse` 对所有错误统一使用 `message: self.to_string()`：`src/error.rs:131-147`；源码注释 `:134` 明确认可基础设施原文会被透出。
- 单元测试已把内部部署细节固化为外部契约：`tests/unit_src/src_modules_auth_routes_tests.rs:98-113` 断言 500 响应包含 `mysql pool is not configured for auth persistence`。

**推断**

- SQL、缓存、消息队列或第三方适配器错误可能包含表名、约束名、主机、查询片段和上游响应；当前全局映射没有最终脱敏层。

**影响**

- 产生信息披露，并让客户端依赖不稳定的内部文案；基础设施调整会无意改变 API 契约，告警排障也缺少稳定 error id。

**增量优化方案**

1. 为 `AppError` 增加 `public_message()`；Config/Database/Mongo/Redis/RabbitMq/Internal 对外只返回通用文案和稳定 code。
2. 在服务端结构化日志记录完整 error chain、request/correlation id；响应返回同一 error id 便于关联。
3. 保留 Validation/Conflict/显式 `Api` 的面向用户文案，审计所有 `AppError::Internal(format!(...))` 调用点。

**验收方法**

- 注入含表名、连接串片段和秘密标记的 SQLx/Redis/Internal 错误，响应体均不含原文，服务端日志可按 error id 找到完整原因。
- 既有状态码和机器 code 保持稳定；Validation/Conflict 的业务文案回归测试通过。

**工作量 / 依赖**

- 工作量：S～M，约 2～3 天。
- 依赖：前端是否错误匹配 message 的盘点、tracing 字段约定、OpenAPI 错误模型更新。

---

### 5. P1-02：强制下线在令牌枚举失败时按成功处理

**事实**

- `revoke_actor_auth_sessions` 调用 Sa-Token 枚举主体令牌，但对错误使用 `.unwrap_or_default()`：`src/modules/auth/mod.rs:320-349`，关键位置 `:327-334`；枚举失败即得到空列表并继续返回成功。
- 用户通用鉴权只验证会话/令牌和 scope，不回查账号表：`src/modules/auth/mod.rs:465-485`；代理鉴权注释明确停用后尚未过期令牌仍可通过，依赖主动撤销：`src/modules/auth/mod.rs:574-588`。
- 管理员停用用户在数据库事务提交后调用该函数：`src/modules/admin/application/users.rs:105-147`；代理改密与后台重置分别在 `src/modules/agent/application.rs:201-241`、`src/modules/admin/application/agents.rs:160-216`；密码重置在 `src/modules/auth/application.rs:1023-1051`。
- 默认访问会话时长为 900 秒：`src/config.rs:205-208`。MySQL 刷新令牌虽在事务中撤销，但已签发访问会话属于另一存储边界。

**推断**

- Sa-Token/Redis 在枚举阶段短暂故障时，密码重置、代理改密或用户停用可能成功提交并返回，而已泄露的访问会话继续有效至过期；部分资金用例会自行回查 active 状态，但鉴权层没有全局保证，代理路径尤其依赖撤销成功。

**影响**

- 安全操作的“requires_relogin/强制下线”语义不可信，攻击窗口最长为当前访问 TTL，属于认证撤销一致性 P1。

**增量优化方案**

1. 去掉 `unwrap_or_default`，枚举失败必须形成显式错误、指标和待重试记录，禁止伪造撤销成功。
2. 增加数据库 `session_epoch`/`credentials_changed_at`，签发时写入会话，鉴权时比较；改密/停用与 epoch 增长同事务提交，使跨 Redis 故障仍能拒绝旧会话。
3. 为 Sa-Token 和项目刷新令牌撤销建立幂等补偿 worker，记录最后成功时间和未完成主体数。

**验收方法**

- 故障注入令牌枚举错误时，安全操作不得报告“全部下线”；待补偿状态和告警可见。
- 在 Redis/Sa-Token 故障期间完成改密/停用后，旧 user/agent token 下一次请求即被拒绝；恢复后补偿任务幂等清理会话。
- admin、user、agent 三种主体和重复撤销均有测试。

**工作量 / 依赖**

- 工作量：M～L，约 4～7 天。
- 依赖：认证表迁移、Sa-Token 适配器故障注入、Redis 集成测试、前端重新登录语义。

---

### 6. P1-03：钱包持久化没有单一所有者

**事实**

- 静态扫描发现钱包上下文之外共有 **54 条**直接 `INSERT/UPDATE/DELETE wallet_accounts|wallet_ledger`，分布在 **17 个文件、11 个业务上下文及 workers**。
- 代表性写点：
  - spot：`src/modules/spot/infrastructure/wallet_accounts.rs:80,200,260,338,394`
  - margin：`src/modules/margin/infrastructure/transfers.rs:72,160`、`settlement.rs:114,212`、`ledger.rs:142`
  - loan：`src/modules/loan/infrastructure.rs:869,920,961,1008,1058,1100`
  - prediction：`src/modules/prediction/infrastructure.rs:1510,1597,1673,1741,1787`
  - new_coin：`src/modules/new_coin/infrastructure.rs:556,566,947,1060,1100,1159,1264`
  - convert：`src/modules/convert/infrastructure.rs:526,532,560,626`
  - earn：`src/modules/earn/infrastructure.rs:694,701,753,760`
  - quick_recharge：`src/modules/quick_recharge/infrastructure.rs:784,792,913`
  - seconds_contract：`src/modules/seconds_contract/infrastructure.rs:804,824`
  - admin/events：`src/modules/admin/infrastructure/wallet_assets.rs:327,344,896,927,956`、`src/modules/events/infrastructure.rs:270`
  - workers：`src/workers/unlock_scanner.rs:394,409`、`seconds_contract_settlement.rs:475,487`、`earn_auto_redemption.rs:245,257`。
- 钱包精度规范要求按 `assets.precision_scale` 量化，并要求 ledger amount/snapshot 与账户值一致：`.trellis/spec/backend/wallet-amount-precision.md:7-22`。

**推断**

- 静态证据证明所有权和实现分散，但不直接证明当前已有余额错误。风险在于锁顺序、精度、账户初始化、ledger 元数据、幂等键和快照不变量的修复必须同步修改多个域，遗漏概率随路径增加。

**影响**

- 任一资金不变量调整都形成大范围跨域变更；并发死锁、精度漂移或“余额已变但流水口径不同”的回归难以由单一契约测试覆盖，定为资金架构 P1。

**增量优化方案**

1. 在 wallet repository/service 定义事务感知的 `WalletPostingPort`：锁账户、credit/debit/freeze/unfreeze、写 ledger、校验精度；接口接收调用方持有的 `&mut Transaction`，不破坏订单与资金的同事务原子性。
2. 先迁移 quick_recharge/earn/seconds 等较小路径，再迁移 prediction/loan/new_coin，最后处理 spot/margin 高并发锁序。
3. 为所有实现运行同一组 contract tests，明确账户锁序、资产精度和 ledger reference/idempotency 规则。

**验收方法**

- 除 wallet infrastructure 和有截止日期的迁移适配层外，静态守卫禁止直接写两张钱包表。
- 每个业务路径通过共享的精度、余额守恒、流水快照、重复请求和并发锁序测试。
- 路由 JSON、SQL schema 和订单+资金原子提交语义保持不变。

**工作量 / 依赖**

- 工作量：XL，分阶段约 4～8 周。
- 依赖：事务端口设计、MySQL 并发集成环境、资金 reference 词典、各业务域 owner 协作。

---

### 7. P1-04：`admin` 限界上下文成为跨域“影子实现”

**事实**

- `src/modules/admin/` 有 84 个 Rust 文件、25,601 行；其中 service 5,262、application 6,666、infrastructure 7,664、presentation 2,836、routes 2,439 行，是最大上下文。
- `src/modules/admin/application.rs:13-130` 大规模直接导入本域 concrete infrastructure 函数；`:297-322` 又直接依赖 agent/auth/kyc/new_coin/platform/security 和 worker 类型。
- `src/modules/admin/infrastructure.rs:12-55` 反向依赖 admin presentation DTO，并引用 agent/market/security/user/wallet 类型。
- `src/modules/admin/service.rs:6-47` 依赖 presentation 请求/响应、多个其他业务域及 `workers::market_feed`。

**推断**

- admin 不只是后台传输适配，而是在平行实现钱包、行情、新币、代理、用户和系统配置用例；同一业务规则可能分别存在于 owner context 和 admin context，修改一侧容易遗漏另一侧。

**影响**

- 改动传播面大、编译耦合强、后台和用户端规则可能漂移；任何拆分都同时触及 DTO、SQL、审计和 worker，形成持续高变更风险。

**增量优化方案**

1. 按资产、钱包、行情、新币、代理、用户、系统配置建立能力清单，明确 owner context。
2. 将后台命令/查询迁入 owner context 的 application public API；admin routes 保留 URL、鉴权和审计上下文，只做委派与响应映射。
3. 每次只迁移一个能力，保留兼容 façade，先锁定 HTTP JSON、SQL 与审计快照测试，再删除 admin 内重复实现。
4. worker 控制通过明确 supervisor port 暴露，禁止 service 直接引用 worker runtime 类型。

**验收方法**

- admin application 不再直接导入 concrete infrastructure；admin service 不依赖 presentation 或 workers。
- 钱包/行情/新币等后台操作由对应 owner context 的用例执行，用户端与后台端共享同一业务规则测试。
- 现有后台路由契约、权限和审计记录快照保持兼容。

**工作量 / 依赖**

- 工作量：XL，约 4～8 周，适合按能力持续交付。
- 依赖：owner context public API 设计、HTTP contract snapshot、数据库集成测试、权限/审计 ADR。

---

### 8. P1-05：声明的分层方向未被架构测试完整执行

**事实**

- 规范要求 transport → application → service/domain/repository ports → infrastructure adapters，并禁止反向依赖：`.trellis/spec/backend/directory-structure.md:56-70`。
- `tests/backend_architecture.rs:89-119` 仅注册 routes、domain、repository、service 四类依赖测试；实现检查位于 `:348-473`。没有 application→infrastructure、infrastructure→presentation、service→presentation、跨 context 或 worker/composition 规则。
- 当前不会被守卫拦截的实例：
  - service→presentation/cross-context：`src/modules/spot/service.rs:6-17`、`src/modules/kyc/service.rs:13-14`、`src/modules/market/service.rs:8-11`。
  - infrastructure→presentation：`src/modules/loan/infrastructure.rs:13-20`、`src/modules/margin/infrastructure/product_config.rs:9-16`。
  - application→concrete infrastructure/SQL：`src/modules/spot/application/queries.rs:3-18`、`src/modules/auth/application.rs:926-945`、`src/modules/admin/application/agents.rs:174-190`。
  - domain→foreign context：`src/modules/spot/domain.rs:10`、`src/modules/new_coin/domain.rs:13-15`、`src/modules/agent/domain.rs:8-11`。
  - repository 通过 Axum re-export 使用 `async_trait`：`src/modules/auth/repository.rs:18-26`、`src/modules/events/repository.rs:16-23`；`Cargo.toml` 没有直接 `async-trait` 依赖。
- `AppState` 把 MySQL/Mongo/Redis/auth/RabbitMQ/hub/supervisor/email 全部建模为 `Option`：`src/state.rs:19-32`；至少 14 个 application 文件直接引用 `AppState`，例如 `src/modules/spot/application/queries.rs:16-25`、`src/modules/news/application.rs:15-30`。

**推断**

- 架构测试可保持绿色，同时 DTO、具体存储和全局运行时状态继续向内层扩散；生产依赖缺失被推迟到请求运行时，测试可构造生产中不会出现的“半状态”。

**影响**

- 分层只形成目录命名而非可执行依赖规则；重构难以隔离，纯业务测试需要装配传输/存储类型，错误也更晚暴露。

**增量优化方案**

1. 扩展架构守卫：禁止 service/infra→presentation、domain→foreign context、application 中 raw SQL；跨域只允许经过显式 `public_api`/port。
2. 直接依赖 `async-trait`，让 repository 端口脱离 Axum。
3. 以中立 command/result/row 类型分离 application、presentation 和 infrastructure；先从 news/market 等小上下文试点。
4. 将 production `RuntimeResources` 设为必需依赖，测试使用独立 builder；application 接收模块级 context/ports，而非整个 `AppState`。

**验收方法**

- 在 fixture 中植入 service→presentation 或 infra→presentation 导入时，`backend_architecture` 必须失败并报告路径/行号。
- 生产路由组合在缺少必需依赖时编译/启动失败，而不是请求时返回 “pool is not configured”。
- 目标上下文 application/service 单测无需构造 Axum DTO、`AppState` 或 concrete SQL adapter。

**工作量 / 依赖**

- 工作量：L，约 2～4 周，按上下文渐进执行。
- 依赖：模块 public API 约定、测试 builder、无宽泛 allowlist 的迁移策略、P1-04 admin 拆分配合。

---

### 9. P1-06：workers 缺少统一生命周期、就绪度与积压治理

**事实**

- `src/main.rs:1-3` 明确说明后台协程以 fire-and-forget 启动；`main` 在 `src/main.rs:59-310` 有 14 处 `tokio::spawn`，除 market feed 自有 supervisor 外未保存 JoinHandle。
- worker 顶层退出后通常只写 error 日志，例如 outbox `src/main.rs:97-107`、秒合约结算 `:159-176`、强平 `:196-210`、钱包链 `:263-272`、inbox `:284-310`。
- 多数循环会捕获单轮错误并继续，这是正向韧性设计：`src/workers/event_outbox.rs:31-41`、`seconds_contract_settlement.rs:316-338`、`margin_liquidation.rs:430-449`、`earn_auto_redemption.rs:166-183`。
- `/health` 无条件返回 ok，不探测依赖或 worker：`src/lib.rs:89-93`。`AppState` 只有 market feed supervisor 状态：`src/state.rs:27-31`。

**推断**

- panic、初始化后退出、持续每轮失败或 backlog 不收敛都不会影响就绪状态；负载均衡仍会把实例视为健康。结算、强平、解禁、逾期、链上入账可在进程存活时长期停摆。

**影响**

- 运维只能依赖日志发现后台业务停滞，缺少机器可判定的 SLO；金融状态延迟和积压会被恒定 200 健康检查掩盖。

**增量优化方案**

1. 引入 `WorkerRegistry/Supervisor`，保存名称、handle、required/optional、最后成功/错误、连续失败、当前 backlog 与退出原因。
2. 区分 `/health`（liveness）与 `/ready`（必需依赖和必需 worker）；明确 panic 的 restart 或 fail-process 策略。
3. 为每个 worker 输出统一 metrics：cycle latency、scanned/succeeded/failed、oldest pending age、dead-letter/backlog。
4. 启动配置采用显式部署角色，关键 worker 被关闭时必须在 readiness 和启动日志中可见。

**验收方法**

- 注入 worker panic、连续查询失败和积压超阈值时，registry 状态更新，`/ready` 返回 503 或按策略终止实例；`/health` 仍只表示进程存活。
- worker 正常恢复后 readiness 自动恢复；required/optional 配置矩阵有单元测试。
- 每个金融 worker 的最后成功时间与 backlog 可由指标查询。

**工作量 / 依赖**

- 工作量：M～L，约 5～10 天。
- 依赖：监控/告警后端、部署就绪探针、各 worker backlog 查询与重启策略。

---

### 10. P1-07：测试数量可观，但 CI 与集成环境允许关键路径未执行

**事实**

- `tests/` 有 113 个 Rust 文件、75,088 行、842 个 `#[test]/#[tokio::test]`；`tests/unit_src/` 有 57 个文件、8,344 行、280 个测试属性。
- 大型测试文件包括 `tests/admin_routes.rs` 16,596 行/92 个测试、`tests/spot_routes.rs` 7,812 行/55 个、`tests/margin_routes.rs` 4,825 行/34 个。
- 集成 helper 在缺少环境变量时返回 `None` 并打印 skipping，例如 `tests/spot_routes.rs:95-123`、`tests/margin_routes.rs:93-119`、`tests/admin_routes.rs:130-145`；同类模式遍布 worker、migration、Redis/Mongo 测试。
- 唯一 GitHub Actions 文件是 `.github/workflows/docker-image.yml`；PR job `:15-49` 只执行多架构 Docker build，没有 `cargo fmt/check/clippy/test`，也没有 MySQL/Redis/RabbitMQ service。
- 已有正向组织：`tests/backend_architecture.rs:80-86` 强制生产源码单测放入 `tests/unit_src`；文件大小守卫位于 `tests/backend_architecture.rs:145-199`。

**推断**

- “镜像构建成功”不能证明 842 个测试被运行；集成 job 若漏配环境，测试进程仍可能绿色退出。资金 SQL、事务锁序、RabbitMQ 投递与 migration 回归存在未执行却绿色的窗口。

**影响**

- 关键回归可进入 main 并发布镜像；巨大测试文件还提高 fixture 耦合与冲突率，失败定位慢。

**增量优化方案**

1. 新增 required `backend-quality`：fmt、check、clippy、纯单元、architecture tests。
2. 新增 MySQL+Redis 集成 lane；RabbitMQ 单独执行 outbox/inbox 端到端；CI 模式缺少必需 URL 时直接失败，不走 skip。
3. 以 feature/profile 区分纯单元与集成测试，而不是在测试体内“成功跳过”。
4. 按 capability 拆分 admin/spot/margin route tests，共享 fixture crate/module，保留独立数据库隔离。

**验收方法**

- PR required checks 明确展示各 lane 的非零测试数；故意破坏 SQL、架构导入或消息绑定时对应 job 必须失败。
- 集成 CI 日志不出现 “DATABASE_URL is not set” 成功跳过。
- 测试报告可按模块定位，admin/spot/margin 不再由单个超大 target 承担全部场景。

**工作量 / 依赖**

- 工作量：M，约 3～5 天；拆分巨型测试可后续持续进行。
- 依赖：GitHub service containers、缓存与运行时预算、RabbitMQ/MySQL 隔离、required check 设置权限。

---

### 11. P1-08：超大文件和长资金函数超过现有守卫的有效覆盖

**事实**

- 共有 21 个生产 Rust 文件达到 1,000 行，6 个超过 1,200 行：

| 行数 | 文件 |
|---:|---|
| 1,830 | `src/modules/prediction/infrastructure.rs` |
| 1,590 | `src/modules/new_coin/infrastructure.rs` |
| 1,506 | `src/modules/auth/infrastructure.rs` |
| 1,363 | `src/modules/market/infrastructure/adapters/provider.rs` |
| 1,264 | `src/modules/market/infrastructure/adapters/feed.rs` |
| 1,212 | `src/modules/loan/infrastructure.rs` |
| 1,194 | `src/workers/market_feed.rs` |
| 1,184 | `src/modules/wallet/application.rs` |
| 1,175 | `src/workers/margin_liquidation.rs` |
| 1,170 | `src/modules/admin/infrastructure/wallet_assets.rs` |
| 1,156 | `src/modules/user/application.rs` |
| 1,150 | `src/modules/admin/infrastructure/system_config.rs` |
| 1,134 | `src/modules/admin/application/market.rs` |
| 1,109 | `src/modules/seconds_contract/infrastructure.rs` |
| 1,080 | `src/modules/auth/application.rs` |
| 1,058 | `src/modules/admin/infrastructure/market.rs` |
| 1,049 | `src/modules/events/infrastructure.rs` |
| 1,036 | `src/modules/user/infrastructure.rs` |
| 1,016 | `src/modules/quick_recharge/infrastructure.rs` |
| 1,014 | `src/modules/admin/service/system_config.rs` |
| 1,007 | `src/workers/kline_recovery.rs` |

- 架构测试只禁止全局超过 2,000 行，并只对少数根目录执行 1,200 行限制：`tests/backend_architecture.rs:145-199`；prediction/new_coin/auth/market/loan 等热点不在 1,200 守卫集合中。
- 文本级函数边界扫描统计 3,839 个函数，其中 30 个至少 100 行。高风险代表：
  - `main`：`src/main.rs:33-319`，287 行/22 个 await。
  - `liquidate_cross_account`：`src/workers/margin_liquidation.rs:699-881`，183 行/11 await。
  - `execute_admin_market_strategy_recovery`：`src/modules/admin/application/market.rs:409-575`，167 行/17 await。
  - `release_due_paid_unlock`：`src/modules/new_coin/infrastructure.rs:438-597`，160 行/12 await。
  - `execute_triggered_{buy,sell}_order_in_tx`：`src/modules/spot/application/triggering.rs:301-459,461-619`，各 159 行/19 await。
  - `handle_gmpay_notify`：`src/modules/quick_recharge/application.rs:428-581`，154 行。
  - `settle_spot_fill`：`src/modules/spot/application/settlement.rs:36-180`，145 行/20 await。
  - `open_margin_position`：`src/modules/margin/application/open_position.rs:75-213`，139 行/15 await。

**推断**

- 行数本身不证明缺陷；但长事务函数同时承担校验、锁定、资金过账、状态迁移、审计和事件决策，修改时更难确认所有错误路径与提交点，风险集中在强平、结算、解禁和下单。

**影响**

- 评审和单元隔离成本高，边界修复容易演变为大范围修改；现有 2,000 行门槛不足以阻止新的千行职责聚合。

**增量优化方案**

1. 先为目标函数补 characterization/事务失败注入测试，再按 validate → load/lock → plan → post → commit → publish 阶段抽取。
2. 优先拆 prediction/new_coin/auth infrastructure 和 margin/spot 资金函数；facade 只声明与 re-export，不复制逻辑。
3. 将 1,200 行守卫扩展至所有新 child module；对资金 mutation/worker 引入约 100 行函数预算，例外必须按符号说明理由。

**验收方法**

- 目标生产文件均低于 1,200 行；核心资金 mutation 函数低于约定阈值且事务开始/提交点唯一可见。
- 拆分前后 HTTP、SQL、事件、余额/流水和失败回滚测试一致；架构守卫可阻止再次膨胀。

**工作量 / 依赖**

- 工作量：L，约 3～6 周，按模块渐进。
- 依赖：P1-03 钱包端口、P1-05 分层守卫、集成测试门禁先行。

---

### 12. 其他结构事实与 P2 改进项

#### 12.1 模块规模与层分布

下表为物理行静态统计；层列只计算标准 `layer.rs` 与 `layer/`，总量还包含 `mod.rs` 和特殊职责文件。

| context | total LOC/files | domain | repository | service | application | infrastructure | presentation | routes |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| admin | 25601/84 | 141 | 334 | 5262 | 6666 | 7664 | 2836 | 2439 |
| auth | 4809/8 | 157 | 149 | 563 | 1080 | 1506 | 267 | 494 |
| events | 4478/16 | 46 | 110 | 2578 | 319 | 1049 | 166 | 176 |
| market | 6452/16 | 635 | 31 | 42 | 172 | 4057 | 238 | 134 |
| margin | 6569/22 | 493 | — | 195 | 2399 | 2504 | 499 | 461 |
| spot | 6260/20 | 543 | 118 | 901 | 1710 | 2526 | 209 | 223 |
| wallet | 5926/13 | 567 | 112 | 228 | 1184 | 3015 | 405 | 383 |
| new_coin | 4097/8 | 452 | 436 | 526 | 541 | 1590 | 301 | 219 |
| prediction | 4004/7 | — | 199 | 697 | 707 | 1830 | 482 | 73 |
| earn | 3523/8 | — | 97 | 929 | 794 | 887 | 316 | 321 |
| seconds_contract | 3235/7 | — | 167 | 671 | 687 | 1109 | 313 | 269 |
| quick_recharge | 3165/7 | — | 242 | 672 | 619 | 1016 | 357 | 228 |
| loan | 2994/7 | 44 | — | 384 | 682 | 1212 | 340 | 306 |
| user | 3213/8 | 42 | 41 | 208 | 1156 | 1036 | 227 | 496 |
| 其余 9 个上下文 | 11942/65 | 详见各目录 |  |  |  |  |  |  |

#### 12.2 事务边界的正向模式与不一致

- 静态统计有 182 个 `.begin().await`：application 153、infrastructure 17、workers 10、根/组合 2。主体上由 application 拥有事务，但部分 infrastructure 自行开事务，所有权并不统一。
- 正向模式：spot/margin/seconds 等核心路径在事务内锁订单与钱包、写流水，提交后才发进程内事件，例如 `src/modules/margin/application/open_position.rs:75-218`、`src/modules/seconds_contract/application.rs:274-422`；与 `.trellis/spec/backend/realtime-websockets.md` 的 post-commit 约定一致。
- 正向模式：worker 对每个候选重新开独立事务并在锁内复核状态，例如 `src/workers/loan_overdue.rs:55-101`、`src/workers/margin_liquidation.rs:430-449`。
- P2：四条邮件验证码路径先提交验证码/冷却记录再 SMTP；发送失败不会回滚且没有 email outbox：`src/modules/auth/application.rs:722-760,765-806`、`src/modules/user/application.rs:333-399,1015-1067`。短期应把失败行标记为 `delivery_failed` 并排除冷却，长期采用可重试、保护验证码明文的邮件 outbox。

#### 12.3 重复模式

- `user_id_from_subject` 9 份、`admin_id_from_subject` 9 份，主体解析逻辑基本相同；代表位置 `src/modules/quick_recharge/service.rs:486-503`、`src/modules/seconds_contract/service.rs:630-646`、`src/modules/loan/service.rs:366-383`。
- `mysql_pool` 13 份、`route_limit` 13 份、`route_offset` 10 份、`optional_string` 17 份、`required_string` 6 份。
- P2 增量方案：在 auth domain 提供类型化 `ActorSubject::{user,admin,agent}`；提供参数化 `PageWindow::new(default,max,offset_max)` 和字符串归一化小工具。分页上限确有 100/200/10,000/100,000 差异，抽取时必须保留策略参数，不能用一个常量覆盖所有域。

#### 12.4 API 组合与 CORS

- `src/lib.rs:23-82` 集中组合 user/admin/agent 三棵路由，结构清晰；各模块 routes 较薄是已有优点。
- `src/lib.rs:27,80` 使用 `CorsLayer::permissive()` 并把收敛责任交给网关。该外部契约未在代码中验证；P2 建议 production 使用明确 origin/method/header 配置，并为网关旁路部署增加启动检查。
- `src/openapi/` 是文档聚合，不替代缺失的 `src/api/`；若项目约定需要 API 层，应优先把 `src/lib.rs::build_router` 视为 composition root，而不是机械新增空目录。

### 13. Files found

| 路径 | 一句话说明 |
|---|---|
| `src/lib.rs` | HTTP 路由组合根、宽松 CORS 与恒定 liveness health。 |
| `src/main.rs` | 外部依赖装配与 14 个后台协程启动。 |
| `src/state.rs` | 全局可选 service locator。 |
| `src/error.rs` | 统一错误枚举与 HTTP 映射。 |
| `src/bootstrap.rs` | 首个管理员引导与固定默认凭据。 |
| `src/modules/` | 23 个业务上下文，主要 DDD 命名层。 |
| `src/modules/admin/` | 最大聚合上下文及跨域后台实现。 |
| `src/modules/events/` | outbox/inbox、RabbitMQ 与进程内 WebSocket 广播。 |
| `src/modules/wallet/` | 钱包领域规则、端口与部分 MySQL 适配器。 |
| `src/workers/` | 事件、结算、强平、解禁、行情、链上等后台循环。 |
| `src/openapi/` | OpenAPI schema 与路由聚合。 |
| `tests/backend_architecture.rs` | 当前架构、测试位置和文件大小守卫。 |
| `tests/unit_src/` | 从生产源码外置的单元测试。 |
| `tests/*.rs` | 路由、worker、migration 与多存储集成测试。 |
| `.github/workflows/docker-image.yml` | 当前唯一 CI，仅构建/发布镜像。 |
| `docker-compose*.yml` | 迁移、默认管理员及生产依赖装配样例。 |
| `src/api/` | **Not found**；API composition 位于 `src/lib.rs` 与各 context routes。 |

### 14. External references / versions

- 审计未访问互联网；外部行为仅用锁定依赖源码交叉核验。
- `Cargo.toml:1-53`：Rust edition 2024；Axum 0.7、SQLx 0.8、Tokio 1、Lapin 2、MongoDB 3、Redis 0.27、Reqwest 0.12。
- `Cargo.lock` 锁定：Axum 0.7.9、SQLx 0.8.6、Tokio 1.52.3、Lapin 2.5.5、MongoDB 3.7.0、Reqwest 0.12.28、tower-http 0.6.11、utoipa 5.5.0；同时存在 Redis 0.27.6 与传递依赖 1.2.3。
- Lapin 2.5.5 本地源码用于核验 `BasicPublishOptions` 和 `Confirmation::NotRequested`，未引用在线文档。

### 15. Related specs

- `.trellis/spec/backend/directory-structure.md:56-98`：层职责、依赖方向、facade 与文件上限。
- `.trellis/spec/backend/database-guidelines.md`：事务、SQL 与迁移边界。
- `.trellis/spec/backend/error-handling.md`：全局错误处理入口。
- `.trellis/spec/backend/quality-guidelines.md`：后端质量与测试要求。
- `.trellis/spec/backend/logging-guidelines.md`：结构化日志约定。
- `.trellis/spec/backend/realtime-websockets.md`：进程内广播、post-commit 与非持久语义。
- `.trellis/spec/backend/wallet-amount-precision.md:7-22`：钱包精度与 ledger 快照不变量。
- `.trellis/spec/guides/cross-layer-thinking-guide.md`、`.trellis/spec/guides/code-reuse-thinking-guide.md`：跨层数据流与复用判断。
- `.trellis/tasks/08-24-project-architecture-business-flow-audit/prd.md`：本次全项目审计范围。

## Caveats / Not Found

- 本报告是静态审计，没有连接生产 MySQL、Redis、MongoDB、RabbitMQ，也没有观察真实队列拓扑、积压、数据量或部署 Secret；P0-02 的运行影响取决于外部是否另行预建并绑定队列，但仓库自身未提供该保证。
- 未执行 `cargo test/check/clippy`：本子任务为只读研究，Cargo 会写入构建目录；结论基于源码与测试定义，不宣称当前测试实际通过。
- 函数长度/await 数来自保持行号的文本级 Rust 边界扫描，是热点指标而非复杂度证明；宏展开和语义复杂度未计算。
- 钱包扫描证明直接 SQL 所有权分散，不等同于已证明当前存在余额错误；需要数据库并发与精度 contract tests 验证具体缺陷。
- 未找到 `src/api/`、仓库内 RabbitMQ queue declaration/binding、publisher confirm、email outbox、独立 backend quality workflow。
