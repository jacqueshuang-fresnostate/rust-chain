# 项目进度记录

本文件记录每次完成的任务切片。后续会话必须先读取本文件，再继续执行任务。

## 2026-05-31 21:57 - Admin 静态说明文案清理

- 完成内容：移除 Admin UI 中通过 Semi Typography/PageHeader 渲染的静态辅助说明文案，包括资源页说明、产品配置/行情订阅/新币/闪兑/行情策略/代理管理页面说明、创建/修改弹窗辅助说明；保留真实数据展示、字段标签、按钮、错误提示、安全警示、操作原因提示和运行状态摘要。
- 修改文件：
  - `web/src/layouts/PageHeader.tsx`
  - `web/src/admin/resources/AdminResourcePage.tsx`
  - `web/src/admin/resources/ResourceCreateActions.tsx`
  - `web/src/admin/actions/ProductStatusActions.tsx`
  - `web/src/admin/actions/MarketFeedConfigPage.tsx`
  - `web/src/admin/actions/ConvertRuleActions.tsx`
  - `web/src/admin/actions/NewCoinActions.tsx`
  - `web/src/admin/actions/MarketStrategyActions.tsx`
  - `web/src/admin/actions/AgentManagementPage.tsx`
  - `web/src/admin/dashboard/DashboardPage.tsx`
  - `web/src/admin/resources/AdminResourcePage.test.tsx`
  - `web/src/admin/resources/resourceConfigs.test.tsx`
  - `web/src/admin/actions/ProductStatusActions.test.tsx`
  - `web/src/admin/actions/MarketFeedConfigPage.test.tsx`
  - `web/src/admin/actions/helperCopy.test.tsx`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已按 TDD 先执行前端 targeted 测试确认新增静态文案断言失败；实现后已重新执行 `npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" test -- AdminResourcePage.test.tsx resourceConfigs.test.tsx ProductStatusActions.test.tsx MarketFeedConfigPage.test.tsx helperCopy.test.tsx`，5 个测试文件、39 个测试通过、0 失败；已执行 `npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" run typecheck`，通过；已执行 `npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" run lint`，通过；已执行 `git diff --check`，通过。
- 后续事项：如需继续去除 Banner 安全警示、空状态/错误描述或确认弹窗操作提示，需要单独确认范围，避免误删功能性信息。

## 2026-05-31 21:18 - Admin 交易对配置页补齐

- 完成内容：补齐 `/admin/market/pairs` 后台交易对配置页，交易对、状态、市场类型筛选改为下拉选择且提交后端枚举/交易对值；隐藏该页默认“查看JSON”；新增行级“查看详情”和“修改”，修改仅提交价格精度、数量精度、最小下单额、市场类型和 reason；后端新增 `PATCH /admin/api/v1/market-pairs/:id` 安全配置更新接口并写入 `trading_pair.config.update` 审计；表格市场类型显示中文标签；补充筛选器样式。
- 修改文件：
  - `src/modules/admin/routes.rs`
  - `tests/admin_routes.rs`
  - `web/src/shared/FilterBar.tsx`
  - `web/src/admin/resources/AdminResourcePage.tsx`
  - `web/src/admin/resources/ResourceCreateActions.tsx`
  - `web/src/admin/resources/resourceConfigs.tsx`
  - `web/src/admin/resources/AdminResourcePage.test.tsx`
  - `web/src/admin/resources/resourceConfigs.test.tsx`
  - `web/src/styles.css`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 `npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" test -- AdminResourcePage.test.tsx resourceConfigs.test.tsx`，2 个测试文件、27 个测试通过、0 失败；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_trading_pair -- --nocapture`，4 个测试通过、0 失败，MySQL-gated 分支因本地未设置 `DATABASE_URL` 按设计跳过；首次执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check` 发现 Rust 格式需调整，已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml"` 后重新执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" run typecheck`，通过；已执行 `npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" run lint`，通过；已执行 `git diff --check`，通过。
- 后续事项：如交易对数量超过当前列表加载上限，后续补专用 symbol options endpoint；交易对身份字段变更需独立工作流，不纳入本切片。

## 2026-05-31 20:02 - Admin Earn 与闪兑行级操作补齐

- 完成内容：新增 Admin Earn 产品详情与申购详情接口，Earn 产品创建/启停强制非空 reason 并保留审计；新增 Admin 闪兑交易对详情与闪兑订单详情接口，闪兑交易对创建/启停强制非空且不超过 512 字符的 reason 并保留审计；前端为 Earn 产品、Earn 申购、闪兑交易对、闪兑订单接入行级“查看详情”，并仅为 Earn 产品和闪兑交易对提供带原因确认的安全启停操作，未开放 Earn 申购或闪兑订单任意状态修改。
- 修改文件：
  - `src/modules/earn/routes.rs`
  - `src/modules/admin/routes.rs`
  - `tests/earn_routes.rs`
  - `tests/admin_routes.rs`
  - `web/src/admin/resources/ResourceCreateActions.tsx`
  - `web/src/admin/resources/resourceConfigs.tsx`
  - `web/src/admin/resources/resourceConfigs.test.tsx`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test earn_routes admin_earn -- --nocapture`，6 个测试通过、0 失败，MySQL-gated 分支因本地未设置 `DATABASE_URL` 按设计跳过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_convert -- --nocapture`，9 个测试通过、0 失败，MySQL-gated 分支因本地未设置 `DATABASE_URL` 按设计跳过；已执行 `npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" test -- resourceConfigs.test.tsx AdminResourcePage.test.tsx`，2 个测试文件、21 个测试通过、0 失败；已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" run typecheck`，通过；已执行 `npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" run lint`，通过；已执行 `git diff --check`，通过。
- 后续事项：Earn 产品强类型详情页、Earn 申购收益/赎回链路联动展示、闪兑订单成交明细页、其他后台资源筛选与行级上下文操作继续补齐。

## 2026-05-31 18:28 - Admin 杠杆与秒合约 CRUD 安全闭环

- 完成内容：新增 Admin 杠杆产品详情、杠杆仓位详情、强平记录详情、秒合约产品详情、秒合约订单详情；杠杆与秒合约产品创建/启停强制非空 reason 并保留审计；秒合约手动结算强制非空 reason，复用原结算事务并仅在新结算成功时写 `seconds_contract_order.settle` 审计；前端为杠杆产品、杠杆仓位、强平记录、秒合约产品、秒合约订单接入行级“查看详情”、安全启停和固定赢/输结算操作。
- 修改文件：
  - `src/modules/margin/routes.rs`
  - `src/modules/seconds_contract/routes.rs`
  - `src/modules/admin/routes.rs`
  - `tests/margin_routes.rs`
  - `tests/seconds_contract_routes.rs`
  - `tests/admin_routes.rs`
  - `web/src/admin/resources/ResourceCreateActions.tsx`
  - `web/src/admin/resources/resourceConfigs.tsx`
  - `web/src/admin/resources/resourceConfigs.test.tsx`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test margin_routes admin_margin -- --nocapture`，8 个测试通过、0 失败，MySQL-gated 分支因本地未设置 `DATABASE_URL` 按设计跳过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test seconds_contract_routes admin_seconds_contract -- --nocapture`，6 个测试通过、0 失败，MySQL-gated 分支因本地未设置 `DATABASE_URL` 按设计跳过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes margin_liquidation -- --nocapture`，2 个测试通过、0 失败，MySQL-gated 分支因本地未设置 `DATABASE_URL` 按设计跳过；已执行 `npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" test -- resourceConfigs.test.tsx AdminResourcePage.test.tsx`，2 个测试文件、17 个测试通过、0 失败；已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" run typecheck`，通过；已执行 `npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" run lint`，通过；已执行 `git diff --check`，通过。
- 后续事项：杠杆仓位强类型详情页、保证金/利息/强平链路联动展示、秒合约订单结算明细页、Earn/闪兑等其他模块行级操作补齐。

## 2026-05-31 14:26 - Admin 运营总览仪表盘

- 完成内容：新增 Admin 运营总览 API `/admin/api/v1/dashboard`，聚合用户、钱包资产、交易对、现货、闪兑、秒合约、杠杆、Earn、风控事件、outbox/inbox 和审计动作状态；重做 Admin 首页为交易所运营看板，展示 KPI、行情订阅、链上托管未接入提示、产品运行、风险积压和最新审计动作，并支持失败提示与手动刷新。
- 修改文件：
  - `src/modules/admin/routes.rs`
  - `tests/admin_routes.rs`
  - `web/src/admin/dashboard/DashboardPage.tsx`
  - `web/src/admin/dashboard/DashboardPage.test.tsx`
  - `web/src/styles.css`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_dashboard -- --nocapture`，2 个测试通过、0 失败，其中 MySQL-gated shape 测试因本地未设置 `DATABASE_URL` 按设计跳过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" test -- DashboardPage.test.tsx`，2 个测试通过、0 失败；已执行 `npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" run typecheck`，通过；已执行 `npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" run lint`，通过；已执行 `git diff --check`，通过。
- 后续事项：继续完善区块链后台的强类型详情页、行级上下文操作、筛选器增强，以及链上充值/提现/冷热钱包/归集/对账等 custody 独立切片。

## 2026-05-31 10:44 - Admin 市场类型中文显示

- 完成内容：将 Admin 添加现货交易对弹窗中的市场类型下拉显示改为中文，`external/internal/strategy` 分别显示为外部行情、内部撮合、策略行情，提交值保持原枚举值不变；补充前端测试覆盖中文选项和值映射。
- 修改文件：
  - `web/src/admin/resources/ResourceCreateActions.tsx`
  - `web/src/admin/resources/resourceConfigs.test.tsx`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 `npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" test -- resourceConfigs.test.tsx`，4 个测试通过、0 失败；已执行 `npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" run typecheck`，通过；已执行 `npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" run lint`，通过。
- 后续事项：无。

## 2026-05-31 10:22 - Admin 交易对创建资产下拉选择

- 完成内容：将 Admin 添加现货交易对、杠杆交易对、秒合约交易对表单中的资产 ID 输入改为资产列表下拉选择；资产选项从 `/admin/api/v1/assets` 读取 active 资产，展示符号、名称和 ID，提交给后端仍保持原 ID 字段；补充前端测试覆盖基础资产、计价资产、保证金资产和押注资产选择。
- 修改文件：
  - `web/src/admin/resources/ResourceCreateActions.tsx`
  - `web/src/admin/resources/resourceConfigs.test.tsx`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 `npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" test -- resourceConfigs.test.tsx`，4 个测试通过、0 失败；已执行 `npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" run typecheck`，通过；已执行 `npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" run lint`，通过。
- 后续事项：无。

## 2026-05-26 - 区块链交易所设计文档拆分与补充

- 完成内容：建立区块链交易所一期 MVP 设计文档，按功能拆分为总览、行情 K 线、新币生命周期、资产现货、后台代理权限、风控测试、闪兑等文档；补充新币上市后认购、解禁规则、解禁矿工费、策略 K 线停机补偿、代理后台边界和闪兑设计。
- 修改文件：
  - `docs/superpowers/specs/2026-05-26-blockchain-exchange-platform-design.md`
  - `docs/superpowers/specs/blockchain-exchange/README.md`
  - `docs/superpowers/specs/blockchain-exchange/01-overview-architecture.md`
  - `docs/superpowers/specs/blockchain-exchange/02-market-kline-storage.md`
  - `docs/superpowers/specs/blockchain-exchange/03-new-coin-lifecycle.md`
  - `docs/superpowers/specs/blockchain-exchange/04-wallet-spot-trading.md`
  - `docs/superpowers/specs/blockchain-exchange/05-admin-agent-permissions.md`
  - `docs/superpowers/specs/blockchain-exchange/06-security-risk-testing.md`
  - `docs/superpowers/specs/blockchain-exchange/07-flash-convert.md`
- 验证结果：已执行引用检查，确认 `认购`、`矿工费`、`unlock_fee`、`new_coin_purchase_orders`、`post-listing-purchase`、`unlock-fee-rule` 在相关拆分文档中存在；占位扫描 `TODO|TBD|FIXME|待定|占位` 无匹配。
- 后续事项：当前仍处于设计文档阶段，尚未进入代码实现计划。

## 2026-05-26 - 建立进度记录与后续会话执行规则

- 完成内容：新增项目级执行规则，要求后续会话先读取项目规则和进度记录；新增持久化进度记录文件，用于记录每次完成的功能、修改文件、验证结果和后续事项。
- 修改文件：
  - `CLAUDE.md`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 `grep -n "进度记录规则\|docs/superpowers/PROGRESS.md\|后续会话" "CLAUDE.md"`，确认项目规则包含进度记录协议；已执行 `grep -n "建立进度记录\|完成内容\|验证结果\|后续事项" "docs/superpowers/PROGRESS.md"`，确认进度文件包含 required fields。
- 后续事项：后续每完成一个任务切片都必须追加更新本文件。

## 2026-05-26 - 完成整体架构设计与新币锁定仓位模型

- 完成内容：依据现有项目文档完善整体架构设计，补充模块化单体分层、核心数据流、部署拓扑、一致性与幂等边界；新增新币锁定仓位模型，明确 `wallet_accounts.locked` 为汇总余额，`asset_lock_positions` 为活跃锁定明细，`asset_lock_position_sources` 为来源追踪；固定日期解禁按 user_id + asset_id + unlock_at 聚合，时间周期解禁按每笔来源订单拆分。
- 修改文件：
  - `docs/superpowers/specs/blockchain-exchange/01-overview-architecture.md`
  - `docs/superpowers/specs/blockchain-exchange/03-new-coin-lifecycle.md`
  - `docs/superpowers/specs/blockchain-exchange/04-wallet-spot-trading.md`
  - `docs/superpowers/specs/blockchain-exchange/05-admin-agent-permissions.md`
  - `docs/superpowers/specs/blockchain-exchange/06-security-risk-testing.md`
- 验证结果：已执行 `grep -R "asset_lock_positions\|asset_lock_position_sources\|fixed_time\|relative_period\|wallet_accounts.locked\|immediate_on_listing\|lock-positions" -n "docs/superpowers/specs/blockchain-exchange"`，确认锁定仓位模型已覆盖架构、新币生命周期、资产账户、后台权限、风控测试文档；已执行 `grep -R "TODO\|TBD\|FIXME\|待定\|占位" -n "docs/superpowers/specs/blockchain-exchange"`，无占位内容；已执行 `grep -R "从派发或上市时间起" -n "docs/superpowers/specs/blockchain-exchange"`，无旧版相对周期解禁表述。
- 后续事项：可进入实现计划阶段，按模块拆分 Rust 后端工程、数据库迁移、领域服务、API、测试与验收任务。

## 2026-05-26 - 完成 Rust 后端工程骨架与基础迁移

- 完成内容：创建 Rust + Axum 模块化单体后端骨架，建立统一配置、状态、错误响应、健康检查、用户/后台/代理路由前缀、基础 infra 模块、领域模块占位、worker 占位、本地环境样例和 Docker Compose；创建 MySQL migration `0001` 到 `0008`，覆盖用户认证、管理员/代理/RBAC、资产钱包流水与锁定仓位、行情策略、现货订单成交、新币生命周期、闪兑、事件 outbox/inbox、风控和审计。
- 修改文件：
  - `Cargo.toml`
  - `.env.example`
  - `docker-compose.yml`
  - `src/main.rs`
  - `src/lib.rs`
  - `src/config.rs`
  - `src/error.rs`
  - `src/state.rs`
  - `src/infra/*`
  - `src/modules/*`
  - `src/workers/*`
  - `migrations/0001_users_auth.sql`
  - `migrations/0002_admin_agent_rbac.sql`
  - `migrations/0003_assets_wallet_ledger_locks.sql`
  - `migrations/0004_market_pairs_strategy.sql`
  - `migrations/0005_spot_orders_trades.sql`
  - `migrations/0006_new_coin_lifecycle.sql`
  - `migrations/0007_flash_convert.sql`
  - `migrations/0008_events_risk_audit.sql`
- 修复内容：修正 Axum router state 类型边界，将路由统一为 `Router<AppState>`；将 RabbitMQ connection 包装为 `Arc<lapin::Connection>` 以满足 `AppState: Clone`；执行 `cargo fmt` 修复格式问题。
- 验证结果：已执行 `cargo fmt --check`，通过；已执行 `cargo check --all-targets`，通过；已执行 `cargo test --all-features`，通过，结果为 2 个单元测试通过、0 失败；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" sqlx migrate run --source migrations`，`0001` 到 `0008` 全部成功应用。执行迁移前已启动 Docker 服务，`docker compose ps` 显示 MySQL healthy，MongoDB、Redis、RabbitMQ up。
- 后续事项：进入 Auth/RBAC、Wallet/Locks、Market/Convert/Events 等并发实现切片。

## 2026-05-26 09:50 - Market/Convert/Events 基础领域助手

- 完成内容：新增行情交易对标准化与白名单校验、K 线 Mongo collection 命名和 upsert key；新增闪兑报价 TTL 校验与 quote_id 幂等键；新增领域事件 routing/idempotency 和 inbox 幂等结构。
- 修改文件：
  - `src/modules/market/mod.rs`
  - `src/infra/mongo.rs`
  - `src/modules/convert/mod.rs`
  - `src/modules/events/mod.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check && cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib market::tests && cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib convert::tests && cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib events::tests`，格式检查通过；market/convert/events 各 2 个测试通过、0 失败。
- 后续事项：无。

## 2026-05-26 - 完成 Auth/RBAC、Wallet/Locks、Market/Convert/Events 基础切片

- 完成内容：完成三组并发基础实现切片。Auth/RBAC 增加 JWT 签发与解析，`Claims` 包含 `scope=user/admin/agent`，并新增 `UserAuth`、`AdminAuth`、`AgentAuth` scope extractor；用户端、管理员端、代理端 auth 路由可签发对应 scope token。Wallet/Locks 增加 available/frozen/locked 余额变更、非负校验、fixed_time 和 immediate_on_listing 聚合 key、relative_period 按来源拆分、locked 汇总一致性校验。Market/Convert/Events 增加交易对标准化与白名单校验、K 线 Mongo collection/upsert key、闪兑报价 TTL 与 quote_id 幂等、领域事件 routing/idempotency 和 inbox 幂等结构。
- 修改文件：
  - `src/modules/auth/mod.rs`
  - `src/modules/auth/routes.rs`
  - `src/modules/wallet/mod.rs`
  - `src/modules/market/mod.rs`
  - `src/infra/mongo.rs`
  - `src/modules/convert/mod.rs`
  - `src/modules/events/mod.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：主线程已重新执行 `cargo fmt --check`，通过；`cargo check --all-targets`，通过；`cargo test --all-features`，通过，18 个测试通过、0 失败；`cargo clippy --all-targets --all-features -- -D warnings`，通过。
- 后续事项：继续实现 Spot Trading、New Coin Lifecycle、Flash Convert 持久化服务、Admin/Agent/Risk、Events/Workers/WebSocket 等切片。

## 2026-05-26 10:37 - Spot Trading MVP 领域切片

- 完成内容：新增现货限价单/市价单纯领域创建校验、交易对启用校验、最小下单额、价格精度、数量精度、订单状态转换、撤单幂等和基础成交填充累计逻辑；新增聚焦集成测试覆盖限价单、市价单、最小下单额、精度拒绝、撤单幂等、partial 到 filled 转换。
- 修改文件：
  - `src/modules/spot/mod.rs`
  - `tests/spot_domain.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 `rustfmt --edition 2024 --check "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/src/modules/spot/mod.rs" "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/tests/spot_domain.rs"`，通过；已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test spot_domain`，7 个测试通过、0 失败。
- 后续事项：无。

## 2026-05-26 10:36 - New Coin Lifecycle MVP 领域切片

- 完成内容：新增新币生命周期纯领域逻辑，覆盖 `preheat -> subscription -> distribution -> listed` 顺序状态迁移、发行期申购准入、上市后认购 `purchase/认购` 标识、`immediate_on_listing` / `fixed_time` / `relative_period` 解禁应用、解禁矿工费 `market_value` / `profit` 计费和未支付阻断释放。
- 修改文件：
  - `src/modules/new_coin/mod.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib new_coin::tests`，7 个测试通过、0 失败。
- 后续事项：无。
## 2026-05-26 10:38 - Admin/Agent/Risk/Workers MVP 领域切片

- 完成内容：新增后台敏感操作二次确认元数据与过期判断；新增代理 `root_agent_id` 团队用户过滤；新增风控审批/拒绝模型，覆盖限频、限额、价格偏离和操作不允许；新增事件重试元数据；新增解禁扫描到期仓位判断；新增 K 线恢复检查点缺口计算。
- 修改文件：
  - `src/modules/admin/mod.rs`
  - `src/modules/agent/mod.rs`
  - `src/modules/risk/mod.rs`
  - `src/modules/events/mod.rs`
  - `src/workers/unlock_scanner.rs`
  - `src/workers/kline_recovery.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib admin::tests`，2 个测试通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib agent::tests`，1 个测试通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib risk::tests`，2 个测试通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib events::tests`，3 个 events 单元测试通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib workers::unlock_scanner::tests`，1 个测试通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib workers::kline_recovery::tests`，2 个测试通过。
