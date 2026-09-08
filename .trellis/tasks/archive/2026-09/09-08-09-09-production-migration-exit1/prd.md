# 修复生产迁移服务退出码 1

## Goal

让 1Panel/外部依赖部署中的 `exchange-migrate` 在 MySQL 短暂未就绪时具备有限重试能力，并在真正的迁移或引导失败时输出可执行的诊断信息，避免只看到 `service "migrate" didn't complete successfully: exit 1`。同时消除已有管理员与陈旧引导环境变量之间的不必要耦合，保证已经初始化过的数据库不会因为未使用的引导口令失效。

## What I already know

- 当前 `src/bin/exchange-migrate.rs` 只调用一次 `MySqlPoolOptions::connect`，1Panel 外部 MySQL 尚未 ready、网络别名错误或连接被重启时会在一秒内退出。
- 迁移失败后当前只带一个通用 `执行 SQLx migrations 失败` 上下文，没有打印数据库版本、最近迁移记录或 dirty 记录，1Panel 日志无法判断是连接、SQL、权限还是 checksum 问题。
- `docker-compose.1panel.example.yml` 依赖外部数据库，不定义 MySQL healthcheck；迁移完成门禁必须保留，不能让 API 在迁移失败时启动。
- 当前引导流程在 `exchange-migrate` 中先解析 `BootstrapAdminConfig`，再由事务检查是否已有管理员。旧环境残留 `BOOTSTRAP_MODE=create_admin` 但口令为空/失效时，即使数据库已经有管理员也会退出 1。
- SQLx migration 必须保持不可变；不能自动删除 dirty 记录、伪造 `success=1`、跳过失败 migration，也不能在没有真实错误日志时改写 0124/0125。

## Requirements

1. 为迁移器增加有界 MySQL 初始连接重试，默认等待外部数据库短暂启动；连接串格式错误、认证失败等非暂时错误不做无意义的无限重试。
2. 重试次数和间隔可由迁移专用环境变量覆盖，范围受限并在日志中显示实际尝试次数；不记录 `DATABASE_URL` 或任何口令。
3. 迁移 SQL 失败时记录完整错误链，并尽力输出 MySQL 版本、`_sqlx_migrations` 是否存在、最近版本及 `success=FALSE` 记录；诊断查询失败不能掩盖原始迁移错误。
4. 成功日志显示迁移器内置的最新版本/数量；仍以退出码 0 表示成功完成，API 继续只依赖 `service_completed_successfully`。
5. `BOOTSTRAP_MODE=create_admin` 时，先在现有命名锁/事务内检查管理员；已有任意管理员直接跳过且不读取或校验未使用的口令/用户名/角色。管理员表为空时保持现有严格校验和一次性创建语义。
6. 更新 1Panel 示例、部署文档和自动化合同测试，说明重试变量、日志采集及 dirty/checksum 的人工恢复边界。

## Acceptance Criteria

- [x] 外部 MySQL 首次连接暂时失败时，迁移器按默认有界策略重试并在恢复后继续迁移；达到上限后以非零码退出且不启动 API。
- [x] 重试次数/间隔均受 `1–30`/`0–30` 范围和固定 300 秒总时限约束，手工构造配置也不能绕过边界。
- [x] 非暂时连接错误不会被吞掉，日志不包含连接串口令，并给出明确的配置/权限错误。
- [x] 任一 migration 失败时，日志包含原始 migration 错误以及可用的服务器版本/迁移状态；没有 `_sqlx_migrations` 时仍保留原错误。
- [x] 已有管理员 + 陈旧/空引导密码的重复运行返回 0、创建 0 个角色且不改现有账号；空管理员库 + 陈旧密码仍返回非零码。
- [x] 1Panel Compose 仍只有 `migrate`/`api`，保留迁移成功门禁；新增变量只注入迁移服务，不传给 API。
- [x] Rust 格式、架构/单元测试、迁移/引导相关集成测试和 Compose 合同检查通过；`docs/superpowers/PROGRESS.md` 已记录验证结果。

## Definition of Done

- 生产迁移入口、引导入口、示例配置与部署 runbook 完成。
- 新行为有独立测试，既有 migration 不被修改，未操作线上数据库/订单/资金。
- 运行最贴近改动的 Rust/Compose 检查并记录任何真实服务限制。
- 经过 `trellis-check` 质量复核后再进入提交/推送交接。

## Technical Approach

- 新增独立的迁移运行辅助模块，集中解析受限重试配置、识别可重试连接错误、建立单连接池，以及在 migration 失败后读取非敏感诊断摘要。
- 二进制入口只负责日志初始化、环境读取、调用辅助模块、运行静态 SQLx migrator 和引导；不自动修复 schema 漂移。
- 在 bootstrap 模块中复用现有命名锁和事务，把“检查已有管理员”置于配置解析之前；创建路径仍调用同一套 `BootstrapAdminConfig` 校验和 Argon2 写入逻辑。
- 1Panel 外部依赖仍由运维保证网络/账号/版本；重试只是启动竞态的兜底，不替代 `docker logs`、`_sqlx_migrations` 和备份恢复 runbook。迁移建连的次数/退避和单次五秒超时共同受固定 300 秒总时限保护。

## Decision (ADR-lite)

**Context**：现有部署日志只有 Compose 包装层的退出码，且外部 MySQL 没有健康检查；陈旧 bootstrap 环境也可能在毫秒级阻断已初始化数据库。

**Decision**：采用“有限连接重试 + 非敏感失败诊断 + 已有管理员先跳过”的最小兼容修复。对 dirty migration、checksum、方言不兼容和数据约束冲突只提供证据与人工 runbook，不做自动删记录或隐式 DDL。

**Consequences**：短暂的数据库启动竞态可自动恢复；错误配置最多等待受限时长后会明确失败。日志会增加版本和迁移状态摘要但不泄露 Secret。真正的 schema 漂移仍需停写、备份和人工确认。

## Out of Scope

- 不编辑已发布的 0124/0125 或其他 migration，不改变交易、钱包、订单和结算逻辑。
- 不自动删除/修改 `_sqlx_migrations`、不自动重建约束、不兼容性降级到 MariaDB。
- 不替 1Panel 创建 MySQL/Mongo/Redis/RabbitMQ，不修改线上环境变量、容器、历史订单或资金。
- 不把 bootstrap 凭据移入 API，也不放宽空管理员库的口令安全校验。

## Technical Notes

- 生产入口：`src/bin/exchange-migrate.rs`、`src/bootstrap.rs`。
- 部署合同：`.trellis/spec/backend/container-delivery.md`、`docs/deployment/docker.md`、`docker-compose.1panel.example.yml`、`docker-compose.1panel.env.example`。
- 相关测试：`tests/bootstrap_admin.rs`、`tests/docker_image_contract.rs`、`tests/unit_src/src_bootstrap_tests.rs`。
- 线上日志本身未包含容器 stderr；实现后仍建议优先采集 `docker logs --tail=200 hippo-exchange-migrate` 以定位具体 SQL/权限问题。
