# 2026-08-24 历史 P1/P2 当前状态映射

## 1. 口径

- 基线为 `main@fac1def`；状态只代表当前静态源码、migration、测试与交付配置能够证明的程度。
- “部分完成”表示旧根因已有实质修复，但仍有可达旁路、未覆盖客户端、未进入 required CI，或缺少运行时/生产核验；不能当作关闭。
- 详细源码证据见本任务其他 research 文件。表内只保留用于报告去重的当前结论，最终优先级由独立检查代理按严格 P0/P1/P2 口径复核。

## 2. 历史 P1（21 项）

| 历史 ID | 当前状态 | 2026-08-30 当前结论与证据入口 |
| --- | --- | --- |
| P1-01 `user.created` MQ 交付 | **仍存在** | publisher 未启用 confirm，consumer 依赖外部预声明 queue/binding，标准 Compose 未强制 consumer；见 `delivery-security-quality.md::DSQ-02`、`backend-core-finance.md::F-04`。 |
| P1-02 会话撤销伪成功 | **部分完成** | `revoke_actor_auth_sessions` 已只把 `SessionNotFound` 当空集合，其余错误上抛；Admin 具备 `auth_session_version`。但 User/Agent access extractor 不回查活跃状态/凭据代际，改密或停用仍依赖提交后 Redis/Sa-Token 清理；见 `backend-core-finance.md::F-02`、`delivery-security-quality.md::DSQ-10`。 |
| P1-03 5xx 泄露底层原文 | **仍存在** | `src/error.rs::IntoResponse` 仍把 `self.to_string()` 写入响应，且测试固化内部错误文案；见 `backend-core-finance.md::F-10`。 |
| P1-04 提现网络与限频 | **部分完成** | quote/提交已锁定并复核 active asset-network 与配置版本；但 `create_withdrawal_request` 仍向风控传 `None`，Redis 跨实例限频不执行，PC 还未接必填 quote 合同；见 `backend-core-finance.md::F-06`、`admin-pc-cross-layer.md::APC-P1-01`。 |
| P1-05 充值费与资产精度 | **仍存在，局部改善** | 提现、闪兑 quote、新币和部分借贷路径已量化；充值仍全额入账/冲正且没有 gross-fee-net 快照，现货、杠杆、理财、人工充值等仍未统一执行 `assets.precision_scale`；见 `backend-core-finance.md::F-05/F-07`、`backend-product-business.md::BPB-P0-02`。 |
| P1-06 现货语义与批量契约 | **部分完成** | 后端已有批量撤单并返回部分失败；系统流动性模式仍缺明确产品 ADR，部分客户端仍以逐笔或旧 read model 包装服务端合同；待 mobile research 与最终报告合并去重。 |
| P1-07 杠杆计息、坏账与 PC 风险 | **仍存在，局部改善** | 全仓转出风险和全仓坏账已补齐，后端已支持部分平仓；计息仍依赖调度分片/当前利率，逐仓负 equity 未记坏账，PC 仍本地拼装风险而不消费权威快照；见 `backend-core-finance.md::F-09`、`admin-pc-cross-layer.md::APC-P0-01`。 |
| P1-08 返佣退款反冲与持久重试 | **仍存在** | prediction invalid refund 不反冲 commission，结算不复核 source 终态；失败 guard 仍是进程内 `HashSet`；见 `backend-product-business.md::BPB-P1-02/BPB-P1-05`。 |
| P1-09 平台双重记账 | **部分完成** | migration `0110_platform_financial_journal.sql` 已覆盖新币解禁费与借贷部分流程；充值、提现、现货/杠杆、闪兑、理财、秒合约、预测、返佣等仍未形成统一平衡科目；见 `backend-core-finance.md::F-08`、`backend-product-business.md::BPB-P1-01`。 |
| P1-10 财务实时跨实例收敛 | **仍存在，mobile 局部较强** | 服务端 hub 仍是进程内 bounded broadcast；PC/Admin 缺半开 watchdog、可信 freshness 与统一 REST 版本对账，Admin 行级连接还会 fan-out；见 `admin-pc-cross-layer.md::APC-P1-05`，待 data/mobile research 补齐。 |
| P1-11 worker 角色/监督/公平重试 | **仍存在** | `src/main.rs` 仍 fire-and-forget 启动全部 worker，API 扩容会重复启动；多类资金 worker 缺持久 retry/dead-letter/fair claim；见 `delivery-security-quality.md::DSQ-03`、`backend-product-business.md::BPB-P1-05/BPB-P1-07`。 |
| P1-12 行情时间信任与多实例配置 | **部分完成** | P0 generation fence 已阻止旧代际继续写；future skew、source/received time、合成 tick 历史归档、多实例配置 ACK/readiness 仍未闭环；见 `baseline-and-p0-verification.md::P0-10`、`backend-product-business.md::BPB-P0-01`，待 data research 补齐。 |
| P1-13 migration/状态约束 | **部分完成** | migration 数量已到 0117，并有 fresh/re-run 测试；CI 无 MySQL，测试可 skip，也没有生产快照 upgrade、旧应用/new schema smoke 与通用 expand-contract gate；见 `delivery-security-quality.md::DSQ-07`。 |
| P1-14 CI/发布质量门禁 | **部分完成** | 已有 required `quality-gate` 且位于镜像发布前；但外部依赖测试静默跳过、PC/Mobile build 缺失、供应链扫描/签名/attestation 缺失；见 `baseline-and-p0-verification.md::2.1`、`delivery-security-quality.md::DSQ-04/05`。 |
| P1-15 共享资金契约与行为测试 | **仍存在，局部改善** | wallet OpenAPI 已增加 quote，但核心 spot/margin/seconds/earn/loan/prediction 仍手工同步；PC/Mobile 大量测试是源码文本合同，已发生 quote/settlement 字段漂移；见 `admin-pc-cross-layer.md::APC-P1-06`。 |
| P1-16 分层与巨型文件 | **仍存在** | 钱包 posting 仍由多上下文直接 SQL，各端仍有巨型 adapter/view/CSS；Rust 架构守卫只覆盖部分目录，未阻止客户端 mega-file 与跨上下文资金规则复制；见 `backend-core-finance.md::F-07/F-08`，待 client research 补齐。 |
| P1-17 Admin/PC 运行边界 | **部分完成** | PC transport 已有 timeout/refresh single-flight，Admin DataTable 能区分 error/empty；但 PC session 双事实、登录 redirect、Admin ANY 动作权限、生产 origin/Tauri updater 与弱 DTO 仍未闭环；见 `admin-pc-cross-layer.md::APC-P1-03/04/09`。 |
| P1-18 readiness/指标/告警 | **仍存在** | `/health` 恒定成功并被容器探针复用，worker 无 required heartbeat；只有日志，没有可抓取业务 backlog/dead-letter/price-age 指标与规则；见 `delivery-security-quality.md::DSQ-03`。 |
| P1-19 Docker/配置/Secret/HA | **仍存在，局部改善** | 一体化镜像已非 root、迁移器独立，Compose 示例不再给默认管理员口令；但可变镜像/Action、typed settings 与 worker 直读 env 并存、Secret 无 key version、资源/能力/HA 仍缺；见 `delivery-security-quality.md::DSQ-05/06`。 |
| P1-20 多存储备份恢复 | **仍存在** | 仓库仍无跨 MySQL/Mongo/Rabbit/Redis/uploads/Secret 的 PITR/restore harness 与定期实演；见 `delivery-security-quality.md::DSQ-08`。 |
| P1-21 Trellis 状态失真 | **仍存在且未收敛** | 124 个未归档任务中 67 个 `in_progress`，并同时存在 `done/completed` 两套完成语义；历史 archive 元数据与 progress/提交记录不一致；见 `baseline-and-p0-verification.md::2`、`delivery-security-quality.md::DSQ-11`。 |