- 后续事项：未实现 RabbitMQ/WebSocket/DB 外部集成，按本切片要求保留为后续任务。

## 2026-05-26 - 完成 Spot/New Coin/Admin-Agent-Risk-Workers 主线程验证

- 完成内容：主线程复核并验证最新三个并发领域切片：Spot Trading MVP、New Coin Lifecycle MVP、Admin/Agent/Risk/Workers MVP。验证过程中修复 clippy 发现的 BigDecimal 比较临时对象和测试中无效 `vec!` 问题。
- 修改文件：
  - `src/modules/new_coin/mod.rs`
  - `src/modules/risk/mod.rs`
  - `src/modules/spot/mod.rs`
  - `src/modules/agent/mod.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 `cargo fmt --check`，通过；`cargo check --all-targets`，通过；`cargo test --all-features`，通过，34 个 lib 测试与 7 个 `spot_domain` 测试全部通过、0 失败；`cargo clippy --all-targets --all-features -- -D warnings`，通过。
- 后续事项：继续进入持久化 Repository / Service / API 集成阶段，优先连接 Auth、Wallet、Spot、New Coin、Convert 与 MySQL/Redis/RabbitMQ 边界。

## 2026-05-26 10:59 - Auth MySQL Repository/API 持久化切片

- 完成内容：为 Auth 模块新增 `AuthRepository` 抽象、`MySqlAuthRepository`、`AuthService`；接入用户/管理员/代理注册登录与刷新流程；使用 Argon2 哈希密码并校验登录；使用确定性 Argon2 哈希存储刷新令牌，记录 `actor_type`、`actor_id`、`user_id`、过期时间；路由在缺少 `state.mysql` 时返回清晰 `AppError::Internal`，保持无数据库测试可执行。
- 修改文件：
  - `src/modules/auth/mod.rs`
  - `src/modules/auth/routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 `cargo fmt --check`，通过；已执行 `cargo test --lib auth`，9 个 Auth 相关测试通过、0 失败；额外执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过。
- 后续事项：未执行真实 MySQL 注册/登录集成测试；当前切片只按现有 migration 表结构完成编译安全持久化 wiring。

## 2026-05-26 - 完成持久化 Service Foundation 主线程验证

- 完成内容：主线程复核并验证 Auth 持久化、Wallet/Spot service foundation、New Coin/Convert service foundation 三组切片。Auth 已新增 MySQL repository/service 与持久化注册登录刷新处理；Wallet/Spot 已新增带 ledger 约束的钱包服务、冻结/解冻/结算、锁仓命令和现货 create/cancel/fill 服务；New Coin/Convert 已新增认购锁仓输出、解禁矿工费 gate、闪兑 quote TTL、quote_id 幂等与重复确认拒绝。验证过程中修复 convert large error、spot/wallet BigDecimal 比较等 clippy 问题。
- 修改文件：
  - `src/modules/auth/mod.rs`
  - `src/modules/auth/routes.rs`
  - `src/modules/wallet/mod.rs`
  - `src/modules/spot/mod.rs`
  - `src/modules/new_coin/mod.rs`
  - `src/modules/convert/mod.rs`
  - `tests/wallet_spot_services.rs`
  - `tests/new_coin_convert_services.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 `cargo fmt --check`，通过；`cargo check --all-targets`，通过；`cargo test --all-features`，通过，37 个 lib 测试、5 个 `new_coin_convert_services` 测试、7 个 `spot_domain` 测试、6 个 `wallet_spot_services` 测试全部通过、0 失败；`cargo clippy --all-targets --all-features -- -D warnings`，通过。
- 后续事项：继续补全真实 MySQL transaction repository 实现、API 路由落地、RabbitMQ outbox publisher/consumer、Redis quote cache、WebSocket 推送和端到端集成测试。

## 2026-05-26 11:20 - New Coin/Convert Redis MySQL Repository 基础

- 完成内容：为 New Coin 新增 `MySqlNewCoinRepository`，覆盖 `new_coin_purchase_orders` 幂等插入和 `asset_unlock_records.fee_paid_status` 查询/置 paid；为 Convert 新增 `RedisConvertQuoteCache` 与 `MySqlConvertRepository`，覆盖 Redis quote TTL JSON cache、`convert_quotes` 插入和 `convert_orders` 基于 `quote_id` 的幂等下单。
- 修改文件：
  - `src/modules/new_coin/mod.rs`
  - `src/modules/convert/mod.rs`
  - `tests/new_coin_repositories.rs`
  - `tests/convert_repositories.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test convert_repositories`，2 个测试通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test new_coin_repositories`，1 个测试通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test new_coin_convert_services`，5 个测试通过。当前 shell 中 `DATABASE_URL` / `REDIS_URL` 未设置，新增集成测试按设计跳过外部连接并返回通过；未在本轮实际连接 MySQL/Redis。
- 后续事项：继续补全 API 路由落地、RabbitMQ outbox publisher/consumer、WebSocket 推送和端到端集成测试。

## 2026-05-26 11:18 - Wallet/Spot SQLx Transaction Repository 基础

- 完成内容：为 `MySqlWalletRepository` 新增不破坏同步 trait 的 async SQLx 方法，覆盖 wallet_accounts 创建/读取、wallet_ledger 事务写入与按 ref 查询、asset_lock_positions 和 asset_lock_position_sources 幂等写入；为 `MySqlSpotRepository` 新增 async SQLx 方法，覆盖 trading_pairs 规则读取、spot_orders 插入/读取/更新、spot_trades 插入和按交易对查询；新增可在缺少 `DATABASE_URL` 时跳过的 MySQL 集成测试，覆盖钱包账户/余额流水和现货订单/成交持久化形状。
- 修改文件：
  - `src/modules/wallet/mod.rs`
  - `src/modules/spot/mod.rs`
  - `tests/wallet_spot_sqlx_repositories.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test wallet_spot_sqlx_repositories`，2 个测试通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test wallet_spot_services`，6 个测试通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test spot_domain`，7 个测试通过；已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `rustfmt --edition 2024 --check "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/src/modules/wallet/mod.rs" "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/src/modules/spot/mod.rs" "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/tests/wallet_spot_sqlx_repositories.rs"`，通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过。
- 后续事项：继续补全 API 路由落地、RabbitMQ outbox publisher/consumer、WebSocket 推送和端到端集成测试。

## 2026-05-26 11:35 - RabbitMQ Outbox Worker 与事件路由基础

- 完成内容：新增事件 outbox MySQL repository/service、RabbitMQ publisher envelope 与 lapin publisher shape；新增 inbox 幂等 claim/retry/dead-letter 基础 helper；新增 `/events/outbox/publish-once` Axum 路由并接入主 router，在缺少 MySQL/RabbitMQ 依赖时返回清晰内部错误；新增 outbox worker `run_once`/`run_loop` 基础。
- 修改文件：
  - `src/modules/events/mod.rs`
  - `src/modules/events/routes.rs`
  - `src/workers/event_outbox.rs`
  - `src/workers/mod.rs`
  - `src/lib.rs`
  - `tests/events_outbox.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test events_outbox`，5 个测试通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib events::tests`，3 个 events 单元测试通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib route_prefixes_are_registered`，1 个路由注册测试通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过。
- 后续事项：未执行真实 RabbitMQ broker 发布集成测试。

## 2026-05-26 11:31 - Repository/API/Event 集成切片主线程验证

- 完成内容：主线程复核 Wallet/Spot SQLx repository、New Coin/Convert Redis/MySQL repository、RabbitMQ outbox worker 与事件路由三组集成切片；修复 `tests/new_coin_repositories.rs` 中测试清理函数参数过多导致的 clippy 失败，将清理上下文收敛为 `NewCoinFixtureCleanup`。
- 修改文件：
  - `tests/new_coin_repositories.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：首次执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings` 失败，原因是 `cleanup_new_coin_fixture` 触发 `clippy::too_many_arguments`；修复后重新执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；重新执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；重新执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；重新执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，通过，37 个 lib 测试、2 个 convert repository 测试、5 个 events outbox 测试、5 个 new_coin_convert service 测试、1 个 new_coin repository 测试、7 个 spot_domain 测试、6 个 wallet_spot service 测试、2 个 wallet_spot_sqlx repository 测试全部通过，0 失败。
- 后续事项：继续补全真实 API handlers、RabbitMQ consumer/worker loop、Redis quote 端到端路径、WebSocket 推送、真实外部依赖集成测试与端到端验收。

## 2026-05-26 11:39 - Wallet API Handler 持久化切片

- 完成内容：将钱包用户路由从占位响应替换为真实持久化查询；`GET /wallet/accounts` 基于 `UserAuth` 只返回当前用户资产账户，`GET /wallet/ledger` 支持当前用户流水查询并按 `asset_id`、`ref_type`、`ref_id` 与限制条数过滤；新增无鉴权、缺少 MySQL 依赖和真实 MySQL 路由集成测试。
- 修改文件：
  - `src/modules/wallet/routes.rs`
  - `tests/wallet_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib wallet::routes`，3 个测试通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test wallet_routes`，1 个测试通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，通过，40 个 lib 测试、2 个 convert repository 测试、5 个 events outbox 测试、5 个 new_coin_convert service 测试、1 个 new_coin repository 测试、7 个 spot_domain 测试、1 个 wallet_routes 测试、6 个 wallet_spot service 测试、2 个 wallet_spot_sqlx repository 测试全部通过，0 失败。
- 后续事项：继续补全 Spot/New Coin/Convert 真实 API handlers，RabbitMQ consumer/worker loop，Redis quote 端到端路径，WebSocket 推送和外部依赖集成测试。

## 2026-05-27 14:27 - Admin 闪兑交易对接口与审计原子性

- 完成内容：将后台 `/admin/api/v1/convert/pairs` 和 `/admin/api/v1/convert/pairs/:id` 从占位路由改为 MySQL-backed list/create/update-status 接口，均要求 AdminAuth；新增敏感变更审计，create 与 update-status 在同一 MySQL transaction 内写入业务表与 `admin_audit_logs`，audit 失败时回滚业务变更；补齐 audit FK 失败回滚回归测试，避免“变更已落库但审计缺失”。
- 修改文件：
  - `src/modules/admin/routes.rs`
  - `tests/admin_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_convert_pair_routes_require_admin_scope_and_mysql -- --nocapture`，实现前 `/convert/pairs` 仍返回 stub 200，期望无 MySQL 时 500，失败符合预期；修复后已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check` 通过，`cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets` 通过，`cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings` 通过，`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes -- --nocapture` 4 个测试通过（当前环境未设置 `DATABASE_URL` 时 MySQL 依赖路径按测试设计跳过），`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test convert_routes -- --nocapture` 4 个测试通过，`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features` 全部通过。已执行代码复核，确认 prior blocker 已关闭，无阻断项；复核环境带 `DATABASE_URL` 时 admin_routes 与 convert_routes 均 4 个通过。
- 后续事项：继续推进后台闪兑订单管理、事件 handler 实际业务副作用与事件消费指标告警。

## 2026-05-27 14:38 - Admin 闪兑订单列表接口

- 完成内容：将后台 `/admin/api/v1/convert/orders` 从占位路由改为 MySQL-backed 列表接口，要求 AdminAuth；响应与用户侧闪兑订单列表对齐并额外返回 `user_id`，支持按 `user_id`、`status` 过滤和 `limit` 夹紧，查询使用 SQLx bind 参数避免拼接注入；补齐后台订单列表鉴权、无 MySQL 错误和 seeded 订单过滤测试。
- 修改文件：
  - `src/modules/admin/routes.rs`
  - `tests/admin_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_convert_order_routes_require_admin_scope_and_mysql -- --nocapture`，实现前 `/convert/orders` 仍返回 stub 200，期望无 MySQL 时 500，失败符合预期；修复后已执行同命令通过。已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_convert_orders_list_filters_by_user_and_status -- --nocapture`，当前环境未设置 `DATABASE_URL` 时 MySQL seeded 路径按测试设计跳过。已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check` 通过，`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes -- --nocapture` 6 个通过，`cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets` 通过，`cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings` 通过，`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test convert_routes -- --nocapture` 4 个通过，`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features` 全部通过。已执行代码复核，无阻断项；复核建议 CI 必须提供 `DATABASE_URL` 跑完整 MySQL 集成路径。
- 后续事项：继续推进后台闪兑新币规则、事件 handler 实际业务副作用与事件消费指标告警。

## 2026-05-27 15:04 - Admin 闪兑新币固定汇率规则

- 完成内容：将后台 `/admin/api/v1/convert/new-coin-rules` 从占位路由改为 MySQL-backed create/upsert 接口，要求 AdminAuth；同一 `convert_pair_id` 重复提交会更新现有规则并在同一 MySQL transaction 内写入 `admin_audit_logs`；后台仅允许 `rate_source = fixed` 且要求正数 `fixed_rate`，拒绝非固定规则；用户闪兑报价查询增加 `rules.rate_source = 'fixed'` 防线，避免非固定 active 规则被当作固定汇率消费。
- 修改文件：
  - `src/modules/admin/routes.rs`
  - `src/modules/convert/routes.rs`
  - `tests/admin_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_convert_new_coin_rule_routes_require_admin_scope_and_mysql -- --nocapture`，实现前 `/convert/new-coin-rules` 仍返回 stub 200，期望无 MySQL 时 500，失败符合预期；代码复核发现非 fixed `rate_source` blocker 后，新增 regression 并确认修复前返回 500、期望 400，失败符合预期。修复后已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check && cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets && cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings && cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes -- --nocapture && cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test convert_routes -- --nocapture && cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全部通过；当前环境未设置 `DATABASE_URL`，MySQL seeded 分支按测试设计跳过。已执行复核，确认 prior blocker 已关闭，无阻断项；复核建议 CI 必须提供 `DATABASE_URL` 跑完整 MySQL 集成路径。
- 后续事项：继续推进事件 handler 实际业务副作用、事件消费指标告警与后台/代理剩余管理接口加固。

## 2026-05-27 15:31 - Admin 新币项目创建与列表接口

- 完成内容：将后台 `/admin/api/v1/new-coins` 从占位路由改为 MySQL-backed create/list 接口，均要求 AdminAuth；创建接口在访问 MySQL 前校验生命周期、供应量、发行价、symbol、解禁规则和矿工费规则；新币项目、`new_coin_lifecycle_events` 与 `admin_audit_logs` 在同一 MySQL transaction 内写入；补齐 `immediate_on_listing` 创建期不强制 `listed_at` 的回归，保留固定时间/相对周期字段互斥校验。
- 修改文件：
  - `src/modules/admin/routes.rs`
  - `tests/admin_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_new_coin_project_routes_require_admin_scope_and_mysql -- --nocapture`，实现前 `/new-coins` 仍返回 stub 200，期望 invalid unlock config 返回 400，失败符合预期；代码复核发现 `immediate_on_listing` 创建期错误要求 `listed_at`，新增 regression 后修复前返回 400、期望 500，失败符合预期。修复后已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check && cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets && cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings && cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes -- --nocapture && cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test new_coin_routes -- --nocapture && cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全部通过；当前环境未设置 `DATABASE_URL`，MySQL seeded 分支按测试设计跳过。已执行复核，确认 blocker 已关闭，无阻断项。
- 后续事项：继续推进后台新币生命周期变更、派发、解禁规则/矿工费规则更新，以及事件消费指标告警。

## 2026-05-27 15:58 - Admin 新币生命周期流转接口

- 完成内容：将后台 `/admin/api/v1/new-coins/:id/lifecycle` 从占位路由改为 MySQL-backed PATCH 接口，要求 AdminAuth；无效生命周期值在访问 MySQL 前返回 validation；业务路径在 transaction 内 `FOR UPDATE` 锁定新币项目，复用 `LifecycleStatus::transition_to` 仅允许 `preheat -> subscription -> distribution -> listed` 顺序流转，上市时写入请求提供的 `listed_at` 或当前时间；同一 transaction 内写入 `new_coin_lifecycle_events` 与 `admin_audit_logs`，事件与审计均包含 before/after JSON，非法跳级或回退不会修改状态。
- 修改文件：
  - `src/modules/admin/routes.rs`
  - `tests/admin_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_new_coin_lifecycle_routes_require_admin_scope_and_mysql -- --nocapture`，实现前 `/new-coins/:id/lifecycle` 仍返回 stub 200，期望无效 lifecycle 返回 400，失败符合预期。实现后已执行 focused GREEN：同命令通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_new_coin_lifecycle_transition_updates_project_events_and_audits -- --nocapture`，当前环境未设置 `DATABASE_URL` 时 MySQL seeded 路径按测试设计跳过。已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check && cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets && cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings && cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes -- --nocapture && cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test new_coin_routes -- --nocapture && cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全部通过：`admin_routes` 12 个通过、`new_coin_routes` 7 个通过，full suite 全部 lib/integration/doc tests 通过。已执行代码复核，无阻断项；复核提醒当前无 `DATABASE_URL`，MySQL seeded 路径需在 CI 或本地 MySQL 环境补跑。
- 后续事项：继续推进后台新币派发、解禁规则/矿工费规则更新、后台新币订单/锁仓/解禁列表，以及事件消费指标告警。

## 2026-05-28 06:52 - Admin 新币派发接口

