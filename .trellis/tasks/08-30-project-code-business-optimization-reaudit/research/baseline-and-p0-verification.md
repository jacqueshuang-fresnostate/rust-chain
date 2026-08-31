# 当前基线与历史 P0 修复复核

## 1. 复核口径

- 基线：`main@fac1def`，对比原审计 `docs/architecture/project-optimization-audit-2026-08-24.md` 与修复提交 `5aa98f1`。
- “已有实现”只表示当前代码、migration 和自动测试中存在对应闸门；不等同于 GitHub Actions 实际运行了依赖 MySQL/Redis/Mongo/RabbitMQ 的分支，也不等同于生产 migration、历史数据和外部基础设施已经核验。
- `5aa98f1` 之后的生产功能改动主要是杠杆部分平仓 `53d10b2` 和 K 线恢复 Clippy 修复 `b81c222`；其余相关变更主要是任务归档、文档与 Mobile 导航。P0 修复没有被整体回滚，但杠杆域需要在本轮继续检查新增部分平仓是否保持原风险闸门。

## 2. 当前仓库基线

| 项目 | 当前事实 |
| --- | --- |
| 当前提交 | `fac1def`，`origin/main` 同步 |
| migration | 114 个文件，最新业务 migration 为 `0117_margin_partial_close.sql` |
| GitHub 发布门禁 | `quality-gate` 已在镜像 build/publish 前执行 `scripts/p0-release-gate.sh` |
| Rust 门禁 | fmt、全目标 Clippy `-D warnings`、`cargo test --all-targets -- --test-threads=1` |
| admin 门禁 | lint、typecheck、Vitest |
| PC 门禁 | type-check + 仅 `test:margin` |
| Mobile 门禁 | type-check + Node 全量合同测试；未执行 PWA/Tauri build |
| 集成环境 | workflow 未声明 MySQL、Redis、MongoDB、RabbitMQ services，也未设置对应 URL |
| Trellis | 124 个未归档任务：67 `in_progress`、3 `planning`、31 `done`、23 `completed`；活动状态中 49 个至少 30 天、46 个至少 60 天 |

### 2.1 门禁的关键限制

`scripts/p0-release-gate.sh` 的覆盖面比旧审计显著提升，因此历史 P1-14 已由“完全未调用质量能力”变为“部分完成”。但 `.github/workflows/docker-image.yml::quality-gate` 没有 service containers；多组资金与迁移测试在缺少 `DATABASE_URL`、`REDIS_URL`、`MONGODB_URI` 时直接打印 skipping 并 `return Ok(())`，例如：

- `tests/bootstrap_admin.rs::bootstrap_creates_once_skips_existing_admins_and_reuses_roles`
- `tests/wallet_chain_worker.rs`、`tests/wallet_routes.rs` 的 MySQL 集成 setup
- `tests/new_coin_routes.rs`、`tests/loan_risk.rs`、`tests/seconds_contract_routes.rs`
- `tests/withdrawal_quote_migration.rs::fresh_migration_chain_supports_withdrawal_quotes`
- `tests/market_ingestion.rs`、`tests/kline_recovery.rs`

因此当前发布门禁可证明编译、静态 lint、无外部依赖单测和前端合同测试通过，但不能证明 P0 的数据库事务、migration、Redis 新鲜度、Mongo K 线或 RabbitMQ 交付分支在 CI 中真实执行。后续应把“缺少依赖时跳过”改成显式区分 local optional 与 CI required lane，并校验 required test executed count。

## 3. 历史 P0 状态映射

