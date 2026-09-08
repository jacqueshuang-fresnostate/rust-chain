# 生产迁移退出码审计

## 观察

从用户提供的 Compose 事件只能确认 `migrate` 进程返回 1，不能区分连接、迁移 SQL、dirty/checksum、权限或 bootstrap 配置。`Started` 到 `Error` 约为一秒，优先级最高的是首次连接失败或引导配置校验；大表 DDL 锁等待通常会留下更长的运行时间，但仍需保留迁移错误诊断。

## 代码证据

- `src/bin/exchange-migrate.rs` 当前只建立一次 MySQL 连接；成功后直接运行静态 `sqlx::migrate!`。
- 1Panel 示例只连接外部网络和外部数据库，`migrate` 没有数据库 `healthcheck`/`depends_on`，因此外部服务重启时存在 ready race。
- `src/bootstrap.rs` 的事务本身会先检查 `admin_users`，但二进制在进入事务前调用 `BootstrapAdminConfig::from_env()`；旧的 `create_admin` + 空密码会在已有管理员环境也失败。
- SQLx 保留 `success=FALSE` 的 dirty 记录；DDL 可能隐式提交，自动忽略或手动标成功会掩盖真实状态。

## 风险排序

1. 外部 MySQL 未就绪、主机名/网络/凭据或 URL 编码错误。
2. 陈旧 `BOOTSTRAP_MODE=create_admin` 与空/旧口令（已有管理员时属于可避免的启动阻断）。
3. 历史 dirty/checksum 或权限问题。
4. 0124 `DROP CHECK` 与生产 MySQL/MariaDB 版本、约束名或既有非法 source 数据不兼容。

## 结论

先改善迁移器的有界建连重试和非敏感诊断，再修正已有管理员的 bootstrap 短路；不在没有线上 stderr 的前提下改写不可变 migration。部署 runbook 必须继续要求采集容器日志、服务器版本和 `_sqlx_migrations` 状态，并由运维决定 dirty 记录恢复。

## 实施与复核结果

- 初始连接仅重试明确的临时网络/DNS/SQLSTATE/客户端连接错误；URL、认证、TLS、权限和 SQL 错误快速失败。次数 `1–30`、退避 `0–30` 秒之外还有不可由环境延长的 300 秒硬总时限。
- `MigrateError` 的原始链只经过单行脱敏后进入 tracing 和最终非零错误，避免 CLI 终止路径再次打印未脱敏 source；诊断查询按三秒独立超时，失败不会覆盖原 migration 错误。
- 已有管理员时，`create_admin` 在命名锁和事务内完成存在性检查后直接跳过，不读取陈旧用户名/角色/口令；空表仍严格校验并保持角色与管理员同事务写入。
- 真实 MySQL 9.3 上完整 0001–0125 迁移成功，手工制造 version 125 dirty 后进程退出 1，并输出 `server_version`、122 条记录、最新版本 125 和 `dirty_versions=[125]`；bootstrap 陈旧环境回归 3/3 通过。Compose 展开验证了重试变量只存在于 migrate，API 的 `service_completed_successfully` 门禁保留。
- 当前证据仍不指向某一个线上 migration；下一次目标部署必须先读取 `执行 SQLx migrations 失败` 与 `迁移失败诊断摘要`，再决定是网络配置、dirty/checksum 还是数据库方言/约束问题。