- 完成内容：将后台 `/admin/api/v1/new-coins/:id/distribute` 从占位路由改为 MySQL-backed POST 接口，要求 AdminAuth；请求在访问 MySQL 前校验派发数量与幂等键；业务路径在 transaction 内锁定新币项目并要求 `distribution` 生命周期；按项目解禁规则将派发数量写入钱包 `available` 或 `locked`，创建/更新锁仓和锁仓来源，写入 `wallet_ledger`；同一 transaction 内写入 `new_coin_distributions`、`new_coin_lifecycle_events` 与 `admin_audit_logs`；重复幂等键和带空格重复幂等键均返回 conflict。
- 修改文件：
  - `src/modules/admin/routes.rs`
  - `tests/admin_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_new_coin_distribution_routes_require_admin_scope_and_mysql -- --nocapture`，实现前 `/new-coins/:id/distribute` 仍返回 stub 200，期望 invalid quantity 返回 400，失败符合预期。实现后已执行 focused GREEN：同命令通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_new_coin_distribution_creates_wallet_lock_event_and_audit -- --nocapture`，当前环境未设置 `DATABASE_URL` 时 MySQL seeded 路径按测试设计跳过。已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`、`cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`、`cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`、`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes -- --nocapture`、`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test new_coin_routes -- --nocapture`、`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全部通过；`admin_routes` 14 个通过、`new_coin_routes` 7 个通过，full suite 全部 lib/integration/doc tests 通过。已执行两轮代码复核，prior blockers 已修复，未发现阻断项。
- 后续事项：继续推进后台新币解禁规则/矿工费规则更新、后台新币订单/锁仓/解禁列表，以及事件消费指标告警。

## 2026-05-28 07:12 - Admin 新币解禁规则与矿工费规则更新接口

- 完成内容：将后台 `/admin/api/v1/new-coins/:id/unlock-rule` 与 `/admin/api/v1/new-coins/:id/unlock-fee-rule` 从占位路由改为 MySQL-backed PATCH 接口，均要求 AdminAuth；请求在访问 MySQL 前校验解禁规则形态、矿工费开关、费率、计费依据和费用资产；业务路径在 transaction 内 `FOR UPDATE` 锁定新币项目，更新规则后写入 `new_coin_lifecycle_events` 与 `admin_audit_logs`，事件和审计均包含 before/after JSON；修复 fixed_time/relative_period 更新时误清空已上市项目 `listed_at` 的回归，确保仅 immediate_on_listing 更新会改写 `listed_at`。
- 修改文件：
  - `src/modules/admin/routes.rs`
  - `tests/admin_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_new_coin_unlock_rule_routes_require_admin_scope_and_mysql -- --nocapture` 与 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_new_coin_unlock_fee_rule_routes_require_admin_scope_and_mysql -- --nocapture`，实现前两个 stub 均返回 200，期望 invalid request 返回 400，失败符合预期。实现后已执行 focused GREEN：上述两条命令通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_new_coin_rule_updates_modify_project_events_and_audits -- --nocapture`，当前环境未设置 `DATABASE_URL` 时 MySQL seeded 路径按测试设计跳过。修复 `listed_at` 回归后已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`、`cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`、`cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`、`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes -- --nocapture`、`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test new_coin_routes -- --nocapture`、`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全部通过；复核 agent 已确认 Task #140 无剩余 blocker。
- 后续事项：继续推进后台新币订单/锁仓/解禁列表，以及事件消费指标告警。

## 2026-05-28 07:35 - Admin 新币订单锁仓解禁列表接口

- 完成内容：将后台 `/admin/api/v1/new-coins/:id/subscriptions`、`/admin/api/v1/new-coins/:id/distributions`、`/admin/api/v1/new-coins/purchases`、`/admin/api/v1/new-coins/lock-positions`、`/admin/api/v1/new-coins/unlocks` 从占位路由改为 MySQL-backed GET 接口，均要求 AdminAuth；申购和派发列表按项目限定并支持 `user_id`、`status`、`limit` 过滤；认购列表支持 `project_id`、`user_id`、`status`、`limit` 过滤；锁仓列表支持 `user_id`、`asset_id`、`status`、`limit` 过滤；解禁列表支持 `user_id`、`asset_id`、`status`、`fee_paid_status`、`limit` 过滤；所有动态条件均使用 SQLx bind 参数，只读查询不写审计、不修改业务表。
- 修改文件：
  - `src/modules/admin/routes.rs`
  - `tests/admin_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_new_coin_listing_routes_require_admin_scope_and_mysql -- --nocapture`，实现前 `/new-coins/:id/subscriptions` 仍返回 stub 200，期望无 MySQL 返回 500，失败符合预期；seeded RED 测试 `admin_new_coin_listing_routes_filter_seeded_records` 当前环境未设置 `DATABASE_URL` 时按设计跳过。实现后已执行 focused GREEN：上述两个测试均通过。已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`、`cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`、`cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`、`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes -- --nocapture`、`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test new_coin_routes -- --nocapture`、`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全部通过；`admin_routes` 19 个通过、`new_coin_routes` 7 个通过、full suite 全部 lib/integration/doc tests 通过。已执行代码复核，未发现 blocker 或 important 问题。
- 后续事项：继续推进事件消费指标告警，以及后台/代理剩余管理接口加固。

## 2026-05-28 08:19 - Event Inbox 指标快照与告警分类

- 完成内容：为事件 inbox 消费结果新增批次指标快照，统计 `consumed`、`duplicates`、`retried`、`dead_lettered` 与总数；新增告警分类，区分 retry backlog、dead letter、processing error、malformed delivery 的 warning/critical 级别；RabbitMQ delivery 处理改为先归一化 `ProcessedInboxDelivery`，坏消息 ACK 后不再向外层冒泡为通用错误；已记录 retry/dead-letter 结果 ACK，内部处理错误 reject/requeue；MySQL inbox claim 主路径和插入唯一冲突 fallback 共用状态判定，避免 `processing` 行被误判为 duplicate ACK，并在 retry 行未到 `next_retry_at` 时拒绝提前 claim。
- 修改文件：
  - `src/modules/events/mod.rs`
  - `tests/events_inbox.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test events_inbox inbox_consumer_batch_exposes_metrics_snapshot_and_alerts -- --nocapture`，实现前缺少 `EventInboxAlert*` 与 `ConsumedInboxBatch::metrics()`，编译失败符合预期；修复 MySQL insert race regression 前执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib existing_processing_inbox_row_returns_error_for_requeue_after_insert_race -- --nocapture`，缺少 `ExistingInboxMessage` 与 `decide_existing_inbox_claim`，失败符合预期。实现后已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib existing_processing_inbox_row_returns_error_for_requeue_after_insert_race -- --nocapture`，1 个通过；已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`、`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test events_inbox -- --nocapture`、`cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`、`cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`、`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全部通过；`events_inbox` 23 个通过，full suite 全部 lib/integration/doc tests 通过。已执行两轮代码复核，最终无 blocker 或 important 问题。
- 后续事项：继续补 DB retry scanner，确保已写入 `retry` 且到期的 inbox 行能被可靠重新扫描处理。

## 2026-05-28 10:21 - Event Inbox DB 重试扫描与并发 fencing

- 完成内容：补齐 Event Inbox 的 MySQL retry scanner，支持从 `event_inbox.payload_json` 重建消息并重放到期 `retry` 行；新增 `payload_json` 迁移和 legacy 缺失 payload 死信处理；补齐 stale `processing` 行扫描、重新领取和 processing token fencing，防止旧 worker 覆盖新 worker 结果；启动入口同时运行 RabbitMQ consumer 与 DB retry scanner；修复多实例 scanner 并发抢同一行时整批中断的问题，已被其他实例领取的行按 duplicate 跳过并继续处理后续行。
- 修改文件：
  - `src/modules/events/mod.rs`
  - `src/workers/event_inbox.rs`
  - `src/main.rs`
  - `src/config.rs`
  - `.env.example`
  - `migrations/0012_event_inbox_payload_json.sql`
  - `migrations/0013_event_inbox_missing_payload_processing_dead_letter.sql`
  - `tests/events_inbox.rs`
- 验证结果：已执行 RED：`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test events_inbox inbox_retry_scanner_skips_rows_claimed_by_another_scanner -- --nocapture`，实现前因 `event inbox message is already processing` 直接冒泡导致测试失败，符合预期；已执行 RED：`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test events_inbox event_inbox_payload_backfill_migration_marks_missing_outbox_rows -- --nocapture`，实现前缺少 legacy missing payload 条件修正，失败符合预期。修复后已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；`cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；`cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test events_inbox -- --nocapture`，32 个测试通过；`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test events_inbox -- --nocapture`，32 个测试通过，其中 MySQL scanner 测试实际执行通过，fencing 测试因缺少 `REDIS_URL`/`MONGO_URL` 按测试设计跳过；`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试通过。已执行代码复核，确认无 blocker 或 important 问题。
- 后续事项：继续推进剩余后台/代理管理接口加固与交易所未完成业务模块。

## 2026-05-28 11:51 - Admin 代理管理接口

- 完成内容：将后台代理管理路由从占位实现改为 MySQL-backed 接口，覆盖代理创建、代理状态更新、代理团队用户列表、用户改派代理和代理佣金列表；创建、状态更新和用户改派均在 MySQL transaction 内完成业务变更与 `admin_audit_logs` 审计；创建代理前校验并锁定用户存在，重复代理映射为 409，缺失用户返回 404；用户改派支持 `root_agent_id = NULL` 的既有归属，并迁移旧归属下的邀请子树，同时用 `root_agent_id <=> old_root_agent_id` 避免同 path 前缀但不同旧 root 的无关团队被误迁移。
- 修改文件：
  - `src/modules/admin/routes.rs`
  - `tests/admin_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_agent_management_routes_require_admin_scope_mysql_and_validation -- --nocapture`，实现前 `user_id = 0` 返回 500、期望 400，失败符合预期；已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_agent_management_create_update_assign_list_and_audit -- --nocapture`，实现前缺失用户创建代理返回 FK 数据库错误 500、期望 404，失败符合预期。修复后已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`、`cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`、`cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`、`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes -- --nocapture`、`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全部通过；`admin_routes` 21 个 MySQL 测试通过，full suite 全部 lib/integration/doc tests 通过。已执行代码复核，最终无 blocker 或 important 问题。
- 后续事项：继续规划并推进下一组后台/代理管理接口或交易所未完成业务模块。

## 2026-05-28 12:44 - Admin 新币上市后认购配置接口

- 完成内容：将后台 `/admin/api/v1/new-coins/:id/post-listing-purchase` 从占位路由改为 MySQL-backed PATCH 接口，要求 AdminAuth；新增 `new_coin_projects.post_listing_purchase_enabled` 与 `post_listing_pair_id` 迁移；启用认购时要求项目已上市、交易对属于新币资产，并在同一 transaction 内激活交易对、更新项目配置、写入生命周期事件和后台审计；关闭认购时清空绑定交易对。用户端上市后认购同步强制检查后台开关和绑定交易对，并在购买 transaction 内重新 `FOR UPDATE` 锁定项目和交易对，基于锁定后的项目规则重新计算锁仓计划，避免后台关闭认购或修改解禁规则时用户按旧快照成交。
- 修改文件：
  - `migrations/0014_new_coin_post_listing_purchase_config.sql`
  - `src/modules/admin/routes.rs`
  - `src/modules/new_coin/routes.rs`
  - `tests/admin_routes.rs`
  - `tests/new_coin_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_new_coin_post_listing_purchase_routes_require_admin_scope_and_validation -- --nocapture`，实现前后台占位路由对缺失 `pair_id` 返回 200、期望 400，失败符合预期；已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test new_coin_routes new_coin_purchase_requires_enabled_post_listing_pair -- --nocapture`，实现前用户可绕过后台开关成交返回 200、期望 400，失败符合预期。修复后已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`、`cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`、`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test new_coin_routes -- --nocapture`、`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes -- --nocapture`、`cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`、`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全部通过；`new_coin_routes` 8 个通过，`admin_routes` 23 个 MySQL 测试通过，full suite 全部 lib/integration/doc tests 通过。已执行三轮代码复核，修复事务外开关检查和事务外锁仓规则计算两个 Important 问题，最终无剩余 blocker 或 important 问题。
- 后续事项：继续推进剩余行情 ticker 接口和交易所未完成业务模块。

## 2026-05-28 13:21 - Market 行情 Ticker 查询接口

- 完成内容：将用户侧 `/api/v1/markets/:symbol/ticker` 从占位响应改为 Redis-backed GET 接口；请求进入 Redis 前先复用行情 symbol 校验和上市交易对校验；Redis 未配置时返回清晰内部错误；命中缓存时只返回 `symbol`、`last_price`、`volume_24h`、`observed_at` 的 ticker 响应，沿用行情摄取写入的 `market:ticker:<symbol>` 缓存键，并保持 K 线查询接口行为不变。
- 修改文件：
  - `src/modules/market/routes.rs`
  - `tests/market_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test market_routes market_ticker_route_returns_clear_error_without_redis -- --nocapture`，实现前 ticker stub 返回 200、期望 500，失败符合预期。修复后已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test market_routes market_ticker_route_returns_clear_error_without_redis -- --nocapture` 与 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test market_routes market_ticker_route_rejects_invalid_symbol_before_redis -- --nocapture`，均通过；已执行 `REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test market_routes market_ticker_route_reads_latest_cached_ticker -- --nocapture`，通过。最终已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`、`cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`、`cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`、`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test market_routes -- --nocapture`、`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全部通过；`market_routes` 7 个通过，full suite 全部 lib/integration/doc tests 通过。已执行代码复核，未发现 blocker 或 important 问题。
- 后续事项：继续查找并推进剩余占位路由或未完成业务模块。

## 2026-05-28 13:31 - Market 行情交易对列表接口

- 完成内容：将用户侧 `/api/v1/markets` 从空列表占位行为改为 MySQL-backed 活跃交易对列表；MySQL 已配置时查询 `trading_pairs` 并关联 base/quote `assets`，只返回 `status = active` 的交易对，包含 symbol、base_asset、quote_asset、price_precision、qty_precision、min_order_value、status、market_type；MySQL 未配置时保留轻量 fallback，避免破坏无数据库路由测试；保持 ticker 和 K 线接口行为不变。
- 修改文件：
  - `src/modules/market/routes.rs`
  - `tests/market_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test market_routes market_list_route_returns_active_pairs_from_mysql -- --nocapture`，实现前 `/markets` 返回空列表，seeded active pair 不存在于响应中，失败符合预期。修复后同命令通过。最终已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`、`cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`、`cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`、`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test market_routes -- --nocapture`、`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全部通过；`market_routes` 8 个通过，full suite 全部 lib/integration/doc tests 通过。已执行代码复核，未发现 blocker 或 important 问题。
- 后续事项：继续查找并推进剩余未完成业务模块。

## 2026-05-28 13:48 - Convert 闪兑确认原子结算

- 完成内容：修复用户侧 `/api/v1/convert/confirm` 的确认与结算事务边界；将 `convert_orders` 插入、钱包行锁定、余额更新、订单完成和双边 `wallet_ledger` 写入收敛到同一个 MySQL transaction 内，确保结算失败时不会留下 `pending` 闪兑订单，也不会让用户重试时被错误判定为重复确认。新增回归测试覆盖缺少目标钱包导致首次结算失败、订单回滚、补齐钱包后同一 quote 可成功重试的路径。
- 修改文件：
  - `src/modules/convert/routes.rs`
  - `tests/convert_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test convert_routes convert_confirm_rolls_back_order_when_settlement_fails_and_allows_retry -- --nocapture`，实现前结算失败后 `convert_orders` 仍残留 1 条记录、期望 0，失败符合预期。修复后同命令通过。最终已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`、`cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`、`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test convert_routes -- --nocapture`、`cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`、`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全部通过；`convert_routes` 5 个通过，full suite 全部 lib/integration/doc tests 通过。已执行代码复核，未发现 blocker 或 important 问题。
- 后续事项：继续查找并推进剩余未完成业务模块。

## 2026-05-29 02:38 - Spot 下单与成交资金原子性硬化

- 完成内容：加固现货下单、撤单、成交资金一致性；下单插入订单与钱包冻结同事务提交；撤单状态更新与订单级剩余预留解冻同事务提交；成交幂等键先占位并在重复键时回滚重放，避免并发重复 `idempotency_key` 暴露原始数据库 500；成交预留校验排除当前占位成交，保留买单 quote 和卖单 base 的订单级预留校验；成交结算前按 `(user_id, asset_id)` 稳定顺序预锁买卖双方 base/quote 钱包行，降低交叉方向成交死锁风险。
- 修改文件：
  - `src/modules/spot/routes.rs`
  - `tests/spot_routes.rs`
  - `migrations/0015_spot_order_reservations.sql`
  - `migrations/0019_spot_order_reservation_total_backfill.sql`
  - `migrations/0020_spot_order_reservation_ledger_backfill.sql`
- 验证结果：已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test spot_routes -- --nocapture`，28 个测试通过、0 失败；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全部测试通过、0 失败。
- 后续事项：等待最终 code review 结果；若无 blocker/important，继续下一个后端硬化切片。

## 2026-05-29 03:48 - Spot 成交锁顺序与幂等重放修复

- 完成内容：修复现货 `/spot/fills` 最终复核发现的成交并发与幂等问题；买卖订单先解析为 canonical 主键并按主键稳定顺序 `FOR UPDATE` 锁定，再映射回请求中的 buy/sell 角色，避免 A/B 与 B/A 请求交叉等待；订单锁定查询只锁 `spot_orders`，将交易对 symbol 查询拆成无 `FOR UPDATE` 的独立读取，避免无效跨交易对请求锁住 `trading_pairs` 行；成交幂等重放使用已锁定订单的 canonical ID 校验，支持带前导零的订单 ID 原请求体重复提交；成交流水 `ref_id` 统一使用 canonical buy/sell 订单 ID；测试资产 symbol 改用 UUID v7 后段，降低并行测试 timestamp 前缀碰撞。
- 修改文件：
  - `src/modules/spot/routes.rs`
  - `tests/spot_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib spot::routes::tests::spot_fill_order_lock_keys_are_canonical_sorted_and_unique`，实现前因缺少 `spot_fill_order_lock_keys` 编译失败，失败符合预期；已执行 RED：`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib spot::routes::tests::locked_spot_order_response_keeps_pair_id_without_locking_pair_row`，实现前因缺少 lock-row helper 编译失败，失败符合预期。修复后已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib spot::routes::tests::spot_fill_order_lock_keys_are_canonical_sorted_and_unique`、`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib spot::routes::tests::locked_spot_order_response_keeps_pair_id_without_locking_pair_row`、`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test spot_routes spot_fill_replays_leading_zero_order_ids_idempotently -- --nocapture`、`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test spot_routes spot_fill_concurrent_duplicate_key_rejects_mismatched_request_without_500 -- --nocapture`，均通过。最终已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`、`cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`、`cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`、`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test spot_routes -- --nocapture`、`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全部通过；`spot_routes` 29 个通过，full suite 全部 lib/integration/doc tests 通过。已执行两轮代码复核，修复 pair row `FOR UPDATE` 死锁风险后最终返回 `[]`，无 blocker 或 important 问题。
- 后续事项：继续查找并推进剩余未完成业务模块。

## 2026-05-29 04:41 - 秒合约 MVP Foundation

- 完成内容：新增秒合约最小后端切片，包含产品表与订单表 migration、用户 active 产品列表、管理员全量产品列表、用户开仓接口、钱包 available 扣款、wallet_ledger 流水记录、用户级 idempotency_key 顺序/并发重放保护；修复 code review 发现的 MySQL 完整性约束误判问题，仅将真实 duplicate entry 作为幂等冲突处理，并补充外键失败回归测试。
- 修改文件：
  - `migrations/0021_seconds_contracts.sql`
  - `src/modules/seconds_contract/mod.rs`
  - `src/modules/seconds_contract/routes.rs`
  - `src/modules/mod.rs`
  - `src/lib.rs`
  - `tests/seconds_contract_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test seconds_contract_routes -- --nocapture`，7 个测试通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试通过。
- 后续事项：继续补齐秒合约结算 worker、后台产品配置与审计、风控/代理佣金/事件推送；后续还需继续实现杠杆与理财产品切片。

## 2026-05-29 05:38 - 杠杆交易 MVP Foundation

- 完成内容：新增杠杆交易最小后端切片，包含杠杆产品表与仓位表 migration、用户 active 产品列表、管理员全量产品列表、用户开仓接口、保证金资产 available 扣款、wallet_ledger 流水记录、用户级 idempotency_key 顺序/并发重放保护；修复复核发现的产品禁用后同 key 重试不能 replay 原仓位问题，并将前置幂等查询改为事务外只读查询以降低锁冲突。
- 修改文件：
  - `migrations/0022_margin_trading.sql`
  - `src/modules/margin/mod.rs`
  - `src/modules/margin/routes.rs`
  - `src/modules/mod.rs`
  - `src/lib.rs`
  - `tests/margin_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test margin_routes margin_routes_require_expected_scope -- --nocapture`，实现前因缺少 `modules::margin` 编译失败，符合预期；已执行产品禁用后幂等重放 RED，修复前返回 `NOT_FOUND`，符合预期。修复后已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test margin_routes -- --nocapture`，7 个测试通过；已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib route_prefixes_are_registered -- --nocapture`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试通过。代码复核确认 prior Important 已解决，无剩余 blocker/important。
- 后续事项：继续补齐杠杆平仓/强平、利息/借贷、风控、后台产品配置与审计、代理佣金和事件推送。

## 2026-05-29 05:38 - 理财 Earn MVP Foundation

- 完成内容：新增理财 Earn 最小后端切片，包含理财产品表与订阅表 migration、用户 active 产品列表、管理员全量产品列表、用户订阅接口、订阅资产 available 扣款、wallet_ledger 流水记录、用户级 idempotency_key 顺序/并发重放保护；接入 `/api/v1/earn/products`、`/api/v1/earn/subscriptions` 与 `/admin/api/v1/earn/products`；根据代码复核修复超过 `DECIMAL(38,18)` 小数位的金额被数据库归一化后破坏幂等重放的问题，订阅金额超过 18 位小数或整数位超过存储精度时提前返回 validation。
- 修改文件：
  - `migrations/0023_earn_products.sql`
  - `src/modules/earn/mod.rs`
  - `src/modules/earn/routes.rs`
  - `src/modules/mod.rs`
  - `src/lib.rs`
  - `tests/earn_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test earn_routes earn_routes_require_expected_scope -- --nocapture`，实现前因缺少 `modules::earn` 编译失败，符合预期；已执行金额精度复核回归 RED，修复前返回缺少 MySQL 的 500、期望 400，符合预期。修复后已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test earn_routes earn_subscribe_rejects_amount_scale_above_decimal_storage -- --nocapture`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test earn_routes -- --nocapture`，7 个测试通过；已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib route_prefixes_are_registered -- --nocapture`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试通过。
- 后续事项：金额精度修复复审已确认无 blocker/important；继续下一个交易所业务切片。

## 2026-05-29 06:08 - 秒合约结算 MVP