| 历史 ID | 当前状态 | 当前证据与剩余验证 |
| --- | --- | --- |
| P0-01 默认管理员 | **代码已实现，CI/生产验证部分完成** | `src/bootstrap.rs::BootstrapAdminConfig::from_env` 拒绝空值和已知默认值；`src/bin/exchange-migrate.rs` 使用显式模式；Compose 默认口令为空；`tests/bootstrap_admin.rs` 覆盖空值/默认值/幂等/文件 Secret。MySQL 集成分支在当前 workflow 会因无 `DATABASE_URL` 跳过；生产首次启动、一次性改密和 Secret 注入仍需部署证据。 |
| P0-02 提现广播歧义 | **代码已实现，CI/生产验证部分完成** | `src/workers/wallet_chain.rs::run_once_with_gateway` 与 unknown/manual-review 分支保留冻结并按稳定 `gateway_request_id` 查询；`tests/wallet_chain_worker.rs::ambiguous_broadcast_keeps_funds_frozen_and_reconciles_by_stable_request_id` 等覆盖歧义/矛盾回执。需要 CI MySQL 真跑、真实网关查询合同和生产 unknown 队列对账。 |
| P0-03 新币权威定价/供给 | **代码已实现，CI/生产验证部分完成** | migration `0111_new_coin_authoritative_issuance.sql`；`src/modules/new_coin/{application,infrastructure,service}.rs` 使用服务端规则、供给预留与请求指纹；`tests/new_coin_routes.rs` 有并发/篡改/幂等回归。当前 CI 无 MySQL，需验证历史项目供给回填与生产并发。 |
| P0-04 新币解禁费真实动账 | **代码已实现，CI/生产验证部分完成** | `src/modules/new_coin/infrastructure/unlock.rs`、platform journal migration `0110_platform_financial_journal.sql` 与 unlock scanner/route 测试覆盖钱包、流水、状态同事务。当前 CI 的相关 MySQL 路径可跳过；需生产费用账户与历史 paid 记录对账。 |
| P0-05 借贷 LTV/估值/清算 | **代码已实现，CI/生产验证部分完成** | `0112_loan_collateral_risk.sql`、`0113_loan_liquidation_accounting.sql`；`src/modules/loan/oracle.rs`、`liquidation.rs`、`src/workers/loan_health.rs`；`tests/loan_risk.rs` 覆盖过期价、阈值和幂等。CI 缺 MySQL/Redis 会跳过关键集成分支；还需生产 oracle SLO、坏账和清算数据核验。 |
| P0-06 秒合约事件时点价 | **代码已实现，CI/生产验证部分完成** | `0114_event_time_price_snapshots.sql`；`src/modules/seconds_contract/application.rs::settle_order` 按 `expires_at` 选不可变 snapshot，窗口内无价保持待结算；worker/route 测试存在。CI 无 MySQL；需生产历史价格完整性与延迟结算对账。 |
| P0-07 预测本地关盘/新鲜度 | **代码已实现，CI/生产验证部分完成** | `0115_prediction_market_local_close.sql`；`src/modules/prediction/service.rs` 在 `now >= end_at`、无同步或陈旧时 fail closed；`src/workers/prediction_market_close.rs`；`tests/prediction_commission_routes.rs::prediction_end_at_boundary_closes_locally_and_rejects_order_without_wallet_change`。CI 无 MySQL，需生产同步延迟和关盘积压指标。 |
| P0-08 闪兑权威报价 | **代码已实现，CI/生产验证部分完成** | `0116_convert_quote_authority.sql`；`src/modules/convert/{application,infrastructure,service}.rs` 保存权威报价、价格观测时间、过期边界并在确认事务锁定/单次消费；`tests/convert_routes.rs` 有陈旧/过期/幂等路径。CI 无 MySQL/Redis，需生产 quote freshness 和对账。 |
| P0-09 全仓转出风险闸门 | **代码已实现，新增路径需继续回归** | `src/modules/margin/application/account_settings.rs`、`infrastructure/cross_accounts.rs` 在锁定账户/持仓/钱包后计算转后维持保证金；`tests/margin_routes.rs` 有并发和阈值测试。后续 `53d10b2` 新增部分平仓/累计盈亏，未删除转出闸门，但需在本轮域审查确认部分平仓、强平、转出共享同一风险事实；CI 无 MySQL。 |
| P0-10 行情旧代际写入 | **代码与无外部依赖测试已实现** | `src/workers/market_feed.rs::{MarketFeedGenerationFence,MarketFeedSupervisor::shutdown_active_generation}` 对 cancel/join 与全部外部副作用持 generation permit；worker 单元测试覆盖旧代际。仍需真实 provider、连续 reload、多实例配置 ACK 和运行指标补证。 |
| P0-11 PC 杠杆意图漂移 | **代码与 CI 定向测试已实现** | `pc/src/domain/marginActions.ts`、`pc/src/api/contract.ts`、`pc/src/stores/contract.ts` 收敛真实能力和部分失败；`pc/tests/contract-margin-actions.test.ts` 由 CI `test:margin` 执行。PC 其余 API/store 行为没有进入通用测试，P1-07/15/17 仍需继续。 |
| P0-12 Mobile 提现费用披露 | **代码与前端 CI 已实现，后端集成验证部分完成** | `src/modules/wallet` 提供权威 withdrawal quote，`mobile/src/core/withdrawalQuote.ts` 与 `WithdrawView.vue` 展示 `amount/fee/net/total_reserved` 并绑定 quote；`mobile/tests/withdrawal-quote-contract.test.ts` 在 CI 执行。后端 quote/migration MySQL 分支会跳过，生产 fee tier/config version 仍需对账。 |

## 4. 初步结论

1. 历史 12 项 P0 不应重新列为“未修复”的当前 P0；当前代码层均有对应实现和测试。
2. 它们共同剩余的系统性风险应归并到一个当前 P1：**CI required integration lane 与生产 reconciliation 未闭环**，不能把 12 项分别重复计数。
3. P0-09 所在杠杆域在 P0 修复后又新增部分平仓，属于必须重点做交叉回归的变更面，但当前静态证据尚不足以把它重新升级为 P0。
4. 旧 P1-14 已部分完成；旧 P1-21 没有收敛，当前 67 个 `in_progress`、46 个至少 60 天，仍是高优先级工程治理缺口。