## 3. 历史 P2（8 项）

| 历史 ID | 当前状态 | 当前结论 |
| --- | --- | --- |
| P2-01 CORS 生产来源 | **仍存在** | `src/lib.rs::build_router` 仍使用 `CorsLayer::permissive()`；网关限制需要运行时补证。 |
| P2-02 邮件验证码交付状态 | **仍存在** | 多条验证码流程先落冷却/验证码再同步 SMTP，失败没有 durable delivery state/outbox；见 `delivery-security-quality.md::DSQ-09`。 |
| P2-03 前端包与样式边界 | **仍存在** | Admin shared resource config、PC adapter、Mobile giant views/CSS 仍显著超出可维护边界；待 client research 给出当前行数与拆分建议。 |
| P2-04 PC i18n | **仍存在/待量化** | `pc/src/i18n/index.ts` 仍为大单文件并存在页面硬编码；最终报告只在当前路径证据确认后保留。 |
| P2-05 重复小工具 | **仍存在/待量化** | normalize/pagination/subject/Decimal 展示等仍有跨域复制，但最终报告应与 P1-16 合并，避免只为“重复”单独计数。 |
| P2-06 Codegraph 索引污染 | **部分完成** | `.gitignore` 已忽略 `.codegraph/`；本地 codegraph 查询仍命中 node_modules，需清理并重建，属于本地工具治理而非生产风险。 |
| P2-07 Admin mega-chunk | **部分完成** | 路由已 lazy load；`resourceConfigs.tsx` 与全部 actions 仍是共享大 chunk，需以 bundle budget/实测验证再拆。 |
| P2-08 文档与错误/日志 spec | **仍存在** | `logging-guidelines.md` 仍是模板，error/database spec 核心章节未完全可执行；见 `delivery-security-quality.md::DSQ-11`。 |

## 4. 去重与报告编排建议

1. P1-01、P1-10、P1-11、P1-18、P1-19 可归并为“异步/worker/部署可靠性”计划，但保留不同验收门禁。
2. P1-05、P1-09、理财精度和新发现的高价值命令幂等应归入同一“统一 WalletPosting + Decimal + Journal + Reconciliation”路线，严重级别按当前直接可达影响分别判定。
3. P1-14、P1-15、P1-13 应形成一条发布门禁依赖链：生成契约 → 真实集成/行为测试 → migration/兼容矩阵 → 镜像签名与部署证据。
4. P1-16、P2-03、P2-04、P2-05、P2-07 不应重复计数为多个高优先级根因；最终报告可用结构热点表承载。
5. 历史 12 项 P0 的当前状态见 `baseline-and-p0-verification.md`，不要把共同的“CI/生产未补证”复制成 12 条新 P1。