- 完成内容：新增后台秒合约结算接口 `POST /seconds-contracts/orders/:id/settle`，要求 AdminAuth；结算事务内 `FOR UPDATE` 锁定订单，`win` 按本金加收益返还用户钱包 available 并写入一条 `seconds_contract_settle_win` 流水，`loss` 只标记订单结算不返还；同结果重复结算返回等价 replay 响应且不重复入账/流水，不同结果重复结算返回 conflict。根据代码复核补齐不同结果 replay 回归测试，并修正秒合约产品列表测试对真实 admin HTTP status 的断言；修复 full suite 中市场列表测试在集成库累积活跃交易对过多时触发 body 长度限制的问题。
- 修改文件：
  - `src/modules/seconds_contract/routes.rs`
  - `tests/seconds_contract_routes.rs`
  - `tests/market_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：秒合约结算路由实现前 `seconds_contract_settle_win_credits_payout_and_writes_ledger` 返回 404、期望 200，符合预期；已执行代码复核，prior Important 为 settled win replay 返回 `payout_amount = 0`，已修复为 replay 同样返回本金加收益。最终已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；`cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；`cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test seconds_contract_routes -- --nocapture`，10 个测试通过；`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test market_routes market_list_route_returns_active_pairs_from_mysql -- --nocapture`，通过；`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib route_prefixes_are_registered -- --nocapture`，通过；`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试通过。
- 后续事项：继续推进秒合约后台产品配置/审计、自动到期结算 worker、行情结果判定、风控/代理佣金和事件推送。

## 2026-05-29 06:46 - 理财 Earn 到期赎回 MVP

- 完成内容：新增用户侧理财赎回接口 `POST /earn/subscriptions/:id/redeem`，要求 UserAuth；赎回事务内 `FOR UPDATE` 锁定订阅和钱包，到期后按本金加 `amount * apr_rate * term_days / 365` 简单收益返还钱包 available，写入单条 `earn_redeem` 流水并标记订阅 `redeemed`；重复赎回只回放响应，不重复入账或写流水。根据代码复核补齐已赎回 replay 一致性回归，修复 replay 从可变订阅字段重算金额的问题，改为从 `wallet_ledger` 的原始 `earn_subscribe` 与 `earn_redeem` 流水恢复本金、收益和赎回总额。
- 修改文件：
  - `src/modules/earn/routes.rs`
  - `tests/earn_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：新增 replay consistency 回归后，修改已赎回订阅的 `amount/apr_rate/term_days` 再次赎回返回 `principal_amount = 100.000000000000000000`、期望原始 `365.000000000000000000`，失败符合预期；修复后已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test earn_routes earn_redeem_matured_subscription_credits_principal_yield_and_writes_ledger -- --nocapture`，通过；`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test earn_routes -- --nocapture`，9 个测试通过；`cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；`cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；`cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib route_prefixes_are_registered -- --nocapture`，通过；`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试通过。已执行代码复核，确认 prior Important 已关闭，无 blocker 或 important 问题。
- 后续事项：继续推进理财后台产品配置/审计、自动到期赎回 worker、理财事件推送，以及秒合约结算 worker 和其他交易所未完成业务模块。

## 2026-05-29 07:20 - 理财 Earn 后台产品配置与审计

- 完成内容：新增后台理财产品配置闭环，`/admin/api/v1/earn/products` 支持 AdminAuth 创建与列表，`/admin/api/v1/earn/products/:id/status` 支持状态更新；创建和状态更新均在业务事务内写入 `admin_audit_logs`，审计失败会回滚产品变更；补齐产品名称、期限、APR、金额、资产、状态和审计 reason 长度校验；用户订阅在事务内锁定产品行，防止后台禁用并发竞态；修复产品禁用后的幂等重放语义，同 key 同请求可 replay，同 key 不同请求保留 409 conflict。
- 修改文件：
  - `src/modules/earn/routes.rs`
  - `tests/earn_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：后台创建 reason 超过 512 字符时实现前返回 500、期望 400；后台更新状态 reason 超过 512 字符时实现前返回 500、期望 400；产品禁用并发重放同 idempotency_key 但不同 amount 时实现前返回 404、期望 409，均符合预期。修复后已执行三个 focused GREEN 测试，均通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test earn_routes -- --nocapture`，15 个测试通过；已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" route_prefixes_mount_expected_modules -- --nocapture`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试通过。已执行最终代码复核，无 blocker 或 important 问题。
- 后续事项：继续推进理财自动到期赎回 worker、理财事件推送、秒合约自动结算 worker、后台产品配置审计扩展和其他交易所未完成业务模块。

## 2026-05-29 08:45 - 秒合约后台产品配置与审计

- 完成内容：新增后台秒合约产品配置闭环，`/admin/api/v1/seconds-contracts/products` 支持 AdminAuth 创建与列表，`/admin/api/v1/seconds-contracts/products/:id/status` 支持状态更新；创建和状态更新均在业务事务内写入 `admin_audit_logs`，审计失败会回滚产品变更；补齐交易对、质押资产、周期、赔率、金额、状态和审计 reason 长度校验；用户开仓在事务内 `FOR UPDATE` 锁定产品行，防止后台禁用并发竞态；修复产品禁用后的幂等重放语义，同 key 同请求可 replay，同 key 不同请求保留 409 conflict。
- 修改文件：
  - `src/modules/seconds_contract/routes.rs`
  - `tests/seconds_contract_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：后台产品路由未实现时新增测试出现空响应体解析失败，后台禁用并发竞态返回 200、期望 404，产品禁用后原 idempotency key 重放返回 404、期望 200，均符合预期。修复后已执行 focused GREEN 测试，均通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test seconds_contract_routes -- --nocapture`，16 个测试通过；已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" route_prefixes_mount_expected_modules -- --nocapture`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试通过。最终代码复核已批准，无 blocker 或 important 问题；复核仅建议后续补充 agent scope、状态更新审计回滚、缺失 stake_asset 和金额边界测试。
- 后续事项：继续推进杠杆后台产品配置/审计、理财自动赎回 worker、秒合约自动结算 worker 和其他交易所未完成业务模块。

## 2026-05-29 09:14 - 杠杆后台产品配置与审计

- 完成内容：新增后台杠杆产品配置闭环，`/admin/api/v1/margin/products` 支持 AdminAuth 创建与列表，`/admin/api/v1/margin/products/:id/status` 支持状态更新；创建和状态更新均在业务事务内写入 `admin_audit_logs`，审计失败会回滚产品变更；补齐交易对、保证金资产、最大杠杆、最小/最大保证金、维持保证金率、状态和审计 reason 长度校验；用户开仓在事务内 `FOR UPDATE` 锁定产品行，防止后台禁用并发竞态；保留产品禁用后的幂等重放语义，同 key 同请求可 replay，同 key 不同请求保留 409 conflict。
- 修改文件：
  - `src/modules/margin/routes.rs`
  - `tests/margin_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：后台产品路由未实现时新增测试出现空响应体解析失败，后台禁用并发竞态返回 200、期望 404，均符合预期。修复后已执行 focused GREEN 测试，均通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test margin_routes -- --nocapture`，12 个测试通过；已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib route_prefixes_are_registered -- --nocapture`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试通过。最终代码复核已批准，无 blocker 或 important 问题；复核仅建议后续补充缺失 margin_asset、状态更新审计回滚和更精确的 create/PATCH user scope 测试。
- 后续事项：继续推进杠杆平仓/强平、利息/借贷、秒合约自动结算 worker、理财自动赎回 worker 和其他交易所未完成业务模块。

## 2026-05-29 10:29 - 新币解禁扫描生产释放循环

- 完成内容：补齐生产可运行的 unlock scanner release loop；按配置在启动时调度扫描；扫描到期 active 锁仓对应的 `pending` 解禁记录，仅释放已支付矿工费或无需矿工费的记录；未支付矿工费记录单独计数，不占用释放批次额度；释放事务内将 `wallet_accounts.locked` 转入 `available`，更新锁仓与解禁记录状态，并写入两条 `wallet_ledger`；防御 cancelled、user/asset mismatch、非正数 unlock_quantity 和 stale update；新币申购/上市后认购锁仓时同步创建 `asset_unlock_records`，让生产扫描器有可释放记录。
- 修改文件：
  - `src/workers/unlock_scanner.rs`
  - `tests/unlock_scanner.rs`
  - `src/main.rs`
  - `src/config.rs`
  - `.env.example`
  - `src/modules/new_coin/routes.rs`
  - `tests/new_coin_routes.rs`
  - 多个包含 `Settings` 字面量的 `src/` 与 `tests/` 文件
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test unlock_scanner -- --nocapture`，6 个测试通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test new_coin_routes -- --nocapture`，8 个测试通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib config::tests -- --nocapture`，2 个测试通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试通过。
- 后续事项：继续推进下一个后端业务切片，优先候选为 K-line recovery worker、秒合约自动结算 worker、理财自动赎回 worker或杠杆平仓/强平。

## 2026-05-29 11:19 - K-line Recovery Worker 生产补偿循环

- 完成内容：补齐生产可运行的 K-line recovery worker；扫描 `strategy_runs` / `market_strategies` / `trading_pairs` 中 due 的 active 策略运行；只补偿已闭合的 1m K 线；按交易对写入 MongoDB `market_klines_<symbol>` collection 并通过 `(interval, open_time)` upsert 保持幂等；成功后更新 `strategy_runs.current_price`、`last_generated_at`、`last_kline_open_time` 与 `recovery_status`；单个策略每轮最多补偿 500 根 K 线；checkpoint 对齐到 interval 边界，并发下 checkpoint 已被推进时按 skipped 处理；新增配置、环境变量和启动调度。
- 修改文件：
  - `src/workers/kline_recovery.rs`
  - `tests/kline_recovery.rs`
  - `src/config.rs`
  - `.env.example`
  - `src/main.rs`
  - 多个包含 `Settings` 字面量的 `src/` 与 `tests/` 文件
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib workers::kline_recovery -- --nocapture`，7 个测试通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" MONGODB_URI="mongodb://exchange:exchange@127.0.0.1:27017" MONGODB_DATABASE="exchange_market" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test kline_recovery -- --nocapture`，1 个测试通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" MONGODB_URI="mongodb://exchange:exchange@127.0.0.1:27017" MONGODB_DATABASE="exchange_market" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试通过；最终 code review 未发现 blocker。
- 后续事项：继续下一个后端业务切片，优先候选为秒合约自动结算 worker、理财自动赎回 worker、杠杆平仓/强平。

## 2026-05-29 13:04 - 秒合约自动结算 Worker

- 完成内容：补齐生产可运行的秒合约自动结算 worker；按配置启动定时循环，扫描到期 `opened` 订单，使用 Redis 最新 ticker 判定 `up/down` 胜负，相等价格按 loss 处理；胜利订单返还本金与收益并写入 `wallet_ledger`；缺失、非正数、陈旧 ticker、缺失 entry price、非法方向或持久性结算失败会写入 `next_settlement_attempt_at` 延后重试，避免坏单卡住后续健康订单；用户开仓时记录 `entry_price` 并校验入场 ticker 新鲜度。
- 修改文件：
  - `src/workers/seconds_contract_settlement.rs`
  - `tests/seconds_contract_settlement_worker.rs`
  - `migrations/0024_seconds_contract_entry_price.sql`
  - `migrations/0025_seconds_contract_settlement_retry_at.sql`
  - `src/modules/seconds_contract/routes.rs`
  - `src/workers/mod.rs`
  - `src/config.rs`
  - `.env.example`
  - `src/main.rs`
  - 多个包含 `Settings` 字面量的 `src/` 与 `tests/` 文件
- 验证结果：已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test seconds_contract_settlement_worker -- --nocapture`，7 个测试通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test seconds_contract_routes -- --nocapture`，16 个测试通过；已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`、`cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`、`cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，均通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试通过；最终 code review 确认最近两个 important finding 无剩余 blocker/important。
- 后续事项：立即处理用户最新要求：全项目涉及时间的字段、接口、缓存和测试统一迁移为时间戳语义。

## 2026-05-29 13:54 - 全项目外部时间字段时间戳迁移

- 完成内容：按用户要求将外部 API、Redis cache payload、事件 payload 和相关测试中的时间字段迁移为 Unix 毫秒时间戳；新增共享 `time` serde 边界；市场 ticker/K 线、闪兑 quote TTL、秒合约 ticker、新币/后台/代理响应、审计 JSON、领域事件等对外 JSON 时间值统一输出或接收 number；保留内部运算与数据库 `DateTime<Utc>` / `TIMESTAMP(6)` 边界，避免破坏已应用 migration。
- 修改文件：
  - `src/lib.rs`
  - `src/time.rs`
  - `src/modules/spot/routes.rs`
  - `src/modules/market/mod.rs`
  - `src/modules/market/routes.rs`
  - `src/modules/convert/mod.rs`
  - `src/modules/convert/routes.rs`
  - `src/modules/events/mod.rs`
  - `src/modules/new_coin/routes.rs`
  - `src/modules/admin/routes.rs`
  - `src/modules/seconds_contract/routes.rs`
  - `src/workers/seconds_contract_settlement.rs`
  - `tests/seconds_contract_routes.rs`
  - `tests/seconds_contract_settlement_worker.rs`
  - `tests/market_routes.rs`
  - `tests/convert_repositories.rs`
  - `tests/admin_routes.rs`
- 验证结果：已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib time -- --nocapture`，11 个相关测试通过；已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`、`cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`、`cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，均通过；已执行 focused tests：`market_routes` 8 个通过、`seconds_contract_routes` 16 个通过、`seconds_contract_settlement_worker` 7 个通过、`convert_repositories` 2 个通过、`admin_routes` 23 个通过、`new_coin_routes` 8 个通过、`market_redis_cache` 1 个通过、`market_ingestion` 1 个通过、`convert_routes` 5 个通过、`market_adapters` 4 个通过、`events_outbox` 9 个通过；已重新执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试输出末尾显示所有 test result 均为 ok，最终 doc-tests 0 failed。
- 后续事项：继续后端未完成业务切片：理财自动赎回 worker 与杠杆强平 worker。

## 2026-05-29 14:10 - 理财自动赎回 Worker

- 完成内容：新增生产可运行的理财自动赎回 worker；按配置启动定时循环，扫描到期 `earn_subscriptions.status = 'subscribed'` 订单，按本金、APR 和期限计算收益，在同一事务内更新用户可用余额、写入 `wallet_ledger.change_type = 'earn_redeem'`、标记订阅为 `redeemed` 并写入 `redeemed_at`；单条异常不会阻塞后续到期订单，批量限制会按已成功赎回数量停止。
- 修改文件：
  - `src/workers/earn_auto_redemption.rs`
  - `tests/earn_auto_redemption_worker.rs`
  - `src/workers/mod.rs`
  - `src/config.rs`
  - `.env.example`
  - `src/main.rs`
  - 多个包含 `Settings` 字面量的 `src/` 与 `tests/` 文件
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已先执行新测试 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test earn_auto_redemption_worker -- --nocapture`，初始失败于缺少 `workers::earn_auto_redemption`；实现后重新执行同命令，3 个测试通过；已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib config -- --nocapture`，2 个测试通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test earn_routes -- --nocapture`，15 个测试通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试输出末尾显示所有 test result 均为 ok，最终 doc-tests 0 failed。
- 后续事项：继续实现杠杆强平 worker。

## 2026-05-29 14:47 - 杠杆强平 Worker

- 完成内容：新增生产可运行的杠杆强平 worker；按配置启动定时循环，扫描待检查的 `margin_positions.status = 'opened'` 仓位，读取 Redis 最新 ticker 作为标记价，按 long/short 方向、开仓价、名义金额、保证金和维持保证金率计算权益与强平阈值；达到强平条件时在同一事务内返还 `max(equity, 0)` 到用户可用余额、写入 `wallet_ledger.change_type = 'margin_position_liquidate'`、更新仓位为 `liquidated` 并记录退出价、已实现盈亏、强平时间和原因；缺失/陈旧 ticker 或坏数据会延后重试且不阻塞后续健康仓位；安全仓位只短暂延后 5 秒并按 `next_liquidation_attempt_at` 优先排序，避免安全老仓位永久占据扫描窗口，也避免 60 秒强平盲区；用户开仓时记录 Redis ticker `entry_price`。
- 修改文件：
  - `migrations/0026_margin_liquidation_fields.sql`
  - `src/workers/margin_liquidation.rs`
  - `tests/margin_liquidation_worker.rs`
  - `src/workers/mod.rs`
  - `src/modules/margin/routes.rs`
  - `src/config.rs`
  - `.env.example`
  - `src/main.rs`
  - 多个包含 `Settings` 字面量的 `src/` 与 `tests/` 文件
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已先执行新测试 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test margin_liquidation_worker -- --nocapture`，初始失败于缺少 `workers::margin_liquidation`；代码复核发现安全仓位长延后和扫描窗口饿死风险后，新增安全仓位短周期轮转与越过安全仓位处理后续危险仓位的回归测试，修复前失败、修复后通过；最终已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test margin_liquidation_worker -- --nocapture`，5 个测试通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test margin_routes -- --nocapture`，12 个测试通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib config -- --nocapture`，2 个测试通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试输出末尾显示所有 test result 均为 ok，最终 doc-tests 0 failed。
- 后续事项：继续推进杠杆利息/借贷、风控、代理佣金、事件推送和剩余交易所业务模块。

## 2026-05-29 15:35 - 杠杆手动平仓接口

- 完成内容：新增用户侧 `POST /api/v1/margin/positions/:id/close` 手动平仓接口；仅允许仓位所属用户操作并对非本人仓位返回 404；平仓时锁定仓位与钱包，读取 Redis 最新 ticker 作为退出价，按 long/short、开仓价和名义金额计算已实现盈亏，返还 `max(margin_amount + realized_pnl, 0)` 到用户可用余额，写入 `wallet_ledger.change_type = 'margin_position_close'`，并更新 `margin_positions.status = 'closed'`、`closed_at`、`exit_price`、`realized_pnl` 与清空下次强平检查时间；重复平仓返回既有已关闭仓位，不重复返钱或写流水；仓位响应补充 entry/exit price、realized_pnl、closed_at，closed_at 对外为 Unix 毫秒时间戳。
- 修改文件：
  - `src/modules/margin/routes.rs`
  - `tests/margin_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test margin_routes margin_close_position_settles_realized_pnl_and_is_idempotent -- --nocapture`，实现前 `/margin/positions/:id/close` 无路由返回空 body，测试解析失败，符合预期；实现后已执行同 focused 测试通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test margin_routes margin_close_position_hides_other_users_position -- --nocapture`，通过；最终已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`、`cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`、`cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`、`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test margin_routes -- --nocapture`，14 个测试通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试输出末尾显示所有 test result 均为 ok，最终 doc-tests 0 failed。
- 后续事项：继续推进杠杆仓位列表/详情、利息/借贷、风控快照、代理佣金和事件推送。

## 2026-05-29 15:53 - 杠杆仓位列表详情接口

- 完成内容：新增用户侧 `GET /api/v1/margin/positions` 和 `GET /api/v1/margin/positions/:id` 查询接口；列表按当前登录用户强制过滤仓位，支持 `status=opened|closed|liquidated` 可选过滤和 limit 限制；详情接口仅返回当前用户自己的仓位，对非本人仓位返回 404；响应复用仓位字段并包含 entry_price、exit_price、realized_pnl，closed_at 对外序列化为 Unix 毫秒时间戳。
- 修改文件：
  - `src/modules/margin/routes.rs`
  - `tests/margin_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test margin_routes margin_position_queries_return_only_authenticated_user_positions -- --nocapture`，实现前 `GET /margin/positions` 与 `GET /margin/positions/:id` 无路由返回空 body，测试解析失败，符合预期；实现后已执行同 focused 测试通过；最终已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`、`cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`、`cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`、`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test margin_routes -- --nocapture`，15 个测试通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试输出末尾显示所有 test result 均为 ok，最终 doc-tests 0 failed。
- 后续事项：继续推进杠杆利息/借贷、风控快照、代理佣金和事件推送。

## 2026-05-29 16:03 - 杠杆强平记录表

- 完成内容：新增 `margin_liquidation_records` 强平快照表，记录每次强平时的仓位、用户、产品、交易对、保证金币种、方向、保证金、名义金额、入场价、强平标记价、维持保证金率、权益、维持保证金、已实现盈亏、返还金额、原因和强平时间；杠杆强平 worker 在同一事务内完成钱包返还、流水写入、强平记录写入和仓位状态更新，强平记录按 `position_id` 唯一保证重放不重复。
- 修改文件：
  - `migrations/0027_margin_liquidation_records.sql`
  - `src/workers/margin_liquidation.rs`
  - `tests/margin_liquidation_worker.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test margin_liquidation_worker margin_liquidation_worker_liquidates_unsafe_position_idempotently -- --nocapture`，实现前失败于 `Table 'exchange.margin_liquidation_records' doesn't exist`，符合缺失记录表预期；实现后同 focused 测试通过；首次最终验证中 `cargo clippy --all-targets --all-features -- -D warnings` 发现测试 tuple 类型复杂度，已改为 `LiquidationRecordRow`；最终已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`、`cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`、`cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`、`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test margin_liquidation_worker -- --nocapture`，5 个测试通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试输出末尾显示所有 test result 均为 ok，最终 doc-tests 0 failed。
- 后续事项：继续推进杠杆利息/借贷、风控快照、代理佣金和事件推送。

## 2026-05-29 16:36 - 后台强平记录查询接口

- 完成内容：新增后台 `GET /admin/api/v1/margin/liquidations` 强平记录列表接口，要求 `AdminAuth` 与 MySQL；支持按 `user_id`、`pair_id`、`position_id` 和夹紧后的 `limit` 查询；返回强平快照完整字段，`liquidated_at` 与 `created_at` 对外为 Unix 毫秒时间戳。
- 修改文件：
  - `src/modules/admin/routes.rs`
  - `tests/admin_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_margin_liquidations_list_filters_seeded_records -- --nocapture`，实现前 `/admin/api/v1/margin/liquidations` 无路由导致空 body 解析失败，符合预期；实现后 focused 测试通过。首次最终验证 `cargo fmt --check` 发现 `tests/admin_routes.rs` 格式问题，已执行 `cargo fmt` 修复。最终已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`、`cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`、`cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`、`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_margin_liquidation -- --nocapture`，2 个 focused 测试通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes -- --nocapture`，25 个测试通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试输出末尾显示所有 test result 均为 ok，最终 doc-tests 0 failed。最终代码复核未发现 blocker 或 important 问题。
- 后续事项：继续推进杠杆利息/借贷、风控快照、代理佣金和事件推送。

## 2026-05-29 16:49 - 用户杠杆仓位风险快照接口

- 完成内容：新增用户侧 `GET /api/v1/margin/positions/:id/risk` 风险快照接口；仅允许当前用户查询自己的 opened 仓位，非本人返回 404，已关闭仓位或缺失入场价返回 validation；读取 Redis 最新 ticker 并复用强平 worker 的 `margin_liquidation_risk_state` 公式，返回 mark price、realized PnL、equity、maintenance margin、是否触发强平和 ticker `observed_at` Unix 毫秒时间戳。
- 修改文件：
  - `src/modules/margin/routes.rs`
  - `tests/margin_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test margin_routes margin_position_risk_snapshot_returns_owned_position_metrics -- --nocapture`，实现前 `/margin/positions/:id/risk` 无路由导致空 body 解析失败，符合预期；实现后同 focused 测试通过。已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，首次发现格式 diff 后已执行 `cargo fmt` 修复并复查通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test margin_routes margin_position_risk_snapshot -- --nocapture`，3 个 focused 测试通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test margin_routes -- --nocapture`，18 个测试通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试输出末尾显示所有 test result 均为 ok，最终 doc-tests 0 failed。最终代码复核未发现 blocker 或 important 问题。
- 后续事项：继续推进杠杆利息/借贷、代理佣金和事件推送。

## 2026-05-29 17:12 - 闪兑确认生成代理佣金

- 完成内容：新增用户闪兑确认结算时的代理佣金生成逻辑；当确认闪兑的用户存在 `user_referrals.root_agent_id`，且该代理存在 active 的 `agent_commission_rules.product_type = 'convert'` 规则时，在同一 MySQL transaction 内生成 `agent_commission_records`，记录 `source_type = 'convert_order'`、`source_amount = from_amount`、`commission_amount = source_amount * commission_rate`、`status = 'pending'`；佣金写入与订单完成、钱包扣减/入账、wallet ledger 写入保持同事务，失败整体回滚。
- 修改文件：
  - `src/modules/convert/routes.rs`
  - `tests/convert_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test convert_routes convert_confirm_creates_pending_agent_commission_for_referred_user -- --nocapture`，实现前断言佣金记录数为 1 但实际为 0，符合缺失佣金生成逻辑预期；实现后同 focused 测试通过。已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test convert_routes -- --nocapture`，6 个测试通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试输出末尾显示所有 test result 均为 ok，最终 doc-tests 0 failed。最终代码复核未发现 blocker 或 important 问题。
- 后续事项：继续推进代理佣金幂等/结算状态管理、杠杆利息/借贷和事件推送。

## 2026-05-29 17:39 - 后台代理佣金状态更新接口

- 完成内容：新增后台 `PATCH /admin/api/v1/agent-commissions/:id/status` 接口；接口要求 `AdminAuth` 与 MySQL，只接受 `settled` / `rejected`，并通过 `SELECT ... FOR UPDATE` 锁定 `agent_commission_records` 后仅允许从 `pending` 状态更新；更新后返回 `AdminAgentCommissionResponse`，并在同一 MySQL transaction 内写入 `admin_audit_logs`，审计记录 `action = 'agent_commission.status.update'`、`target_type = 'agent_commission'`、before/after 状态和 reason；重复更新非 pending 佣金返回 conflict，避免重复结算或覆盖。
- 修改文件：
  - `src/modules/admin/routes.rs`
  - `tests/admin_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_agent_commission_status -- --nocapture`，实现前缺少路由导致未认证请求返回 404、成功路径 body 为空解析失败，符合缺失接口预期；实现后同 focused 测试 2 个通过。已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes -- --nocapture`，27 个测试通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试输出末尾显示所有 test result 均为 ok，最终 doc-tests 0 failed。最终代码复核未发现 blocker 或 important 问题。
- 后续事项：继续推进代理佣金幂等键/source_id、杠杆利息/借贷和事件推送。

## 2026-05-29 17:55 - 代理佣金来源幂等键

- 完成内容：新增 append-only migration `0028_agent_commission_source_id.sql`，为 `agent_commission_records` 增加 `source_id`，对历史数据回填 `legacy:<id>`，并添加 `(agent_id, source_type, source_id)` 唯一键；闪兑确认生成代理佣金时将 `quote_id` 写入 `source_id`，并使用 MySQL duplicate key 兜底避免同一代理、同一来源类型、同一来源 ID 重复生成佣金记录；同步更新后台/代理测试夹具插入佣金记录时生成唯一 `source_id`。
- 修改文件：
  - `migrations/0028_agent_commission_source_id.sql`
  - `src/modules/convert/routes.rs`
  - `tests/convert_routes.rs`
  - `tests/admin_routes.rs`
  - `tests/agent_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test convert_routes convert_confirm_creates_pending_agent_commission_for_referred_user -- --nocapture`，实现前失败于 `Unknown column 'source_id' in 'field list'`，符合缺失来源 ID 字段预期；实现后同 focused 测试通过，并断言 `source_id == quote_id`。已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_agent_commission_status -- --nocapture`，2 个测试通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test agent_routes agent_commission -- --nocapture`，1 个测试通过；已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`、`cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`、`cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，均通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试输出末尾显示所有 test result 均为 ok，最终 doc-tests 0 failed。最终代码复核未发现 blocker 或 important 问题。
- 后续事项：继续推进杠杆利息/借贷、事件推送和代理佣金结算出账。

## 2026-05-29 18:20 - 代理佣金结算出账

- 完成内容：后台代理佣金从 `pending` 更新为 `settled` 时，若来源为 `convert_order`，在同一 MySQL transaction 内通过 `source_id -> convert_orders.quote_id` 推导闪兑来源资产，将 `commission_amount` 入账到代理 owner 用户对应资产的 `wallet_accounts.available`，并写入 `wallet_ledger.change_type = 'agent_commission_payout'`、`ref_type = 'agent_commission'`、`ref_id = commission_id`；`rejected` 保持只更新状态与审计；重复状态更新仍由 pending-only 校验返回 conflict，避免重复出账。
- 修改文件：
  - `src/modules/admin/routes.rs`
  - `tests/admin_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_agent_commission_status -- --nocapture`，实现前失败于代理 owner 钱包 `available` 仍为 `1.000000000000000000`、期望 `6.000000000000000000`，符合缺失佣金出账预期；实现后同 focused 测试 2 个通过。已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，首次发现测试文件格式 diff，已执行 `cargo fmt` 修复并复查通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，首次发现测试 helper 参数过多，已改为 `AgentCommissionSeed` 后复查通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes -- --nocapture`，27 个测试通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试输出末尾显示所有 test result 均为 ok，最终 doc-tests 0 failed。
- 后续事项：继续推进杠杆利息/借贷、事件推送和代理佣金结算查询扩展。

## 2026-05-29 18:47 - 代理佣金结算查询扩展

- 完成内容：扩展代理侧 `GET /agent/api/v1/commissions` 返回字段；在保持 `records.agent_id` 与 `user_referrals.root_agent_id` 双重过滤的基础上，返回佣金 `source_id`，并对已结算佣金通过代理 owner 用户的钱包流水关联 `wallet_ledger.change_type = 'agent_commission_payout'`、`ref_type = 'agent_commission'`、`ref_id = commission_id`，展示 `payout_ledger_id`、`payout_asset_id`、`payout_amount`、`payout_balance_after` 和 `payout_created_at`；pending 佣金对应出账字段保持 null，`payout_created_at` 对外为 Unix 毫秒时间戳。
- 修改文件：
  - `src/modules/agent/routes.rs`
  - `tests/agent_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test agent_routes agent_commissions_only_return_authenticated_agent_team_records -- --nocapture`，实现前失败于 `records[0]["source_id"]` 为 null、期望 seeded source id，符合代理佣金列表未暴露来源与出账字段预期；实现中首次同 focused 测试因 MySQL collation 在 `payout.ref_id = CAST(records.id AS CHAR)` 比较时报错，已改为 `CAST(payout.ref_id AS UNSIGNED) = records.id` 后复查通过。已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test agent_routes -- --nocapture`，8 个测试通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试输出末尾显示所有 test result 均为 ok，最终 doc-tests 0 failed。
- 后续事项：继续推进杠杆利息/借贷、事件推送和代理佣金后台/代理查询细化。

## 2026-05-29 18:57 - 私有 WebSocket 用户事件推送

- 完成内容：补齐 `/ws/private?token=<user token>` 私有事件订阅链路；新增 `WebSocketChannel::private_user(user_id)` 与 `EventBroadcastMessage::private_user(user_id, payload)`，私有频道文本格式为 `private:user:<id>`；私有 WS 在通过 `PrivateWsAuth` 校验用户 token 后订阅 `EventBroadcastHub` 对应用户频道，只向当前连接转发精确匹配用户的私有广播，其他用户私有消息会被过滤；保留原有订阅确认和 ping/pong 行为，public WS 行为不变。
- 修改文件：
  - `src/modules/events/mod.rs`
  - `src/modules/events/routes.rs`
  - `tests/events_ws.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test events_ws private_ws_receives_only_authenticated_user_broadcasts -- --nocapture`，实现前编译失败于 `EventBroadcastMessage::private_user` 不存在，符合私有广播构造与订阅缺失预期；实现后同 focused 测试通过。首次 `cargo fmt --check` 发现 `src/modules/events/routes.rs` 和 `tests/events_ws.rs` 格式 diff，已执行 `cargo fmt` 修复并复查通过。最终已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test events_ws -- --nocapture`，9 个测试通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试输出末尾显示所有 test result 均为 ok，最终 doc-tests 0 failed。已调用 `superpowers:code-reviewer` 复核最近两个切片，未发现 blocker/important 问题。
- 后续事项：继续推进私有事件生产端接入、杠杆利息/借贷、代理佣金后台/代理查询细化。

## 2026-05-29 19:09 - 闪兑确认私有事件发布

- 完成内容：在用户成功 `POST /api/v1/convert/confirm` 后，于闪兑订单、钱包结算和代理佣金事务提交成功之后，通过 `EventBroadcastHub` 向当前用户的 `private:user:<id>` 频道发布 `convert.confirmed` 私有事件；事件 payload 包含 `type`、`quote_id` 和 `status = "completed"`；未配置 hub 时跳过发布，不影响闪兑结算原子性。
- 修改文件：
  - `src/modules/convert/routes.rs`
  - `tests/convert_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test convert_routes convert_confirm_settles_wallet_balances_and_marks_order_completed -- --nocapture`，实现前失败于 `Internal("event broadcast channel is closed")`，符合确认成功后未发布私有事件预期；实现后同 focused 测试通过。已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test convert_routes -- --nocapture`，6 个测试通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试通过，最终 doc-tests 0 failed。
- 后续事项：继续推进杠杆利息/借贷、更多业务私有事件生产端接入和代理佣金查询细化。

## 2026-05-29 19:19 - 现货下单私有事件发布

- 完成内容：在用户成功 `POST /api/v1/spot/orders` 创建现货订单并完成钱包冻结事务后，通过 `EventBroadcastHub` 向当前用户的 `private:user:<id>` 频道发布 `spot.order.created` 私有事件；事件 payload 包含 `type`、`order_id`、`pair_id`、`side`、`order_type` 和 `status`；未配置 hub 时跳过发布，不影响现货下单与钱包冻结事务原子性。
- 修改文件：
  - `src/modules/spot/routes.rs`
  - `tests/spot_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test spot_routes spot_create_limit_buy_order_freezes_quote_wallet -- --nocapture`，实现前失败于 `Internal("event broadcast channel is closed")`，符合下单成功后未发布私有事件预期；实现后同 focused 测试通过。已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test spot_routes -- --nocapture`，29 个测试通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试通过，最终 doc-tests 0 failed。
- 后续事项：继续推进现货撤单/成交、理财、杠杆和新币等更多业务私有事件生产端接入，以及杠杆利息/借贷。

## 2026-05-29 19:46 - 现货下单私有事件幂等修正

- 完成内容：根据代码复核修正现货下单私有事件幂等边界；`insert_order_and_freeze_wallet` 保留订单是否新插入的结果，只有真实新订单且钱包冻结已执行时才发布 `spot.order.created`，并发重复 `idempotency_key` replay 不再重复推送事件；同一用户复用相同幂等键但请求核心字段不同（交易对、方向、订单类型、价格、数量、冻结金额、请求 price、market reference_price）时返回 conflict，避免错误复用历史订单；补充数字交易对 ID replay 兼容，避免已入库 canonical symbol 与原始数字 `pair_id` 比较导致相同请求被误判为 conflict。
- 修改文件：
  - `src/modules/spot/routes.rs`
  - `tests/spot_routes.rs`
  - `migrations/0029_spot_order_request_reference_price.sql`
  - `migrations/0030_spot_order_request_price.sql`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test spot_routes spot_create_order_idempotency_key_rejects_mismatched_replay_request -- --nocapture`，实现前同 key 不同数量 replay 返回 200、期望 409，符合缺失 mismatch 检测预期；已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test spot_routes spot_create_order_idempotency_key_accepts_numeric_pair_id_replay -- --nocapture`，实现前数字 `pair_id` 相同请求 replay 返回 409、期望 200，符合交易对规范化缺失预期；已执行 RED：`spot_create_market_sell_idempotency_rejects_changed_reference_price`，实现前 market sell 同 key 改 reference_price 仍返回 200、期望 409；已执行 RED：`spot_create_market_order_idempotency_accepts_same_unused_price_replay`，实现前 market 单携带相同 request price replay 返回 409、期望 200；已执行 RED：`spot_create_market_order_idempotency_rejects_changed_unused_price`，实现前 market 单同 key 改 request price 仍返回 200、期望 409；实现后上述 focused 测试通过。已执行并发同 key 回归测试 `spot_create_order_concurrent_idempotency_key_freezes_once`，确认只冻结一次且只收到一条 `spot.order.created` 私有事件；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" sqlx migrate run --source "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/migrations"`，追加迁移 `0029` 与 `0030` 已应用；已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test spot_routes -- --nocapture`，34 个测试通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试通过，最终 doc-tests 0 failed。
- 后续事项：继续推进现货撤单/成交、理财、杠杆和新币等更多业务私有事件生产端接入，以及杠杆利息/借贷。

## 2026-05-29 20:16 - 现货下单幂等遗留兼容修正

- 完成内容：根据最终代码复核继续修正现货下单幂等 replay 边界；同一 `idempotency_key` 的交易对 symbol 大小写别名 replay 不再误判 conflict；迁移前旧订单 `request_price` / `request_reference_price` 为 NULL 时采用保守兼容：限价单回退比较持久化 `orders.price`，market buy 仅在 `reserved_amount` 已证明同一 `reference_price * quantity` 时允许 replay，market sell 因冻结金额无法证明 reference_price 一致而继续拒绝变更；legacy market replay 也拒绝新增或变更 unused `price`，避免绕过新指纹字段。
- 修改文件：
  - `src/modules/spot/routes.rs`
  - `tests/spot_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：新增 `spot_create_order_idempotency_key_accepts_case_alias_replay`，实现前 replay 返回 409、期望 200；新增 legacy NULL 指纹测试，`spot_create_limit_order_idempotency_accepts_legacy_null_request_price` 和 `spot_create_market_order_idempotency_accepts_legacy_null_reference_price` 实现前均返回 409、期望 200；代码复核发现 legacy market sell 与 unused price 风险后，新增 `spot_create_legacy_market_sell_idempotency_rejects_changed_reference_price` 和 `spot_create_legacy_market_order_idempotency_rejects_added_unused_price`，实现前均返回 200、期望 409。修复后 focused tests 均通过。已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test spot_routes -- --nocapture`，39 个测试通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试输出所有 test result 均为 ok，最终 doc-tests 0 failed；最终 code-reviewer 复审未发现 blocker/important 问题。
- 后续事项：继续推进现货撤单/成交、理财、杠杆和新币等更多业务私有事件生产端接入，以及杠杆利息/借贷。

## 2026-05-29 21:52 - 现货撤单与成交私有事件发布

- 完成内容：在用户成功撤销现货订单且实际发生状态变更后，于钱包解冻事务提交后向 `private:user:<id>` 发布 `spot.order.cancelled` 私有事件；重复撤单返回幂等结果但不重复推送。后台撮合成交成功后，于成交、订单状态和钱包结算事务提交后向买卖双方分别发布 `spot.trade.filled` 私有事件，payload 包含成交 ID、订单 ID、对手订单 ID、交易对、买卖方向、价格、数量和订单状态；同一成交幂等键 replay 返回历史成交但不重复推送。
- 修改文件：
  - `src/modules/spot/routes.rs`
  - `tests/spot_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test spot_routes spot_cancel_is_idempotent_without_repeating_unfreeze -- --nocapture`，实现前因未收到 `spot.order.cancelled` 私有事件而 `Elapsed(())` 失败；修复后同 focused 测试通过。已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test spot_routes spot_fill_is_idempotent_for_repeated_request_key -- --nocapture`，实现前因未收到买卖双方 `spot.trade.filled` 私有事件而 `Elapsed(())` 失败；修复后同 focused 测试通过。已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，首次发现格式 diff，已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml"` 修复并复查通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test spot_routes -- --nocapture`，39 个测试通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试输出所有 test result 均为 ok，最终 doc-tests 0 failed。
- 后续事项：继续推进理财、杠杆、新币等更多业务私有事件生产端接入，以及杠杆利息/借贷。

## 2026-05-29 22:15 - 理财 Earn 订阅与赎回私有事件发布

- 完成内容：在用户成功创建理财订阅且钱包扣款事务提交后，向 `private:user:<id>` 发布 `earn.subscription.created` 私有事件，payload 包含订阅 ID、产品 ID、资产 ID、金额和状态；订阅幂等 replay 返回既有订阅但不重复推送。用户成功赎回到期理财订阅且钱包入账事务提交后，向同一私有频道发布 `earn.subscription.redeemed` 私有事件，payload 包含订阅 ID、产品 ID、资产 ID、本金、收益、赎回总额和状态；已赎回 replay 不重复推送。
- 修改文件：
  - `src/modules/earn/routes.rs`
  - `tests/earn_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test earn_routes earn_subscribe_debits_wallet_and_writes_ledger -- --nocapture`，实现前因未收到 `earn.subscription.created` 私有事件而 `Elapsed(())` 失败；修复后同 focused 测试通过。已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test earn_routes earn_redeem_matured_subscription_credits_principal_yield_and_writes_ledger -- --nocapture`，实现前因未收到 `earn.subscription.redeemed` 私有事件而 `Elapsed(())` 失败；修复后同 focused 测试通过。已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，首次发现格式 diff，已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml"` 修复并复查通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test earn_routes -- --nocapture`，15 个测试通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试输出所有 test result 均为 ok，最终 doc-tests 0 failed。已执行代码复核，未发现 blocker 或 important 问题；建议后续可补充订阅 replay 不重复事件的显式测试。
- 后续事项：继续推进杠杆、新币、秒合约等更多业务私有事件生产端接入，以及杠杆利息/借贷。

## 2026-05-30 03:22 - 杠杆开仓与平仓私有事件发布

- 完成内容：选择下一私有事件生产端切片为杠杆用户开仓/平仓；在用户成功新建杠杆仓位且钱包扣保证金事务提交后，向 `private:user:<id>` 发布 `margin.position.opened` 私有事件；在用户成功平仓且钱包结算事务提交后，向同一私有频道发布 `margin.position.closed` 私有事件；开仓幂等 replay 和已平仓 replay 返回既有结果但不重复推送。
- 修改文件：
  - `src/modules/margin/routes.rs`
  - `tests/margin_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test margin_routes margin_open_position_debits_wallet_and_writes_ledger -- --nocapture`，实现前因未收到 `margin.position.opened` 私有事件而 `Elapsed(())` 失败；修复后同 focused 测试通过。已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test margin_routes margin_close_position_settles_realized_pnl_and_is_idempotent -- --nocapture`，实现前因未收到 `margin.position.opened` 私有事件而 `Elapsed(())` 失败；修复后同 focused 测试通过并确认平仓 replay 不重复推送。已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test margin_routes -- --nocapture`，18 个测试通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试输出所有 test result 均为 ok，最终 doc-tests 0 failed。
- 后续事项：继续推进秒合约、新币、杠杆强平等更多业务私有事件生产端接入，以及杠杆利息/借贷。

## 2026-05-30 03:46 - 秒合约开单与结算私有事件发布

- 完成内容：选择下一私有事件生产端切片为秒合约用户开单/结算；在用户成功新建秒合约订单且钱包扣款事务提交后，向 `private:user:<id>` 发布 `seconds_contract.order.opened` 私有事件；在后台成功结算秒合约订单且钱包派彩事务提交后，向订单用户发布 `seconds_contract.order.settled` 私有事件；开单幂等 replay 与已结算 replay 返回既有结果但不重复推送。
- 修改文件：
  - `src/modules/seconds_contract/routes.rs`
  - `tests/seconds_contract_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test seconds_contract_routes seconds_contract_open_order_debits_wallet_and_writes_ledger -- --nocapture`，实现前因未收到 `seconds_contract.order.opened` 私有事件而 `Elapsed(())` 失败；修复后同 focused 测试通过。已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test seconds_contract_routes seconds_contract_settle_win_credits_payout_and_writes_ledger -- --nocapture`，实现前因未收到 `seconds_contract.order.settled` 私有事件而 `Elapsed(())` 失败；修复后同 focused 测试通过并确认结算 replay 不重复推送。已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，首次发现 `src/modules/seconds_contract/routes.rs` 与 `tests/seconds_contract_routes.rs` 格式 diff，已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml"` 修复并复查通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test seconds_contract_routes -- --nocapture`，16 个测试通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试输出所有 test result 均为 ok，最终 doc-tests 0 failed。
- 后续事项：继续推进新币、杠杆强平等更多业务私有事件生产端接入，以及杠杆利息/借贷。

## 2026-05-30 04:07 - 新币申购认购与解禁私有事件发布

- 完成内容：在用户发行期申购成功且钱包扣款/锁仓事务提交后，向 `private:user:<id>` 发布 `new_coin.subscription.created` 私有事件；在上市后认购成功且钱包扣款/锁仓事务提交后，发布 `new_coin.purchase.created` 私有事件；在用户解禁释放成功且钱包 locked 转 available 事务提交后，发布 `new_coin.unlock.released` 私有事件；已释放解禁 replay 返回 OK 但不重复推送。
- 修改文件：
  - `src/modules/new_coin/routes.rs`
  - `tests/new_coin_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test new_coin_routes new_coin_routes_release_due_paid_unlock_updates_wallet_and_lock_state -- --nocapture`，实现前因未收到 `new_coin.unlock.released` 私有事件而 `Elapsed(())` 失败；修复过程中确认 replay 不重复推送，最终同 focused 测试通过。已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test new_coin_routes new_coin_subscription_debits_quote_wallet_and_locks_fixed_time_allocation -- --nocapture`，实现前因未收到 `new_coin.subscription.created` 私有事件而 `Elapsed(())` 失败；修复后同 focused 测试通过。已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test new_coin_routes new_coin_purchase_debits_quote_wallet_and_locks_fixed_time_allocation -- --nocapture`，实现前因未收到 `new_coin.purchase.created` 私有事件而 `Elapsed(())` 失败；修复后同 focused 测试通过。已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，首次发现格式 diff，已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml"` 修复并复查通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test new_coin_routes -- --nocapture`，8 个测试通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试输出所有 test result 均为 ok，最终 doc-tests 0 failed；已执行 `superpowers:code-reviewer` 复核，未发现 blocker 或 important 问题。
- 后续事项：继续推进杠杆强平等更多业务私有事件生产端接入，以及杠杆利息/借贷。

## 2026-05-30 05:19 - 杠杆强平私有事件发布

- 完成内容：在杠杆强平 worker 成功将 unsafe 仓位更新为 `liquidated` 且钱包入账、流水、强平记录事务提交后，向 `private:user:<id>` 发布 `margin.position.liquidated` 私有事件；payload 包含仓位、产品、交易对、保证金资产、方向、保证金、名义金额、入场价、标记价、已实现盈亏、返还金额、强平原因和 Unix milliseconds 的 `liquidated_at`；重复扫描已强平仓位不会重复发布。生产启动路径已改为向强平 loop 传入 `AppState`，确保自动强平也能使用 `EventBroadcastHub`。
- 修改文件：
  - `src/workers/margin_liquidation.rs`
  - `src/main.rs`
  - `tests/margin_liquidation_worker.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test margin_liquidation_worker margin_liquidation_worker_liquidates_unsafe_position_idempotently -- --nocapture`，实现前因未收到 `margin.position.liquidated` 私有事件而 `Elapsed(())` 失败；修复后同 focused 测试通过，并确认重复扫描不重复推送。已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，首次发现 `src/main.rs` 格式 diff，已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml"` 修复并最终复查通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test margin_liquidation_worker -- --nocapture`，5 个测试通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，首次发现 `large_enum_variant`，改为 boxed event 后复查通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试输出所有 test result 均为 ok，最终 doc-tests 0 failed；已执行两轮 `superpowers:code-reviewer` 复核，第一轮指出生产 loop 未携带 hub，已修复，第二轮未发现 blocker 或 important 问题。
- 后续事项：继续推进杠杆利息/借贷等剩余交易所后端能力。

## 2026-05-30 06:09 - 杠杆借款本金与利息累计基础

- 完成内容：新增杠杆借款与利息累计基础能力；杠杆产品支持 `hourly_interest_rate`，创建时校验非负和小数精度；用户开仓时记录 `borrowed_amount = notional_amount - margin_amount`，初始化 `interest_amount` 和 `interest_accrued_at`，并在开仓响应与 `margin.position.opened` 私有事件中返回借款本金和利息；新增生产可运行的 `margin_interest` worker，扫描 opened 仓位，按完整小时累计 `borrowed_amount * hourly_interest_rate * elapsed_full_hours`，使用行锁和 `interest_accrued_at` 保证重复同一时间执行幂等，不直接改动钱包余额；新增配置、环境变量和启动调度；补充 `0032` 迁移回填既有 opened 仓位借款本金，避免已上线 `0031` 校验和变更。
- 修改文件：
  - `migrations/0031_margin_borrow_interest.sql`
  - `migrations/0032_margin_borrow_interest_backfill.sql`
  - `src/modules/margin/routes.rs`
  - `src/workers/margin_interest.rs`
  - `src/workers/mod.rs`
  - `src/config.rs`
  - `.env.example`
  - `src/main.rs`
  - `tests/margin_routes.rs`
  - `tests/margin_liquidation_worker.rs`
  - 多个包含 `Settings` 字面量的 `src/` 与 `tests/` 文件
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED/GREEN：`margin_open_position_debits_wallet_and_writes_ledger` 先失败于缺少 `hourly_interest_rate` 字段，修复后通过；`margin_interest_worker_accrues_elapsed_full_hours_idempotently` 先失败于 `todo!()`，实现后通过。最终已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test margin_routes margin_open_position_debits_wallet_and_writes_ledger -- --nocapture`，1 个通过；已执行同环境 `cargo test --test margin_liquidation_worker margin_interest_worker_accrues_elapsed_full_hours_idempotently -- --nocapture`，1 个通过；已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check && cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets && cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，全部通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试所有 test result 均为 ok，最终 doc-tests 0 failed；已执行 `superpowers:code-reviewer` 复核，第一轮指出既有 opened 仓位未回填借款本金，已追加 `0032` 回填迁移，第二轮返回 `[]`。
- 后续事项：继续推进杠杆利息对平仓/强平结算的影响、利息流水或费用收取策略，以及剩余交易所后端能力。

## 2026-05-30 08:50 - 杠杆利息平仓强平结算

- 完成内容：补齐杠杆利息结算闭环；用户手动平仓返还金额改为 `max(margin_amount + realized_pnl - interest_amount, 0)`；强平风险权益改为 `margin_amount + realized_pnl - interest_amount`，强平返还继续使用扣息后的非负权益；用户风险快照复用同一扣息公式并返回 `interest_amount`；平仓与强平私有事件增加利息金额，平仓事件同时返回实际返还金额；重复平仓和重复强平扫描保持不重复入账、不重复流水、不重复记录、不重复推送事件。
- 修改文件：
  - `src/modules/margin/routes.rs`
  - `src/workers/margin_liquidation.rs`
  - `tests/margin_routes.rs`
  - `tests/margin_liquidation_worker.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED/GREEN：`margin_close_position_settles_realized_pnl_and_is_idempotent` 先失败于平仓响应/事件缺少利息字段，修复后通过；`margin_liquidation_worker_liquidates_unsafe_position_idempotently` 先失败于强平事件缺少利息字段，修复后通过；已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test margin_routes margin_close_position_settles_realized_pnl_and_is_idempotent -- --nocapture`，1 个通过；已执行同环境 `cargo test --test margin_liquidation_worker margin_liquidation_worker_liquidates_unsafe_position_idempotently -- --nocapture`，1 个通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试所有 test result 均为 ok，最终 doc-tests 0 failed；已执行 `superpowers:code-reviewer` 复核，返回 `[]`。
- 后续事项：继续推进剩余交易所后端能力，优先检查杠杆利息结算后的后台/用户可观测性和剩余风控闭环。

## 2026-05-30 09:03 - 杠杆强平利息审计可见性

- 完成内容：补齐杠杆强平记录的利息审计字段；新增 append-only migration `0033`，为 `margin_liquidation_records` 增加非负 `interest_amount` 并对既有记录安全默认 0；强平 worker 写入强平记录时持久化仓位累计利息；后台强平记录列表返回 `interest_amount`，并在测试中确认强平记录的权益和返还金额使用扣息后的数值。
- 修改文件：
  - `migrations/0033_margin_liquidation_interest_amount.sql`
  - `src/workers/margin_liquidation.rs`
  - `src/modules/admin/routes.rs`
  - `tests/margin_liquidation_worker.rs`
  - `tests/admin_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`margin_liquidation_worker_liquidates_unsafe_position_idempotently` 和 `admin_margin_liquidations_list_filters_seeded_records` 均先失败于 `Unknown column 'interest_amount' in 'field list'`；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" sqlx migrate run --source "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/migrations"`，成功应用 `0033`；已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test margin_liquidation_worker -- --nocapture`，6 个通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_margin_liquidation -- --nocapture`，2 个通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试所有 test result 均为 ok，最终 doc-tests 0 failed；已执行 `superpowers:code-reviewer` 复核，返回 `[]`。
- 后续事项：继续推进剩余交易所后端能力，优先检查杠杆利息在平仓历史/后台仓位查询中的可观测性。

## 2026-05-30 09:33 - 后台杠杆仓位历史查询

- 完成内容：新增后台杠杆仓位历史列表接口 `GET /margin/positions`，要求 `AdminAuth`，支持按 `user_id`、`pair_id`、`status` 和 `limit` 过滤；响应返回 opened/closed/liquidated 仓位的借款本金、累计利息、平仓时间、强平时间和强平原因，其中 `closed_at`、`liquidated_at` 按外部边界统一输出 Unix milliseconds 或 null，便于后台运营排查杠杆利息结算后的仓位状态。
- 修改文件：
  - `src/modules/margin/routes.rs`
  - `tests/margin_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test margin_routes admin_margin_positions -- --nocapture`，实现前 `/margin/positions` 后台路由缺失，测试分别失败于 404 和空响应解析；实现后同 focused 测试 2 个通过。已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test margin_routes -- --nocapture`，20 个测试通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试所有 test result 均为 ok，最终 doc-tests 0 failed；已执行 `superpowers:code-reviewer` 复核，返回 `[]`。
- 后续事项：继续推进剩余交易所后端能力，优先检查杠杆利息费用归集/后台统计和业务私有事件范围记录等收尾项。

## 2026-05-30 09:57 - 后台杠杆利息汇总可见性

- 完成内容：新增后台杠杆利息汇总接口 `GET /margin/interest/summary`，要求 `AdminAuth`，支持按 `user_id`、`pair_id`、`status` 和 `limit` 过滤；按 `margin_asset + status` 聚合仓位数量、借款本金合计和累计利息合计，金额统一 18 位小数字符串输出，便于后台查看 opened/closed/liquidated 仓位的利息费用规模，不改变钱包结算和强平/平仓行为。
- 修改文件：
  - `src/modules/margin/routes.rs`
  - `tests/margin_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test margin_routes admin_margin_interest_summary -- --nocapture`，实现前 `/margin/interest/summary` 后台路由缺失，测试分别失败于 404 和空响应解析；实现后同 focused 测试 2 个通过。已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，首次发现 `tests/margin_routes.rs` 格式 diff，已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml"` 修复并复查通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test margin_routes -- --nocapture`，22 个测试通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试所有 test result 均为 ok，最终 doc-tests 0 failed；已执行 `superpowers:code-reviewer` 复核，返回 `[]`。
- 后续事项：继续推进剩余交易所后端能力，优先处理遗留业务私有事件范围记录和整体收尾检查。

## 2026-05-30 10:07 - Earn 自动赎回私有事件发布

- 完成内容：补齐 Earn 自动赎回 worker 的私有事件生产端；自动赎回到期订阅并完成钱包入账、流水写入和订阅状态事务提交后，向 `private:user:<id>` 发布 `earn.subscription.redeemed`，payload 包含订阅 ID、产品 ID、资产 ID、本金、收益、赎回总额和状态；生产启动改为向 worker 传入 `AppState`，确保自动 worker 可访问 `EventBroadcastHub`；幂等 replay 或已赎回记录不重复推送事件。
- 修改文件：
  - `src/workers/earn_auto_redemption.rs`
  - `src/main.rs`
  - `tests/earn_auto_redemption_worker.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test earn_auto_redemption_worker earn_auto_redemption_worker_redeems_matured_subscription_idempotently -- --nocapture`，实现前失败于 `run_once_with_broadcast` 未定义，符合自动赎回事件发布入口缺失预期；实现后 focused 测试通过，并断言自动赎回收到 `earn.subscription.redeemed` 且 replay 不重复推送。已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test earn_auto_redemption_worker -- --nocapture`，3 个测试通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试所有 test result 均为 ok，最终 doc-tests 0 failed；已执行 `superpowers:code-reviewer` 复核，未发现 blocker 或 important 问题。
- 后续事项：继续审计剩余自动 worker 私有事件一致性和整体后端收尾项。

## 2026-05-30 10:20 - 秒合约自动结算私有事件发布

- 完成内容：补齐秒合约自动结算 worker 的私有事件生产端；自动读取 Redis ticker 并完成到期订单结算、钱包派彩、流水写入和订单状态事务提交后，向订单用户 `private:user:<id>` 发布 `seconds_contract.order.settled`，payload 包含订单 ID、产品 ID、交易对 ID、押注资产、方向、押注金额、派彩金额、结果和状态；生产启动改为向 worker 传入 `AppState`，确保自动 worker 同时访问 MySQL、Redis 和 `EventBroadcastHub`；幂等 replay 或已结算记录不重复推送事件。
- 修改文件：
  - `src/workers/seconds_contract_settlement.rs`
  - `src/main.rs`
  - `tests/seconds_contract_settlement_worker.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test seconds_contract_settlement_worker seconds_contract_settlement_worker_settles_due_orders_from_cached_ticker_idempotently -- --nocapture`，实现前失败于 `Elapsed(())`，符合自动结算未推送私有事件预期；实现后同 focused 测试通过，并断言 replay 不重复推送。已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，首次发现格式 diff，已执行 `cargo fmt` 修复并复查通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test seconds_contract_settlement_worker -- --nocapture`，7 个测试通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试所有 test result 均为 ok，最终 doc-tests 0 failed；已执行 `superpowers:code-reviewer` 复核，未发现 blocker 或 important 问题。
- 后续事项：继续审计剩余自动 worker 私有事件一致性和整体后端收尾项。

## 2026-05-30 10:47 - 自动解禁扫描私有事件发布

- 完成内容：补齐自动解禁扫描 worker 的私有事件生产端；到期解禁记录完成钱包 locked 到 available 转移、锁定仓位和解禁记录状态事务提交后，向用户 `private:user:<id>` 发布 `new_coin.unlock.released`；payload 兼容手动解禁事件字段 `unlock_idempotency_key`、`unlock_quantity`、`released`，并保留自动扫描使用的 `unlock_id`、`lock_position_id`、`released_amount`、`status`；生产启动改为向 worker 传入 `AppState`，确保自动 worker 可访问 MySQL 和 `EventBroadcastHub`；幂等 replay、fee-blocked 和 skipped 记录不重复推送事件。
- 修改文件：
  - `src/workers/unlock_scanner.rs`
  - `src/main.rs`
  - `tests/unlock_scanner.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test unlock_scanner unlock_scanner_releases_due_paid_unlock_and_is_idempotent -- --nocapture`，实现前失败于 `expected &Pool<MySql>, found &AppState`，符合自动 scanner 未接入 `AppState` 和广播入口预期；实现后 focused 测试通过，并断言自动解禁收到 `new_coin.unlock.released` 且 replay 不重复推送。已执行 schema 兼容 RED，同 focused 测试先失败于 `unlock_idempotency_key` 为 `Null`，补齐兼容 payload 后通过。已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test unlock_scanner -- --nocapture`，6 个测试通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试所有 test result 均为 ok，最终 doc-tests 0 failed；已执行 `superpowers:code-reviewer` 复核，最终未发现 blocker 或 important 问题。
- 后续事项：继续审计剩余后端缺口，重点确认自动 worker 私有事件、后台可观测性和整体收尾项是否仍有遗漏。

## 2026-05-30 11:06 - Event Outbox 生产启动接入

- 完成内容：补齐 RabbitMQ outbox 发布 worker 的生产启动接入；新增 `EVENT_OUTBOX_PUBLISHER_ENABLED` 和 `EVENT_OUTBOX_PUBLISHER_INTERVAL_SECONDS` 配置，默认启用并每 5 秒扫描发布；生产启动在 MySQL 与 RabbitMQ 均可用时启动 `event_outbox::run_loop`，让已写入 `event_outbox` 的领域事件自动发布，不再只依赖后台手动 `publish-once`。
- 修改文件：
  - `src/config.rs`
  - `src/main.rs`
  - `.env.example`
  - 多个测试辅助 `Settings` 构造位置
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib config::tests::settings_from_env_parses_market_feed_lists -- --nocapture`，实现前失败于 `no field event_outbox_publisher_enabled` 和 `no field event_outbox_publisher_interval_seconds`，符合配置缺失预期；实现后 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib config::tests::settings_from_env -- --nocapture`，2 个测试通过。已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，首次发现新增 Settings 字段缩进 diff，已执行 `cargo fmt` 修复并复查通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试所有 test result 均为 ok，最终 doc-tests 0 failed；已执行 `superpowers:code-reviewer` 复核，未发现 blocker 或 important 问题。
- 后续事项：按用户最新要求，将“项目中所有涉及时间的都必须使用时间戳”写入需求/设计文档并验证不会被遗忘。

## 2026-05-30 11:16 - 全局时间戳需求文档固化

- 完成内容：将“项目中所有涉及时间的都必须使用时间戳”固化到拆分设计文档、总览文档、风控测试验收文档和单体设计文档；明确 REST API、WebSocket、RabbitMQ、Redis 和 MongoDB 对外时间字段统一使用 Unix milliseconds，Rust/MySQL 内部可使用 `DateTime<Utc>` / `TIMESTAMP(6)` 但跨边界必须转换；补充测试验收要求，并修正单体设计文档 4.x 小节编号。
- 修改文件：
  - `docs/superpowers/specs/blockchain-exchange/README.md`
  - `docs/superpowers/specs/blockchain-exchange/01-overview-architecture.md`
  - `docs/superpowers/specs/blockchain-exchange/06-security-risk-testing.md`
  - `docs/superpowers/specs/2026-05-26-blockchain-exchange-platform-design.md`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 `rg -n "时间戳|Unix milliseconds|DateTime<Utc>|TIMESTAMP\\(6\\)" ...`，确认 4 个目标文档均包含时间戳要求和内部/外部时间边界说明；已执行 `rg -n "^### 4\\.[0-9]+|^## [0-9]+\\." ...`，确认更新后的章节编号连续，单体文档 4.x 已从 4.1 至 4.7 顺序排列；已执行 `rg -n "TODO|TBD|FIXME|待定|占位" ...`，无输出，确认目标文档未新增占位内容。
- 后续事项：继续推进剩余后端缺口审计与整体收尾验证。

## 2026-05-30 11:31 - 用户资料接口补齐

- 完成内容：补齐一期 MVP 用户资料接口 `GET /api/v1/user/profile`；新增 `user` 模块并挂载到 `/api/v1`，接口要求 `UserAuth`，只按 JWT subject 查询当前用户本人，返回 `id`、`email`、`phone`、`status`、`kyc_level`、`created_at`；`created_at` 按全局时间边界序列化为 Unix milliseconds 数字，非 user scope token 被拒绝。
- 修改文件：
  - `src/modules/user/mod.rs`
  - `src/modules/user/routes.rs`
  - `src/modules/mod.rs`
  - `src/lib.rs`
  - `tests/user_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test user_routes user_profile_route -- --nocapture`，实现前 `/api/v1/user/profile` 返回 404，符合接口缺失预期；实现后同 focused 测试 2 个通过，并断言 `created_at` 为数字时间戳。已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，首次发现 `tests/user_routes.rs` 格式 diff，已执行 `cargo fmt` 修复并复查通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试所有 test result 均为 ok，最终 doc-tests 0 failed；已执行 `superpowers:code-reviewer` 复核，返回 `[]`。
- 后续事项：继续推进剩余后端缺口审计与整体收尾验证。

## 2026-05-30 12:10 - 用户推荐邀请接口补齐

- 完成内容：补齐用户端推荐邀请 MVP 接口 `GET /api/v1/referral/my-code`、`POST /api/v1/referral/bind`、`GET /api/v1/referral/my-invites`；接口统一要求 `UserAuth`，基于 `invite_codes` 与 `user_referrals` 实现用户邀请码生成、代理邀请码绑定、用户下级绑定、直属邀请列表查询；绑定时校验邀请码 active 状态、usage_limit、代理 active 状态，已绑定用户重复提交按现有绑定幂等返回且不重复增加 `used_count`；返回中的 `created_at` 均按 Unix milliseconds 数字序列化。
- 修改文件：
  - `src/modules/user/routes.rs`
  - `tests/user_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test user_routes user_referral -- --nocapture`，实现前 `/api/v1/referral/bind` 返回 404，符合接口缺失预期；实现后 focused `user_referral` 测试 2 个通过。代码评审发现 active invite code 未校验代理状态，已补充 RED：`user_referral_bind_rejects_disabled_agent_codes`，实现前返回 200、期望 400；修复后 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test user_routes user_referral -- --nocapture`，3 个 referral 测试通过。已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试所有 test result 均为 ok，`tests/user_routes.rs` 5 个通过，最终 doc-tests 0 failed；已执行 `superpowers:code-reviewer` 复核，返回 `[]`。
- 后续事项：继续推进剩余后端缺口审计与整体收尾验证。

## 2026-05-30 12:42 - 用户理财申购列表接口补齐

- 完成内容：在路由覆盖审计中确认用户端 Earn 已有理财产品列表、申购和赎回接口，但缺少当前用户理财申购/持仓记录查询；补齐 `GET /api/v1/earn/subscriptions`，复用 `UserAuth`，仅返回当前认证用户的 `earn_subscriptions`，按 `created_at DESC, id DESC` 排序并支持 `limit` 限制；响应中的 `matures_at` 继续按 Unix milliseconds 数字序列化。
- 修改文件：
  - `src/modules/earn/routes.rs`
  - `tests/earn_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test earn_routes earn_lists_current_user_subscriptions_with_timestamp -- --nocapture`，实现前 `GET /earn/subscriptions` 返回 405、期望 200，符合 GET handler 缺失预期；实现后同命令 1 个测试通过。已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test earn_routes -- --nocapture`，16 个 Earn route 测试通过。已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试所有 test result 均为 ok，`tests/earn_routes.rs` 16 个通过，最终 doc-tests 0 failed。
- 后续事项：继续推进剩余后端缺口审计与整体收尾验证。

## 2026-05-30 12:54 - 用户秒合约订单列表接口补齐

- 完成内容：在产品路由审计中确认秒合约已有用户产品列表、开单和后台结算接口，但用户端缺少订单历史查询；补齐 `GET /api/v1/seconds-contracts/orders`，复用 `UserAuth`，仅返回当前认证用户的 `seconds_contract_orders`，按 `created_at DESC, id DESC` 排序并支持 `limit` 限制；响应中的 `expires_at` 继续按 Unix milliseconds 数字序列化。
- 修改文件：
  - `src/modules/seconds_contract/routes.rs`
  - `tests/seconds_contract_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test seconds_contract_routes seconds_contract_lists_current_user_orders_with_timestamp -- --nocapture`，实现前 `GET /seconds-contracts/orders` 返回 405、期望 200，符合 GET handler 缺失预期；实现后同命令 1 个测试通过。已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test seconds_contract_routes -- --nocapture`，17 个 seconds contract route 测试通过。已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试所有 test result 均为 ok，`tests/seconds_contract_routes.rs` 17 个通过，最终 doc-tests 0 failed。
- 后续事项：继续推进剩余后端缺口审计与整体收尾验证。

## 2026-05-30 14:54 - 后台理财申购列表接口补齐

- 完成内容：在后台产品历史路由审计中确认 Earn 后台已有产品创建、列表和状态管理，但缺少后台理财申购/持仓记录查询；补齐 `GET /admin/api/v1/earn/subscriptions`，要求 `AdminAuth`，返回所有用户的 `earn_subscriptions`，按 `created_at DESC, id DESC` 排序，支持 `limit`、`user_id` 和 `status` 过滤；响应中的 `matures_at` 继续按 Unix milliseconds 数字序列化。
- 修改文件：
  - `src/modules/earn/routes.rs`
  - `tests/earn_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test earn_routes admin_earn_lists_subscriptions_with_filters_and_timestamp -- --nocapture`，实现前 `/earn/subscriptions` 后台路由返回 404、期望 200，符合后台申购列表接口缺失预期；实现后同 focused 测试通过。已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test earn_routes -- --nocapture`，17 个 Earn route 测试通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试所有 test result 均为 ok，`tests/earn_routes.rs` 17 个通过，最终 doc-tests 0 failed。
- 后续事项：继续推进剩余后端缺口审计，优先检查后台秒合约订单列表可见性。

## 2026-05-30 15:20 - 后台秒合约订单列表接口补齐

- 完成内容：在后台产品历史路由审计中确认秒合约后台已有产品创建、列表、状态管理和单笔结算接口，但缺少后台订单历史查询；补齐 `GET /admin/api/v1/seconds-contracts/orders`，要求 `AdminAuth`，返回所有用户的 `seconds_contract_orders`，按 `created_at DESC, id DESC` 排序，支持 `limit`、`user_id` 和 `status` 过滤；响应中的 `expires_at` 继续按 Unix milliseconds 数字序列化。
- 修改文件：
  - `src/modules/seconds_contract/routes.rs`
  - `tests/seconds_contract_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test seconds_contract_routes admin_seconds_contract_lists_orders_with_filters_and_timestamp -- --nocapture`，实现前 `/seconds-contracts/orders` 后台路由返回 404、期望 200，符合后台订单列表接口缺失预期；实现后同 focused 测试通过。已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test seconds_contract_routes -- --nocapture`，18 个 seconds contract route 测试通过；已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试所有 test result 均为 ok，`tests/seconds_contract_routes.rs` 18 个通过，最终 doc-tests 0 failed。
- 后续事项：继续推进剩余后端缺口审计和整体收尾验证。

## 2026-05-30 15:34 - 后台现货订单与成交历史接口补齐

- 完成内容：在后台产品历史路由审计中确认现货后台已有成交填充接口，但缺少后台订单和成交历史查询；补齐 `GET /admin/api/v1/spot/orders` 与 `GET /admin/api/v1/spot/trades`，要求 `AdminAuth`，订单支持 `limit`、`pair_id`、`status`、`user_id` 过滤，成交支持 `limit`、`pair_id`、`user_id` 参与方过滤，均按 `created_at DESC, id DESC` 排序；成交响应中的 `created_at` 继续按 Unix milliseconds 数字序列化。
- 修改文件：
  - `src/modules/spot/routes.rs`
  - `tests/spot_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test spot_routes admin_spot_lists_orders_and_trades_with_filters -- --nocapture`，实现前 `/spot/orders` 后台路由返回 404、期望 200，符合后台现货历史接口缺失预期；实现后同 focused 测试 1 个通过。已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test spot_routes -- --nocapture`，40 个 spot route 测试通过；已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试所有 test result 均为 ok，`tests/spot_routes.rs` 40 个通过，最终 doc-tests 0 failed。
- 后续事项：继续推进剩余后端缺口审计和整体收尾验证。

## 2026-05-30 15:56 - 后台行情策略接口补齐

- 完成内容：在剩余后端缺口审计中确认设计文档要求后台可管理 internal/strategy 行情策略，但现有后台未暴露 `market_strategies` 配置接口；补齐 `GET /admin/api/v1/market-strategies`、`POST /admin/api/v1/market-strategies`、`PATCH /admin/api/v1/market-strategies/{id}/status`，要求 `AdminAuth`，创建时仅允许绑定 active 的 internal/strategy 交易对，写入 `market_strategies`、初始 `strategy_versions`、`strategy_runs` 检查点、`strategy_events` 和 `admin_audit_logs`；列表支持 `limit`、`pair_id`、`status` 过滤并返回策略运行检查点，时间字段继续按 Unix milliseconds 数字序列化。
- 修改文件：
  - `src/modules/admin/routes.rs`
  - `tests/admin_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_market_strategy -- --nocapture`，实现前 `/market-strategies` 后台路由返回 404，符合后台行情策略接口缺失预期；实现后同 focused 测试 2 个通过。已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes -- --nocapture`，29 个 admin route 测试通过；已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试所有 test result 均为 ok，`tests/admin_routes.rs` 29 个通过，最终 doc-tests 0 failed。
- 后续事项：继续推进剩余后端缺口审计和整体收尾验证。

## 2026-05-30 16:22 - 后台行情策略状态一致性修复

- 完成内容：修复后台 `PATCH /admin/api/v1/market-strategies/{id}/status` 的状态一致性问题；当历史或人工写入的 `market_strategies` 缺少对应 `strategy_runs` 检查点时，状态更新现在返回 conflict 并回滚事务，避免出现策略状态已更新但 `run_status` 为 null、且错误写入策略事件或后台审计的半提交状态。
- 修改文件：
  - `src/modules/admin/routes.rs`
  - `tests/admin_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_market_strategy_status_update_rolls_back_when_run_checkpoint_missing -- --nocapture`，实现前返回 200 且响应 `run_status: null`，符合缺失一致性保护预期；实现后同测试通过。已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check` 与 focused `admin_market_strategy`，3 个测试通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes -- --nocapture`，30 个 admin route 测试通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试所有 test result 均为 ok，最终 doc-tests 0 failed；已执行 `superpowers:code-reviewer` 复核，返回 `[]`。
- 后续事项：继续推进剩余后端缺口审计和整体收尾验证。

## 2026-05-30 16:59 - 后台审计日志查询接口补齐

- 完成内容：在剩余后台缺口审计中确认设计文档要求平台后台可查看管理员关键操作审计日志；补齐 `GET /admin/api/v1/audit-logs`，要求 `AdminAuth`，支持 `admin_id`、`action`、`target_type`、`target_id` 和 `limit` 过滤，按 `created_at DESC, id DESC` 排序返回 `admin_audit_logs`，并确保 `created_at` 以 Unix milliseconds 数字序列化。
- 修改文件：
  - `src/modules/admin/routes.rs`
  - `tests/admin_routes.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_audit_log -- --nocapture`，实现前 `/admin/api/v1/audit-logs` 返回 404 且查询测试解析空响应失败，符合后台审计日志查询接口缺失预期；实现后同 focused 测试 2 个通过。已执行 `superpowers:code-reviewer` 复核，指出测试只验证时间字段为数字、未验证 Unix milliseconds；已修复测试为断言 `created_at == timestamp_millis()`。修复后已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 focused `admin_audit_log`，2 个测试通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes -- --nocapture`，32 个 admin route 测试通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features`，全量测试所有 test result 均为 ok，`tests/admin_routes.rs` 32 个通过，最终 doc-tests 0 failed；已执行 `superpowers:code-reviewer` 复核修复后的切片，返回 `[]`。
- 后续事项：继续推进最终后端 API 缺口审计和整体收尾验证。

## 2026-05-30 17:09 - 最终后端缺口审计与运行 smoke

- 完成内容：完成后台审计日志接口后的最终后端 API 缺口审计；确认 `src/modules/**/routes.rs`、`src/lib.rs`、`src/main.rs`、`src/config.rs`、`src/workers/*.rs` 与拆分设计文档中的核心 API/worker 面没有实际缺失或 stub；使用 docker-compose 凭据启动 API 做运行级 smoke，并验证 `/health` 和后台审计日志鉴权边界。
- 修改文件：
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 CodeGraph context/explore 审计核心 route/worker surface；已执行 placeholder 扫描，`src/**/*.rs`、`tests/**/*.rs`、`docs/superpowers/specs/blockchain-exchange/**/*.md` 中 `TODO`、`FIXME`、`todo!`、`unimplemented!`、`placeholder`、`stub`、`not implemented`、`StatusCode::NOT_IMPLEMENTED` 返回 `NO_PLACEHOLDER_MARKERS_FOUND_IN_SRC_TESTS_OR_SPLIT_SPECS`；已提取 `src/modules/**/routes.rs` 的 94 个 route declarations 和拆分 spec 的 80 条 API reference lines 做覆盖核对；独立 Explore 审计返回无实际 missing/stubbed backend gaps。运行 smoke 中，前两次尝试分别因本机 `timeout` 命令不存在、以及 RabbitMQ guest 凭据/缺少 `MONGODB_DATABASE` 配置失败；按 `.env.example` 与 `docker-compose.yml` 修正为 `exchange/exchange` 凭据和完整 env 后，`cargo run --bin exchange-api` 成功监听 `127.0.0.1:18080`；已执行 `curl -sS -i http://127.0.0.1:18080/health` 返回 200 `{"status":"ok"}`；已执行 `curl -sS -i http://127.0.0.1:18080/admin/api/v1/audit-logs` 返回 401 `UNAUTHORIZED`，符合后台审计日志接口必须鉴权的边界；smoke 后已停止进程。
- 后续事项：无明确剩余后端 route/worker stub；后续可进入部署配置、安全加固或更完整端到端业务验收。

## 2026-05-30 18:59 - Admin 前端 Vite Scaffold

- 完成内容：在 `web/` 下创建 Vite React TypeScript + Semi Design 前端骨架，接入 React Query provider、临时路由 `/ -> /login` 和中文登录占位页；修复 Vite 8 / Semi UI 2.99 的类型与 CSS export 边界。
- 修改文件：
  - `web/package.json`
  - `web/package-lock.json`
  - `web/index.html`
  - `web/tsconfig.json`
  - `web/tsconfig.node.json`
  - `web/vite.config.ts`
  - `web/vitest.setup.ts`
  - `web/eslint.config.js`
  - `web/src/main.tsx`
  - `web/src/styles.css`
  - `web/src/app/providers.tsx`
  - `web/src/app/router.tsx`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 `npm install --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web"`，成功安装 420 packages；首次 `npm run typecheck --prefix ... && npm run lint --prefix ... && npm run build --prefix ...` 失败于 Vite test 类型、CSS module declaration 和 Semi CSS export；修复后重新执行同一串命令，typecheck、lint、build 均通过。build 输出包含 Vite/Rolldown 对 `node_modules/lottie-web/build/player/lottie.js` direct eval 的第三方依赖警告，构建仍成功。
- 后续事项：继续 Task 2 前端认证与 API client。

## 2026-05-30 19:18 - Admin 前端认证与路由切片

- 完成内容：实现 Admin authStore、本地存储安全解析与清理、API client、Admin 登录 API、中文登录页、RequireAdmin 守卫、403/404 页面和 `/login`、`/403`、`/admin/*`、`*` 路由；按 TDD 为 authStore、apiRequest、RequireAdmin 增加测试。
- 修改文件：
  - `web/src/auth/authStore.ts`
  - `web/src/auth/authStore.test.ts`
  - `web/src/api/types.ts`
  - `web/src/api/client.ts`
  - `web/src/api/client.test.ts`
  - `web/src/api/adminAuth.ts`
  - `web/src/auth/LoginPage.tsx`
  - `web/src/auth/RequireAdmin.tsx`
  - `web/src/auth/RequireAdmin.test.tsx`
  - `web/src/pages/ForbiddenPage.tsx`
  - `web/src/pages/NotFoundPage.tsx`
  - `web/src/app/router.tsx`
  - `web/src/styles.css`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 `npm run test --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" -- src/auth/authStore.test.ts src/api/client.test.ts src/auth/RequireAdmin.test.tsx`，3 个测试文件、9 个测试通过；已执行 `npm run typecheck --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web"`，失败于既有非本切片文件 `web/src/shared/DataTable.tsx` 的 RowKey 类型和 `web/src/shared/JsonDrawer.tsx` 导入不存在的 Semi `Drawer`，本切片新增类型错误已修复。
- 后续事项：需修复既有 shared 组件 typecheck 问题后再跑全量 typecheck。


## 2026-05-30 19:21 - Admin 前端共享资源展示组件

- 完成内容：新增后台前端共享展示与资源页基础组件，覆盖时间戳、Decimal 金额、状态标签、数据表、筛选栏、JSON 抽屉、原因确认动作、资源列表请求封装、通用后台资源页和纯静态后台提示页；测试先行覆盖格式化组件与 AdminResourcePage。
- 修改文件：
  - `web/src/shared/TimestampText.tsx`
  - `web/src/shared/AmountText.tsx`
  - `web/src/shared/StatusTag.tsx`
  - `web/src/shared/format.test.tsx`
  - `web/src/api/adminResources.ts`
  - `web/src/shared/DataTable.tsx`
  - `web/src/shared/FilterBar.tsx`
  - `web/src/shared/JsonDrawer.tsx`
  - `web/src/shared/ConfirmAction.tsx`
  - `web/src/admin/resources/AdminResourcePage.tsx`
  - `web/src/admin/resources/AdminResourcePage.test.tsx`
  - `web/src/admin/resources/AdminNoticePage.tsx`
  - `web/vitest.setup.ts`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`npm run test --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" -- src/shared/format.test.tsx src/admin/resources/AdminResourcePage.test.tsx`，实现前因组件缺失失败，符合预期；实现后同命令 2 个测试文件、10 个测试通过。已执行 `npm run typecheck --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web"`，通过。修复过程中为 jsdom 增加 canvas getContext 测试桩，避免 Semi UI lottie 依赖阻断测试导入。
- 后续事项：无。

## 2026-05-30 19:40 - Admin 前端页面路由与动作切片

- 完成内容：实现后台 AdminLayout 与中文菜单，接入 `/admin` 守卫布局和子路由；新增仪表盘、后台资源配置、真实只读资源页路由、静态提示页路由；新增代理、新币生命周期、行情策略、闪兑规则、产品状态动作页，所有动作通过 `ConfirmAction` 要求原因后调用后端真实接口。
- 修改文件：
  - `web/src/layouts/PageHeader.tsx`
  - `web/src/layouts/AdminLayout.tsx`
  - `web/src/layouts/AdminLayout.test.tsx`
  - `web/src/admin/dashboard/DashboardPage.tsx`
  - `web/src/admin/resources/resourceConfigs.tsx`
  - `web/src/admin/actions/AgentManagementPage.tsx`
  - `web/src/admin/actions/NewCoinActions.tsx`
  - `web/src/admin/actions/MarketStrategyActions.tsx`
  - `web/src/admin/actions/ConvertRuleActions.tsx`
  - `web/src/admin/actions/ProductStatusActions.tsx`
  - `web/src/admin/routes.tsx`
  - `web/src/app/router.tsx`
  - `web/src/styles.css`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 `npm run test --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" -- src/layouts/AdminLayout.test.tsx`，1 个测试文件、1 个测试通过；已执行 `npm run typecheck --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web"`，通过。过程中先按 TDD 运行同一 AdminLayout 测试，失败于 Semi Nav 依赖 jsdom 缺少 `ResizeObserver`，改为项目内原生导航后通过。
- 后续事项：无。

## 2026-05-30 19:50 - Admin 前端最终验证

- 完成内容：完成 Admin-only 前端最终验证；修复 `web/src/api/client.test.ts` 中未使用的 `ApiError` 导入导致的 ESLint 失败；确认 Admin 登录、守卫路由、后台布局菜单、通用资源页、动作页和生产构建均可通过当前前端验证链路。
- 修改文件：
  - `web/src/api/client.test.ts`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 `npm run typecheck --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web"`，通过；首次执行 `npm run lint --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web"` 失败于 `web/src/api/client.test.ts` 未使用的 `ApiError` 导入，修复后重新执行通过；已执行 `npm run test --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web"`，6 个测试文件、20 个测试通过；已执行 `npm run build --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web"`，构建通过并生成 `dist/`，构建输出仍包含第三方依赖 `node_modules/lottie-web/build/player/lottie.js` direct eval 警告和 chunk size 警告，未阻断构建。未执行浏览器人工 smoke，原因是本轮以 CLI 验证链路完成最终验收。
- 后续事项：无。

## 2026-05-30 21:38 - Admin 功能补全与 UI 一致性验证

- 完成内容：接通 Admin 新币申购/派发资源页，补齐 Admin 用户列表/详情、钱包账户/流水、风控规则/事件后端 API 与前端资源页；接入 Semi `ConfigProvider` 中文 locale 与上海时区；扩展状态中文化；为 DataTable 增加本地受控分页；将 Admin 内容区样式统一为 Semi-like 浅色后台风格并保留深色侧边栏；修复前端路由测试类型问题、API client 测试基础 URL 断言，以及 Admin route 测试中短 UUID 引发的并行重复数据问题。
- 修改文件：
  - `src/modules/admin/routes.rs`
  - `tests/admin_routes.rs`
  - `web/src/app/providers.tsx`
  - `web/src/app/providers.test.tsx`
  - `web/src/shared/StatusTag.tsx`
  - `web/src/shared/StatusTag.test.tsx`
  - `web/src/shared/DataTable.tsx`
  - `web/src/shared/DataTable.test.tsx`
  - `web/src/admin/routes.tsx`
  - `web/src/admin/routes.test.tsx`
  - `web/src/admin/resources/resourceConfigs.tsx`
  - `web/src/api/client.test.ts`
  - `web/src/styles.css`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 `npm run typecheck --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web"`，通过；已执行 `npm run lint --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web"`，通过；已执行 `npm run test --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web"`，10 个测试文件、52 个测试通过；已执行 `npm run build --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web"`，构建通过，仍有第三方 `lottie-web` direct eval 与 chunk size 警告，未阻断构建；已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes -- --nocapture`，36 个测试通过；首次执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" REDIS_URL="redis://127.0.0.1:6379" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-features` 失败于 `tests/admin_routes.rs` 中短 UUID 生成的 admin role 名重复，修复后重新执行通过，最终所有测试结果均为 ok，doc-tests 0 failed。
- 后续事项：无。

## 2026-05-30 22:03 - Admin 侧边栏二级导航与拖拽宽度

- 完成内容：将 Admin 后台侧边栏导航改为可展开/收起的二级目录，当前路由所在分组自动展开并保持 active 状态；侧边栏和导航默认占满视口高度，导航区内部滚动；新增侧边栏宽度拖拽 handle，并支持键盘左右方向键调整宽度，限制在 240px 到 420px。
- 修改文件：
  - `web/src/layouts/AdminLayout.tsx`
  - `web/src/layouts/AdminLayout.test.tsx`
  - `web/src/styles.css`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`npm run test --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" -- src/layouts/AdminLayout.test.tsx`，实现前 2 个新增测试失败，分别缺少二级目录展开按钮和侧边栏拖拽 handle，符合预期。实现后已执行 `npm run test --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" -- src/layouts/AdminLayout.test.tsx && npm run typecheck --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" && npm run lint --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web"`，AdminLayout 测试 3 个通过，TypeScript typecheck 通过，ESLint 通过。
- 后续事项：无。

## 2026-05-30 22:19 - 第三方行情订阅启动配置修复

- 完成内容：按系统化调试确认 API 启动入口已创建 market feed task，未启动订阅的直接原因是 `.env` 中 `MARKET_FEED_SYMBOLS` 为空；该空配置会触发 `market_feed::run_loop` 的 disabled 分支并直接返回。已为本地环境配置 BTC/ETH 对 USDT 的第三方行情订阅，并开启 1m/5m/15m/1h/1d K 线 interval 与 Bitget、HTX provider。
- 修改文件：
  - `.env`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib config::tests::settings_from_env_parses_market_feed_lists -- --nocapture && cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test market_feed_worker market_feed_runtime_config_validates_startup_symbols_and_intervals -- --nocapture && cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test market_feed_worker runtime_provider_codes_default_and_deduplicate_in_order -- --nocapture`，3 个聚焦测试均通过。未执行真实第三方 WebSocket smoke，原因是本次修复聚焦本地启动配置与订阅启用条件，未启动完整依赖服务和长连接运行。
- 后续事项：如需线上自动订阅后台交易对，应后续改为从数据库 active trading pairs 加载 symbols，而不是依赖 `.env` 固定列表。

## 2026-05-31 00:26 - Admin 行情订阅配置与凭证管理

- 完成内容：新增后台可配置第三方行情订阅闭环，包含 MySQL `market_feed_configs` 与 `market_source_credentials` migration、`CREDENTIAL_ENCRYPTION_KEY` 配置、凭证 AES-GCM 加密/掩码展示、Admin 保存配置/凭证/状态/手动重载 API、market feed supervisor 手动 reload 与启动时 DB 配置优先 fallback、React Admin 行情订阅配置页和导航入口；保存配置不会立即生效，需点击“重载行情订阅”应用。
- 修改文件：
  - `Cargo.toml`
  - `.env`
  - `migrations/0034_market_feed_admin_config.sql`
  - `src/config.rs`
  - `src/state.rs`
  - `src/main.rs`
  - `src/workers/market_feed.rs`
  - `src/modules/admin/mod.rs`
  - `src/modules/admin/market_feed_config.rs`
  - `src/modules/admin/routes.rs`
  - `tests/admin_routes.rs`
  - `tests/market_feed_worker.rs`
  - `web/src/admin/actions/MarketFeedConfigPage.tsx`
  - `web/src/admin/actions/MarketFeedConfigPage.test.tsx`
  - `web/src/admin/routes.tsx`
  - `web/src/admin/routes.test.tsx`
  - `web/src/layouts/AdminLayout.tsx`
  - `web/src/layouts/AdminLayout.test.tsx`
  - `web/src/shared/ConfirmAction.tsx`
  - `web/src/shared/StatusTag.tsx`
  - `web/src/shared/StatusTag.test.tsx`
  - `docs/superpowers/specs/2026-05-30-market-feed-admin-config-design.md`
  - `docs/superpowers/plans/2026-05-30-market-feed-admin-config-implementation.md`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib modules::admin::market_feed_config::tests -- --nocapture`，3 个测试通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test market_feed_worker market_feed_supervisor_status_tracks_reload_success -- --nocapture`，1 个测试通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_market_feed -- --nocapture`，4 个测试通过，其中当前环境未设置 `DATABASE_URL`，3 个 MySQL seeded 分支按测试设计跳过；已执行 `npm run test --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" -- MarketFeedConfigPage StatusTag AdminLayout routes`，4 个测试文件、41 个测试通过；已执行 `npm run typecheck --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web"`，通过；已执行 `npm run lint --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web"`，通过；已执行 `npm run test --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web"`，11 个测试文件、62 个测试通过。未执行真实第三方 WebSocket reload smoke，原因是本次验证聚焦配置、加密、API、supervisor 状态和前端交互，未启动完整 MySQL/Redis/Mongo/RabbitMQ 与外部长连接。
- 后续事项：生产环境需配置强随机 32 字节 `CREDENTIAL_ENCRYPTION_KEY` 并用真实 `DATABASE_URL` 补跑 market-feed Admin MySQL seeded 集成路径；后续如需让 provider adapter 实际消费私有 API 凭证，可在行情源私有接口需求明确后继续接入。

## 2026-05-31 01:43 - RabbitMQ 事件 exchange 自动声明

- 完成内容：定位 `NOT_FOUND - no exchange 'exchange.events'` 根因为事件 outbox 发布前未声明 RabbitMQ exchange；在 `RabbitMqOutboxPublisher` 发布前自动声明 durable topic exchange，避免新 vhost（例如 `/hippo`）缺少 `exchange.events` 时关闭 channel。
- 修改文件：
  - `src/modules/events/mod.rs`
  - `tests/events_outbox.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 `RABBITMQ_URL="amqp://exchange:exchange@127.0.0.1:5672/%2f" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test events_outbox rabbitmq_outbox_publisher_declares_exchange_before_publish -- --nocapture`，修复前失败 `InvalidChannelState(Closed)`，修复后 1 个测试通过；已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test events_outbox`，10 个测试通过、0 失败；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过。
- 后续事项：如果生产 RabbitMQ 用户没有 `configure` 权限，需要为该用户补充 exchange 声明权限，或由运维预先创建 durable topic exchange `exchange.events`。

## 2026-05-31 02:20 - Bitget ticker 结构兼容与日志中文化

- 完成内容：为用户提供的 Bitget `snapshot/ticker` payload 增加精确回归测试，确认现有解析逻辑支持 `lastPr`、`baseVolume` 和 data 内 `ts`；将后端 `tracing` 运行日志文案中文化，并同步事件 inbox alert 测试断言。
- 修改文件：
  - `src/modules/market/mod.rs`
  - `src/error.rs`
  - `src/main.rs`
  - `src/modules/events/mod.rs`
  - `src/workers/event_outbox.rs`
  - `src/workers/event_inbox.rs`
  - `src/workers/market_feed.rs`
  - `src/workers/kline_recovery.rs`
  - `src/workers/unlock_scanner.rs`
  - `src/workers/seconds_contract_settlement.rs`
  - `src/workers/earn_auto_redemption.rs`
  - `src/workers/margin_liquidation.rs`
  - `src/workers/margin_interest.rs`
  - `tests/events_inbox.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --lib bitget_ticker_from_ws_accepts_snapshot_payload_shape -- --nocapture`，1 个测试通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test events_inbox`，32 个测试通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test events_outbox`，10 个测试通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过。
- 后续事项：无。

## 2026-05-31 02:26 - Admin 行情订阅配置页交互优化

- 完成内容：优化 Admin 行情订阅配置页面，将 K 线 `intervals` 从逗号输入改为多选勾选，将行情 `providers` 改为可多选自由切换，并在运行状态中明确显示“当前启动 providers”。
- 修改文件：
  - `web/src/admin/actions/MarketFeedConfigPage.tsx`
  - `web/src/admin/actions/MarketFeedConfigPage.test.tsx`
  - `web/src/styles.css`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`npm run test --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" -- src/admin/actions/MarketFeedConfigPage.test.tsx`，实现前 2 个测试失败，缺少 interval/provider checkbox 与当前启动 providers 展示，符合预期；实现后已执行同一测试命令，4 个测试通过；已执行 `npm run typecheck --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web"`，通过；已执行 `npm run lint --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web"`，通过。
- 后续事项：无。

## 2026-05-31 02:39 - 未识别行情频道跳过处理

- 完成内容：为未识别行情 WebSocket payload 增加 `MarketFeedChannel::None` 跳过路径，避免 account 等非行情频道被创建为 `MarketFeedFrame` 进入 ingestion；补齐 `parse_feed_frame` 对 `None` 的穷尽处理，并保留 `ticker/detail`、`depth/books`、`kline/candle`、`trade` 的识别行为。
- 修改文件：
  - `src/modules/market/mod.rs`
  - `src/workers/market_feed.rs`
  - `tests/market_feed_worker.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test market_feed_worker market_feed_socket_action_ignores_unrecognized_channel_payloads -- --nocapture`，实现前先失败于 `MarketFeedChannel::None` match 未穷尽，符合缺失处理预期；实现后同一测试 1 个通过。已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test market_feed_worker market_feed_socket_action_handles_pings_closes_and_data_frames -- --nocapture`，1 个通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test market_feed_worker -- --nocapture`，31 个测试通过；已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过。
- 后续事项：无。

## 2026-05-31 08:05 - Admin 现货杠杆秒合约添加入口

- 完成内容：新增后台 `GET/POST /admin/api/v1/market-pairs`，支持 Admin 创建/查询现货交易对，包含资产启用校验、交易对符号规范化、精度/最小下单额/status/market_type 校验、重复交易对 conflict 和 `trading_pair.create` 审计日志；Admin 前端产品动作页新增创建现货交易对、创建杠杆产品、创建秒合约产品三个表单，均通过 `ConfirmAction` 提交操作原因；交易对资源页改用 Admin 交易对接口；现货导航新增交易对配置和现货动作入口；现货交易对创建按钮在价格精度/数量精度等必填字段有效前保持禁用，避免空精度被提交为 0。
- 修改文件：
  - `src/modules/admin/routes.rs`
  - `tests/admin_routes.rs`
  - `web/src/admin/actions/ProductStatusActions.tsx`
  - `web/src/admin/actions/ProductStatusActions.test.tsx`
  - `web/src/admin/resources/resourceConfigs.tsx`
  - `web/src/admin/routes.tsx`
  - `web/src/admin/routes.test.tsx`
  - `web/src/layouts/AdminLayout.tsx`
  - `web/src/layouts/AdminLayout.test.tsx`
  - `src/workers/market_feed.rs`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行后端 RED：新增 Admin 交易对测试后 `/admin/api/v1/market-pairs` 先返回 404，注册路由后编译失败于缺少 handler，符合缺失接口预期；实现后 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes -- --nocapture` 通过 42 个测试，但当前环境未设置 `DATABASE_URL`，MySQL seeded 分支按测试设计跳过。已执行前端 RED：`npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" test -- ProductStatusActions.test.tsx` 初始 3 个测试失败于找不到新增表单标签；实现后同命令通过 3 个测试。代码复核发现现货动作入口不可发现、空精度可能被当作 0 提交；补充 RED：`npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" test -- routes.test.tsx AdminLayout.test.tsx ProductStatusActions.test.tsx` 先失败于缺少 `spot/actions` 路由、缺少“现货动作”导航、创建按钮未禁用；修复后 3 个文件 16 个测试通过。最终已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_trading_pair_routes_require_admin_scope_mysql_and_validation -- --nocapture`，1 个通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" test -- ProductStatusActions.test.tsx routes.test.tsx AdminLayout.test.tsx`，3 个文件 16 个测试通过；已执行 `npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" run typecheck`，通过；已执行 `npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" run lint`，通过；已执行两轮 `superpowers:code-reviewer`，第二轮确认无剩余 blocker/important 问题。
- 后续事项：当前环境未设置 `DATABASE_URL`，如需真实数据库路径验证，可补充带 `DATABASE_URL` 的 Admin 交易对创建/审计测试。

## 2026-05-31 09:12 - Admin 交易对添加入口拆分到配置页

- 完成内容：根据登录 Admin 后入口不可见反馈，确认根因是添加入口集中在产品动作页而非各配置页；已将现货交易对添加按钮放到交易对配置页，点击后弹窗填写基础/计价资产、交易对符号、精度、最小下单额、状态、市场类型和操作原因；已将杠杆交易对添加按钮放到杠杆产品页，点击后弹窗创建杠杆产品；已将秒合约交易对添加按钮放到秒合约产品页，点击后弹窗创建秒合约产品。
- 修改文件：
  - `web/src/admin/resources/AdminResourcePage.tsx`
  - `web/src/admin/resources/ResourceCreateActions.tsx`
  - `web/src/admin/resources/resourceConfigs.tsx`
  - `web/src/admin/resources/resourceConfigs.test.tsx`
  - `web/src/styles.css`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" test -- resourceConfigs.test.tsx`，实现前 3 个测试失败于找不到“添加交易对 / 添加杠杆交易对 / 添加秒合约交易对”按钮，符合入口缺失预期；实现后同命令 1 个文件 3 个测试通过。已执行 `npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" test -- resourceConfigs.test.tsx AdminResourcePage.test.tsx ProductStatusActions.test.tsx routes.test.tsx AdminLayout.test.tsx`，5 个文件 22 个测试通过；已执行 `npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" run typecheck`，通过；已执行 `npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" run lint`，通过。
- 后续事项：无。

## 2026-05-31 09:52 - Admin 资产管理页面和接口

- 完成内容：新增 Admin 资产管理闭环，后台提供 `GET/POST /admin/api/v1/assets`，支持 AdminAuth 鉴权、资产符号大写规范化、资产名称/精度/类型/状态校验、重复资产 conflict、筛选列表和 `asset.create` 审计日志；前端在“钱包资产”二级导航下增加“资产管理”页面，显示资产列表并提供“添加资产”弹窗，通过二次确认原因提交创建资产。
- 修改文件：
  - `src/modules/admin/routes.rs`
  - `tests/admin_routes.rs`
  - `web/src/admin/resources/AdminResourcePage.tsx`
  - `web/src/admin/resources/ResourceCreateActions.tsx`
  - `web/src/admin/resources/resourceConfigs.tsx`
  - `web/src/admin/resources/resourceConfigs.test.tsx`
  - `web/src/admin/routes.tsx`
  - `web/src/admin/routes.test.tsx`
  - `web/src/layouts/AdminLayout.tsx`
  - `web/src/layouts/AdminLayout.test.tsx`
  - `web/src/styles.css`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_asset -- --nocapture`，2 个测试通过，其中无 `DATABASE_URL` 场景按测试设计跳过 seeded MySQL 分支；已执行 `DATABASE_URL="mysql://exchange:exchange@127.0.0.1:3306/exchange" cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_asset_create_list_and_audit -- --nocapture`，1 个 MySQL-backed 资产创建/列表/审计测试通过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" test -- resourceConfigs.test.tsx routes.test.tsx AdminLayout.test.tsx`，3 个文件 17 个测试通过；已执行 `npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" run typecheck`，通过；已执行 `npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" run lint`，通过；已执行 `git diff --check`，通过；已执行 `superpowers:code-reviewer` 复核，无 blocker/important 问题。
- 后续事项：无。

## 2026-05-31 16:22 - Admin 现货交易对与订单 CRUD 安全闭环

- 完成内容：新增 Admin 交易对详情与启停接口 `GET /admin/api/v1/market-pairs/:id`、`PATCH /admin/api/v1/market-pairs/:id/status`，启停写入 `trading_pair.status.update` 审计，服务端强制操作原因非空，并在交易对行锁定后读取审计 before 快照；新增 Admin 现货订单详情与管理员撤单接口 `GET /admin/api/v1/spot/orders/:id`、`POST /admin/api/v1/spot/orders/:id/cancel`，管理员撤单复用现货撤单状态机和钱包解冻事务，服务端强制操作原因非空，并写入 `spot_order.cancel` 审计；前端通用资源页支持行级动作，交易对列表支持启用/禁用，现货订单列表支持查看详情/管理员撤单，并补充订单已成交数量与成交手续费列。
- 修改文件：
  - `src/modules/admin/routes.rs`
  - `src/modules/spot/routes.rs`
  - `tests/admin_routes.rs`
  - `tests/spot_routes.rs`
  - `web/src/admin/resources/AdminResourcePage.tsx`
  - `web/src/admin/resources/AdminResourcePage.test.tsx`
  - `web/src/admin/resources/ResourceCreateActions.tsx`
  - `web/src/admin/resources/resourceConfigs.tsx`
  - `web/src/admin/resources/resourceConfigs.test.tsx`
  - `docs/superpowers/PROGRESS.md`
- 验证结果：已执行 RED：`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_trading_pair_detail_and_status_routes_require_admin_scope_mysql -- --nocapture`，实现前失败于 404，符合路由缺失预期；已执行 RED：`cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test spot_routes admin_spot_order_detail_and_cancel_routes_require_admin_scope_mysql -- --nocapture`，实现前失败于 404，符合路由缺失预期；已执行 RED：`npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" test -- AdminResourcePage.test.tsx`，实现前失败于找不到“查看详情”行级按钮，符合通用行级动作缺失预期；已执行 RED：`npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" test -- resourceConfigs.test.tsx`，新增交易对/订单行级动作测试实现前 3 个失败，符合按钮和 fee 列缺失预期。实现后已执行 `cargo fmt --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --check`，通过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test admin_routes admin_trading_pair -- --nocapture`，4 个测试通过，其中 MySQL-gated seeded 分支因本地未设置 `DATABASE_URL` 按设计跳过；已执行 `cargo test --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --test spot_routes admin_spot_order -- --nocapture`，2 个测试通过，其中 MySQL-gated seeded 分支因本地未设置 `DATABASE_URL` 按设计跳过；已执行 `cargo check --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets`，通过；已执行 `cargo clippy --manifest-path "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/Cargo.toml" --all-targets --all-features -- -D warnings`，通过；已执行 `npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" test -- AdminResourcePage.test.tsx resourceConfigs.test.tsx`，2 个文件 11 个测试通过；已执行 `npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" run typecheck`，通过；已执行 `npm --prefix "/Users/huangkunhuang/Public/程序工程目录/复合工程/rust-chain/web" run lint`，通过；已执行 `git diff --check`，通过。
- 后续事项：交易对完整编辑、现货订单强类型详情页、成交只读详情、冻结资产/成交明细/钱包流水/审计记录联动，以及杠杆、秒合约、Earn、闪兑等模块的详情页和安全行级操作。
