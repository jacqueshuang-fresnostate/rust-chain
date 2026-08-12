# Research: Rust 后端中文注释与 DDD 架构全量审计（2026-08-12）

- Query: 对 `src/**/*.rs`（排除生成产物）做可量化的全量架构与中文注释审计，核查注释覆盖、风险职责、routes 越权、DDD 空壳层、复杂度/重复和 PRD 已完成项，并给出行为保持不变的分批方案。
- Scope: internal
- Date: 2026-08-12

## Findings

### 1. 结论摘要

- **总体结论：目录形态已 DDD 化，职责边界仍是“渐进迁移中”，且中文业务注释远未形成可验收覆盖。** `src` 共 **251 个 Rust 文件、81,811 行、3,468 个函数/方法/trait 方法**；其中只有 **162 个（4.7%）**具备中文 `///`/`//!` 文档注释，**201 个（5.8%）**函数体内含中文普通行注释，按“中文文档注释或函数体内中文行注释任一命中”计为 **359 个（10.4%）**。
- **公开职责说明缺口很大。** `pub`/`pub(crate)`/受限公开/trait 方法共 **2,122 个**；其中仅 **137 个（6.5%）**有中文文档注释，**313 个（14.8%）**在函数级范围内命中任一中文注释，故 **1,985 个（93.5%）**没有中文文档职责说明，**1,809 个（85.2%）**连函数体内业务注释也没有。
- **风险函数缺口更集中。** 按名称命中钱包、余额、流水、充提、订单、结算、强平、保证金、利息、鉴权、密码、Token、2FA、风控、幂等、事务等风险词的公开函数共有 **944 个**，仅 **58 个（6.1%）**有中文文档注释；**886 个（93.9%）**无中文业务约束文档，其中 **247 个**长度至少 25 行、**71 个**至少 50 行、**10 个**至少 100 行。
- **routes 的“原始 SQL/事务”目标基本达成，但仍有边界泄漏。** 对全部 `src/modules/*/routes.rs` 搜索 `sqlx::query`、`QueryBuilder`、`.begin(` 均为 0；多数 handler 已是鉴权、取 pool、调用 application、返回 DTO。例：wallet 路由见 `src/modules/wallet/routes.rs:84-197`，margin 开/平/撤仓见 `src/modules/margin/routes.rs:278-385`。但 `events/routes.rs` 直接导入并调用 infrastructure 查询/重排（`src/modules/events/routes.rs:8-18,58-99`）；admin/user routes 直接调用 `multipart_file_input` infrastructure adapter（`src/modules/admin/routes.rs:115`、`src/modules/user/routes.rs:27`）；auth route 自己持有 85 行 Turnstile HTTP provider 工作流与策略分支（`src/modules/auth/routes.rs:386-539`），未做到“调用一个 application use case”。
- **六层文件“存在”不等于六层有实现。** 22 个 bounded context 的六个 layer 文件均存在，但至少 **15 个 layer 文件是纯 marker/空壳**：`earn/prediction/quick_recharge/seconds_contract/domain.rs`，`risk/presentation.rs`，`security/presentation.rs`，`countries/kyc/loan/margin/news/platform/security/repository.rs`，`news/security/service.rs`。因此 PRD 的“anchor files 存在”是结构事实，不是全量 DDD 完成度证明。
- **生产测试体并未全部外移。** `src/openapi/auth.rs:394-443` 仍含 `#[cfg(test)] mod tests { ... }`。执行 `cargo test --manifest-path Cargo.toml --test backend_architecture`：4 项中 2 项失败，失败项正是“无内嵌测试体”和“测试模块必须指向 `tests/unit_src`”。PRD 第 54、56、57 项的 `[x]` 当前均不准确；第 58 项则在本次复核通过。
- **复杂度与重复风险仍高。** 15 个文件超过 1,000 行，4 个超过 2,000 行；159 个函数超过 50 行，45 个超过 80 行，26 个超过 100 行。精确归一化 SQL 字符串共 663 种，其中 61 组重复（177 次出现），42 组跨文件重复。最突出的是钱包行锁、余额更新、流水写入散布于 loan/prediction/spot/margin/earn/quick_recharge/workers/admin 等路径。

### 2. 统计方法与口径

#### 2.1 文件与语法统计

1. 文件集：`find src -type f -name '*.rs'`，排除 `target`/生成目录；本仓库 `src` 下未发现生成产物目录，共 251 文件。
2. 行数：物理行（`wc -l`），共 81,811 行。
3. 函数/方法：使用离线临时 `syn 2` AST 解析器遍历 `ItemFn`、`ImplItemFn`、`TraitItemFn`；251 文件全部解析成功、0 个 parse error。闭包不计入，宏展开生成的方法不计入；trait 声明方法计入。
4. 可见性：`pub`、`pub(crate)`、其他 `pub(...)` 与 trait 方法归为“公开职责”；private 单独统计。
5. 函数长度：函数名所在行至 AST span 结束行（含签名与函数体），用于排序，不等价于 cyclomatic complexity。

#### 2.2 中文注释覆盖

- **中文文档注释**：函数/方法自身 `#[doc = ...]`（由 `///` 转换）含 U+3400–U+9FFF 汉字；文件级 `//!` 只说明文件职责，不自动记为每个函数覆盖。
- **中文普通行注释**：函数 AST span 内非 doc 的 `//` 注释含汉字。字符串/日志中的中文不算注释。
- **任一中文覆盖**：函数中文 doc 或函数体中文行注释至少一个命中。由于普通注释只要求“出现”，这是宽松上限，不代表已解释完整职责、前置条件、幂等、事务和失败语义。
- 全仓物理注释行另作辅助：**787 个 doc 注释行（487 个中文）**、**259 个普通 `//` 注释行（251 个中文）**。其分布很薄，不应把“文件顶部有中文层职责”误判为函数覆盖。

#### 2.3 风险、SQL、空壳与重复

- 风险函数是**启发式**：公开函数名匹配 wallet/balance/ledger/withdraw/deposit/order/settle/liquidation/margin/interest/auth/login/password/token/2FA/security/risk/idempotency/transaction/in_tx 等；会包含少量假阳性，也会漏掉命名模糊的真风险函数。
- routes 越权：静态搜索原始 SQL/事务、直接 infrastructure/service 依赖，再人工阅读命中点。
- 空壳层：文件只有层注释和 `*LayerMarker`，或只有层注释；“含迁移锚点文案但已有真实代码”的文件记为 partial，不算纯空壳。
- SQL 重复：抽取源码字符串字面量中包含 `SELECT/INSERT/UPDATE/DELETE` 的 SQL，压缩空白并转小写后精确比较；动态拼接、语义相同但文本不同的不计入，因此 61 组是保守下限。

### 3. 分组量化统计

下表 `中文 doc` 是函数自身中文文档注释；`中文行注释` 是函数体内中文普通注释；`任一覆盖` 不等于两列简单相加（少量函数两者同时存在）；`公开覆盖` 是公开函数命中任一中文注释。

| 分组 | Rust 行数 | 函数/方法 | 中文 doc | 中文行注释 | 任一覆盖 | 任一覆盖率 | 公开函数 | 公开覆盖 | 公开覆盖率 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `src/modules/admin` | 18,018 | 703 | 10 | 57 | 67 | 9.5% | 493 | 64 | 13.0% |
| `agent` | 1,527 | 57 | 3 | 7 | 10 | 17.5% | 44 | 9 | 20.5% |
| `auth` | 3,853 | 195 | 5 | 12 | 16 | 8.2% | 106 | 13 | 12.3% |
| `convert` | 1,662 | 69 | 3 | 5 | 8 | 11.6% | 51 | 6 | 11.8% |
| `countries` | 253 | 11 | 2 | 1 | 3 | 27.3% | 8 | 3 | 37.5% |
| `earn` | 2,878 | 123 | 5 | 2 | 7 | 5.7% | 73 | 6 | 8.2% |
| `events` | 3,095 | 155 | 10 | 4 | 13 | 8.4% | 109 | 13 | 11.9% |
| `kyc` | 1,374 | 48 | 0 | 4 | 4 | 8.3% | 31 | 3 | 9.7% |
| `loan` | 2,251 | 94 | 13 | 12 | 25 | 26.6% | 63 | 23 | 36.5% |
| `margin` | 4,542 | 175 | 19 | 14 | 32 | 18.3% | 90 | 29 | 32.2% |
| `market` | 3,969 | 304 | 0 | 0 | 0 | 0.0% | 212 | 0 | 0.0% |
| `new_coin` | 3,497 | 124 | 0 | 8 | 8 | 6.5% | 59 | 4 | 6.8% |
| `news` | 354 | 18 | 1 | 1 | 2 | 11.1% | 12 | 2 | 16.7% |
| `platform` | 400 | 20 | 3 | 1 | 4 | 20.0% | 12 | 4 | 33.3% |
| `prediction` | 3,099 | 105 | 4 | 7 | 11 | 10.5% | 97 | 11 | 11.3% |
| `quick_recharge` | 2,572 | 95 | 6 | 4 | 10 | 10.5% | 56 | 9 | 16.1% |
| `risk` | 677 | 30 | 12 | 1 | 13 | 43.3% | 12 | 6 | 50.0% |
| `seconds_contract` | 2,580 | 105 | 9 | 9 | 18 | 17.1% | 73 | 17 | 23.3% |
| `security` | 1,102 | 53 | 2 | 1 | 3 | 5.7% | 44 | 3 | 6.8% |
| `spot` | 5,574 | 213 | 19 | 23 | 41 | 19.2% | 127 | 34 | 26.8% |
| `user` | 2,590 | 110 | 4 | 8 | 12 | 10.9% | 78 | 12 | 15.4% |
| `wallet` | 5,188 | 204 | 29 | 9 | 38 | 18.6% | 119 | 37 | 31.1% |
| **全部 modules** | **67,477** | **3,011** | **159** | **190** | **345** | **11.5%** | **1,969** | **308** | **15.6%** |
| `src/workers` | 5,382 | 236 | 3 | 9 | 12 | 5.1% | 101 | 4 | 4.0% |
| `src/infra` | 444 | 29 | 0 | 0 | 0 | 0.0% | 21 | 0 | 0.0% |
| 其他 `src` | 8,508 | 192 | 0 | 2 | 2 | 1.0% | 31 | 1 | 3.2% |
| **总计** | **81,811** | **3,468** | **162** | **201** | **359** | **10.4%** | **2,122** | **313** | **14.8%** |

> 注：按 PRD 要求重点分组时，`src/modules/<context>` 行数合计 67,477；另外 `src/modules/mod.rs` 25 行归入“其他 src”。

### 4. P0/P1 中文业务约束缺失清单

#### 4.1 P0：资金/结算/强平/幂等边界（先补职责合同，再拆代码）

以下函数均无中文函数文档；“有行内注释”也只解释局部，不构成入口合同。

| 优先级 | 证据 | 风险与应说明的中文约束 |
|---|---|---|
| P0 | `src/modules/prediction/infrastructure.rs:242-354` `create_order_in_tx` | 113 行内处理 quote 所有权/过期、幂等重放、订单插入、钱包冻结、手续费、代理佣金、commit；应写明单事务边界、重复键重放、quote 单次消费和资金/佣金一致性。 |
| P0 | `src/modules/prediction/infrastructure.rs:356-489` `settle_market_in_tx` | 134 行批量结算或退款；应写明 market/order 行锁、重复结算幂等、invalid refund policy、每订单钱包流水与市场状态同事务。 |
| P0 | `src/modules/prediction/infrastructure.rs:1149-1361` 三个 `apply_wallet_prediction_*` | 冻结、胜负结算、退款三条钱包路径无中文入口合同；代码直接更新 `wallet_accounts` 和多笔 ledger（如 `:1166-1221`、`:1241-1286`）。 |
| P0 | `src/modules/wallet/infrastructure.rs:1064-1141` `reserve_withdrawal_request` | 请求插入、余额行锁、available→frozen、ledger、回读同事务；无文档说明费用计入 total_reserved、锁顺序与幂等责任。 |
| P0 | `src/modules/wallet/infrastructure.rs:1233-1420` `release_withdrawal_in_tx` / `confirm_withdrawal_in_tx` | 释放冻结与链上确认涉及状态机、冻结余额扣减/退回和流水；只有前者局部中文，两个入口均无完整状态前置条件及重放语义。 |
| P0 | `src/modules/wallet/infrastructure.rs:1579-1741` `observe_deposit_event` / `reverse_deposit_event` | 92/70 行，处理链事件幂等、账户入账/冲正、流水与事件状态；均无中文函数合同。 |
| P0 | `src/modules/spot/application.rs:619-763` `settle_spot_fill` | 145 行，有局部锁序注释但无入口 doc；应描述买卖订单锁序、成交幂等、保留金额、四条资金腿、订单状态与事件/outbox 边界。 |
| P0 | `src/modules/spot/infrastructure.rs:1469-1624` `apply_spot_wallet_freeze` / `apply_spot_wallet_settlement_leg` | 直接做钱包余额和流水，重复实现通用资金写入；应说明 available/frozen 双向流水、负数防护和调用方事务所有权。 |
| P0 | `src/modules/margin/application.rs:96-220` `open_margin_position` | 125 行，局部中文但无入口合同；应说明幂等重放、报价/杠杆/方向验证、仓位插入、抵押金钱包来源与提交顺序。 |
| P0 | `src/modules/margin/infrastructure.rs:1239-1428` `debit_margin_position_open_collateral` / `credit_margin_position_amount` | cross/isolated 在 margin/spot 钱包之间回退选择，直接写不同 ledger；无文档说明 wallet_scope 选择优先级和一致性。 |
| P0 | `src/workers/margin_liquidation.rs:544-725` `liquidate_cross_account` | 私有但极高风险，182 行；虽有 `///` 中文入口说明命中统计，应仍按强平验收核对：账户级锁、权益分摊、坏账/负权益、事件发布在 commit 后。证据说明“已有注释≠函数过长已解决”。 |
| P0 | `src/modules/seconds_contract/application.rs:235-374,389-472` `open_order` / `settle_order` | 分别 140/84 行，只有局部中文；应写明期限/赔率快照、幂等键、入场/结算价来源、钱包冻结/派奖、管理员重复结算语义。 |
| P0 | `src/modules/quick_recharge/application.rs:372-525` `handle_gmpay_notify` | 154 行，验签、PID/status/amount/token 校验、重复回调、订单锁、钱包入账集中一处；`:506-514` 有关键局部注释，但入口无回调幂等与敏感日志边界合同。 |
| P0 | `src/workers/wallet_chain.rs:98-249` `run_once_with_gateway` | 152 行，提现广播/轮询/确认、充值游标、死信和重试混合；有局部中文但无入口 doc，且 worker 直接依赖 wallet infrastructure（`src/workers/wallet_chain.rs:4-17`）。 |

#### 4.2 P1：鉴权、安全、公开 API 和后台作业职责

- `src/modules/auth/application.rs:410-460` `login_user_with_optional_two_factor`：无中文 doc；`login_2fa_mode` 决定发 Token、登录挑战或首次绑定挑战（`:415-459`），应明确凭据验证成功不等于会话签发、mandatory 2FA 不得降级。
- `src/modules/security/application.rs:91-122` `verify_user_security_action`：无中文 doc；按支付策略决定资金密码/TOTP 组合，disabled 直接返回 method；应说明策略来源、错误语义、验证次序与“不校验即放行”的配置含义。
- `src/modules/auth/mod.rs:261-284` `revoke_actor_auth_sessions`、`:324-343` `issue_token`、`:372-387` `claims_from_bearer_token`：会话撤销/签发/兼容解码均无中文职责说明，关联 `.trellis/spec/backend/auth-sessions.md:38-58`。
- `src/infra/auth.rs:14-41`、`src/infra/secrets.rs:10-80`：cross-context 鉴权存储和密钥加解密共 21 个公开 infra 函数整体 0 中文覆盖；至少给 `connect`、`memory_manager`、`encrypt_secret`、`decrypt_secret` 写职责、密钥/nonce/错误边界。
- workers 共有 101 个公开函数仅 4 个命中任一中文注释、0 个中文函数文档。`margin_interest`（`src/workers/margin_interest.rs:72-90`）、agent commission（`src/workers/agent_commission_settlement.rs:45-85`）、earn/seconds/unlock 等批处理入口应写清：扫描上限、行锁、逐项失败是否继续、幂等键、commit 后事件、重试/死信策略。
- `market` 是明显盲区：304 个函数、212 个公开函数，函数级中文覆盖 **0**；最大文件 `src/modules/market/infrastructure.rs` 2,871 行。行情解析、provider failover、Kline 持久化虽不是直接资金写入，但价格是结算/强平输入，建议列 P1 而非一般 P2。

#### 4.3 建议的注释验收格式（不是机械逐行注释）

对 P0/P1 入口使用 3–6 行中文 `///`，至少回答：

1. **职责/业务结果**：这个 use case 改变什么状态；
2. **前置条件/权限**：身份、状态、价格/精度、幂等键；
3. **事务/锁顺序**：哪些写入必须同事务，谁负责 begin/commit；
4. **资金不变量**：available/frozen/locked 与 ledger 如何对应；
5. **重放/失败语义**：重复请求返回旧结果、跳过还是冲突；
6. **外部副作用**：事件、HTTP、消息发布在 commit 前还是后。

### 4.4 风险公开函数长度 >=50 行且缺少中文 doc：完整 71 项

本清单严格复现第 2.3 节风险词口径，并限定：非 private（含 trait 方法）、函数长度 `end_line - name_line + 1 >= 50`、函数自身中文 `///` 文档注释未命中。运行下方离线脚本得到：`functions_total=3468`、`public_total=2122`、`risk_public_total=944`、`risk_public_missing_chinese_doc_len_ge_50=71`、`parse_errors=0`。因此 71 项的“当前是否中文 doc”均为“否”；这里保留该列，便于后续批次复跑后直接对比。

| 文件 | 起始行 | 函数名 | 长度 | 当前中文 doc |
|---|---:|---|---:|---|
| `src/modules/admin/application/agents.rs` | 139 | `reset_admin_agent_password` | 54 | 否 |
| `src/modules/admin/application/convert.rs` | 52 | `create_admin_convert_pair` | 57 | 否 |
| `src/modules/admin/application/convert.rs` | 110 | `update_admin_convert_pair` | 105 | 否 |
| `src/modules/admin/application/market_feed.rs` | 145 | `reload_admin_market_feed_config` | 68 | 否 |
| `src/modules/admin/application/new_coin.rs` | 309 | `update_admin_new_coin_unlock_fee_rule` | 51 | 否 |
| `src/modules/admin/application/new_coin.rs` | 503 | `upsert_admin_new_coin_convert_rule` | 51 | 否 |
| `src/modules/admin/application/risk_security.rs` | 66 | `create_admin_risk_rule` | 50 | 否 |
| `src/modules/admin/application/users.rs` | 132 | `recharge_admin_user_wallet` | 52 | 否 |
| `src/modules/admin/application/wallet_assets.rs` | 306 | `create_admin_deposit_network_config` | 54 | 否 |
| `src/modules/admin/application/wallet_assets.rs` | 361 | `update_admin_deposit_network_config` | 51 | 否 |
| `src/modules/admin/application/wallet_assets.rs` | 465 | `create_admin_deposit_address_pool` | 56 | 否 |
| `src/modules/admin/application/wallet_assets.rs` | 522 | `create_admin_deposit_address_pool_batch` | 58 | 否 |
| `src/modules/admin/application/wallet_assets.rs` | 581 | `update_admin_deposit_address_pool` | 60 | 否 |
| `src/modules/admin/infrastructure/new_coin.rs` | 654 | `apply_admin_new_coin_distribution_allocation_in_tx` | 55 | 否 |
| `src/modules/admin/infrastructure/system_config.rs` | 568 | `upsert_admin_upload_config_in_tx` | 51 | 否 |
| `src/modules/admin/infrastructure/wallet_assets.rs` | 328 | `ensure_asset_has_no_references_in_tx` | 71 | 否 |
| `src/modules/admin/infrastructure/wallet_assets.rs` | 400 | `list_admin_wallet_accounts` | 51 | 否 |
| `src/modules/admin/infrastructure/wallet_assets.rs` | 452 | `list_admin_wallet_ledger` | 53 | 否 |
| `src/modules/admin/infrastructure/wallet_assets.rs` | 651 | `list_admin_deposit_address_pool` | 54 | 否 |
| `src/modules/admin/service/convert.rs` | 28 | `validate_convert_pair_values` | 57 | 否 |
| `src/modules/agent/infrastructure.rs` | 30 | `insert_agent_business_commission_in_tx` | 78 | 否 |
| `src/modules/auth/application.rs` | 410 | `login_user_with_optional_two_factor` | 51 | 否 |
| `src/modules/auth/infrastructure.rs` | 598 | `verify_registration_email_code_in_tx` | 50 | 否 |
| `src/modules/auth/infrastructure.rs` | 649 | `prepare_referral_binding_in_tx` | 56 | 否 |
| `src/modules/convert/application.rs` | 67 | `create_convert_quote` | 81 | 否 |
| `src/modules/kyc/application.rs` | 77 | `create_user_kyc_submission_in_tx` | 67 | 否 |
| `src/modules/kyc/domain.rs` | 136 | `validate_kyc_submission` | 102 | 否 |
| `src/modules/loan/application.rs` | 162 | `create_loan_order_use_case` | 85 | 否 |
| `src/modules/loan/application.rs` | 474 | `repay_loan_order_use_case` | 53 | 否 |
| `src/modules/loan/infrastructure.rs` | 634 | `apply_loan_wallet_freeze` | 55 | 否 |
| `src/modules/margin/application.rs` | 96 | `open_margin_position` | 125 | 否 |
| `src/modules/margin/application.rs` | 482 | `get_margin_position_risk_snapshot` | 50 | 否 |
| `src/modules/margin/application.rs` | 533 | `transfer_margin_funds` | 102 | 否 |
| `src/modules/margin/application.rs` | 718 | `close_margin_position` | 76 | 否 |
| `src/modules/margin/infrastructure.rs` | 111 | `transfer_spot_to_margin_wallets` | 75 | 否 |
| `src/modules/margin/infrastructure.rs` | 187 | `transfer_margin_to_spot_wallets` | 75 | 否 |
| `src/modules/margin/infrastructure.rs` | 1239 | `debit_margin_position_open_collateral` | 101 | 否 |
| `src/modules/margin/infrastructure.rs` | 1359 | `credit_margin_position_amount` | 70 | 否 |
| `src/modules/new_coin/domain.rs` | 190 | `apply_unlock_rule` | 102 | 否 |
| `src/modules/new_coin/domain.rs` | 293 | `calculate_unlock_fee` | 51 | 否 |
| `src/modules/prediction/infrastructure.rs` | 242 | `create_order_in_tx` | 113 | 否 |
| `src/modules/prediction/infrastructure.rs` | 356 | `settle_market_in_tx` | 134 | 否 |
| `src/modules/prediction/infrastructure.rs` | 1149 | `apply_wallet_prediction_open` | 75 | 否 |
| `src/modules/prediction/infrastructure.rs` | 1225 | `apply_wallet_prediction_settlement` | 63 | 否 |
| `src/modules/prediction/infrastructure.rs` | 1289 | `apply_wallet_prediction_refund` | 73 | 否 |
| `src/modules/quick_recharge/application.rs` | 113 | `create_user_quick_recharge_order` | 83 | 否 |
| `src/modules/quick_recharge/application.rs` | 197 | `save_admin_quick_recharge_config` | 73 | 否 |
| `src/modules/quick_recharge/application.rs` | 271 | `test_admin_quick_recharge_config` | 63 | 否 |
| `src/modules/quick_recharge/infrastructure.rs` | 213 | `create_gmpay_order_with_name` | 81 | 否 |
| `src/modules/seconds_contract/application.rs` | 235 | `open_order` | 140 | 否 |
| `src/modules/seconds_contract/application.rs` | 389 | `settle_order` | 84 | 否 |
| `src/modules/spot/application.rs` | 170 | `create_spot_order_with_events` | 85 | 否 |
| `src/modules/spot/application.rs` | 439 | `resolve_spot_order_execution_price` | 53 | 否 |
| `src/modules/spot/application.rs` | 619 | `settle_spot_fill` | 145 | 否 |
| `src/modules/spot/infrastructure.rs` | 624 | `insert_spot_trade` | 55 | 否 |
| `src/modules/spot/infrastructure.rs` | 894 | `insert_spot_order_in_tx` | 77 | 否 |
| `src/modules/spot/infrastructure.rs` | 1182 | `remaining_spot_fill_reservation_before_trade_in_tx` | 53 | 否 |
| `src/modules/spot/infrastructure.rs` | 1469 | `apply_spot_wallet_freeze` | 58 | 否 |
| `src/modules/spot/infrastructure.rs` | 1565 | `apply_spot_wallet_settlement_leg` | 60 | 否 |
| `src/modules/spot/service.rs` | 252 | `create_order` | 72 | 否 |
| `src/modules/user/application.rs` | 199 | `bind_user_referral_code` | 78 | 否 |
| `src/modules/user/application.rs` | 403 | `change_user_password` | 55 | 否 |
| `src/modules/user/application.rs` | 600 | `reset_user_fund_password` | 52 | 否 |
| `src/modules/wallet/application.rs` | 88 | `get_or_assign_deposit_address` | 50 | 否 |
| `src/modules/wallet/application.rs` | 591 | `create_withdrawal_request` | 84 | 否 |
| `src/modules/wallet/infrastructure.rs` | 564 | `save_account_with_ledger_async` | 50 | 否 |
| `src/modules/wallet/infrastructure.rs` | 1064 | `reserve_withdrawal_request` | 78 | 否 |
| `src/modules/wallet/infrastructure.rs` | 1233 | `release_withdrawal_in_tx` | 80 | 否 |
| `src/modules/wallet/infrastructure.rs` | 1357 | `confirm_withdrawal_in_tx` | 64 | 否 |
| `src/modules/wallet/infrastructure.rs` | 1579 | `observe_deposit_event` | 92 | 否 |
| `src/modules/wallet/infrastructure.rs` | 1672 | `reverse_deposit_event` | 70 | 否 |

### 4.5 可复现离线 AST 统计脚本

受 trellis-research 写入边界约束，本次未创建 `scripts/audit_backend_docs.rs` 或 `tests/support` 文件；完整可执行脚本内嵌如下。脚本只读取 `<repo>/src/**/*.rs`，使用 `syn 2` span 统计函数起止行并输出上述 71 项 Markdown 表。代码块顶部包含临时 Cargo 项目依赖与运行命令。

```rust
//! 可复现的 Rust 后端中文文档审计脚本。
//!
//! 运行方式（不会改项目文件）：
//! 1. `mkdir -p /tmp/audit_backend_docs/src`
//! 2. 将本代码块保存为 `/tmp/audit_backend_docs/src/main.rs`
//! 3. 将下方 Cargo.toml 保存为 `/tmp/audit_backend_docs/Cargo.toml`
//! 4. `cargo run --offline --quiet --manifest-path /tmp/audit_backend_docs/Cargo.toml -- "$PWD"`
//!
//! Cargo.toml：
//! ```toml
//! [package]
//! name = "audit-backend-docs"
//! version = "0.1.0"
//! edition = "2021"
//!
//! [dependencies]
//! proc-macro2 = { version = "1", features = ["span-locations"] }
//! quote = "1"
//! regex = "1"
//! syn = { version = "2", features = ["full", "visit"] }
//! walkdir = "2"
//! ```

use proc_macro2::Span;
use quote::ToTokens;
use regex::Regex;
use std::{env, fs, path::PathBuf};
use syn::{
    Attribute, Expr, ImplItemFn, ItemFn, Lit, Meta, TraitItemFn, Visibility,
    spanned::Spanned,
    visit::{self, Visit},
};
use walkdir::WalkDir;

#[derive(Debug)]
struct FunctionAudit {
    path: String,
    name: String,
    visibility: String,
    name_line: usize,
    end_line: usize,
    has_chinese_doc: bool,
}

#[derive(Default)]
struct Collector {
    path: String,
    functions: Vec<FunctionAudit>,
}

fn visibility(value: &Visibility) -> String {
    match value {
        Visibility::Public(_) => "pub".into(),
        Visibility::Restricted(restricted) => {
            format!("pub({})", restricted.path.to_token_stream())
        }
        Visibility::Inherited => "private".into(),
    }
}

fn contains_chinese(value: &str) -> bool {
    value
        .chars()
        .any(|ch| matches!(ch as u32, 0x3400..=0x9fff))
}

fn has_chinese_doc(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("doc") {
            return false;
        }
        match &attr.meta {
            Meta::NameValue(value) => match &value.value {
                Expr::Lit(expr) => match &expr.lit {
                    Lit::Str(doc) => contains_chinese(&doc.value()),
                    _ => false,
                },
                _ => false,
            },
            _ => false,
        }
    })
}

impl Collector {
    fn push(
        &mut self,
        name: &syn::Ident,
        visibility_value: &Visibility,
        attrs: &[Attribute],
        span: Span,
    ) {
        self.functions.push(FunctionAudit {
            path: self.path.clone(),
            name: name.to_string(),
            visibility: visibility(visibility_value),
            name_line: name.span().start().line,
            end_line: span.end().line,
            has_chinese_doc: has_chinese_doc(attrs),
        });
    }
}

impl<'ast> Visit<'ast> for Collector {
    fn visit_item_fn(&mut self, item: &'ast ItemFn) {
        self.push(&item.sig.ident, &item.vis, &item.attrs, item.span());
        visit::visit_item_fn(self, item);
    }

    fn visit_impl_item_fn(&mut self, item: &'ast ImplItemFn) {
        self.push(&item.sig.ident, &item.vis, &item.attrs, item.span());
        visit::visit_impl_item_fn(self, item);
    }

    fn visit_trait_item_fn(&mut self, item: &'ast TraitItemFn) {
        // Trait 方法属于上下文公开契约，统一按公开职责统计。
        self.push(
            &item.sig.ident,
            &Visibility::Public(syn::token::Pub::default()),
            &item.attrs,
            item.span(),
        );
        visit::visit_trait_item_fn(self, item);
    }
}

fn main() {
    let root = PathBuf::from(env::args().nth(1).unwrap_or_else(|| ".".into()));
    let risk_name = Regex::new(concat!(
        "(?i)(wallet|balance|ledger|withdraw|deposit|settle|settlement|liquidat|",
        "margin|collateral|interest|loan|repay|order|trade|fill|price|fee|commission|",
        "recharge|convert|unlock|fund|auth|login|password|token|session|permission|scope|",
        "two_factor|totp|security|risk|idemp|transaction|in_tx|referral|kyc|audit|",
        "reserve|freeze|unfreeze|credit|debit)"
    ))
    .expect("valid risk regex");

    let mut functions = Vec::new();
    let mut parse_errors = Vec::new();

    for entry in WalkDir::new(root.join("src"))
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if !path.is_file()
            || path.extension().and_then(|value| value.to_str()) != Some("rs")
            || path.components().any(|part| part.as_os_str() == "target")
        {
            continue;
        }
        let relative = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();
        let source = match fs::read_to_string(path) {
            Ok(source) => source,
            Err(error) => {
                parse_errors.push(format!("{relative}: {error}"));
                continue;
            }
        };
        match syn::parse_file(&source) {
            Ok(file) => {
                let mut collector = Collector {
                    path: relative,
                    ..Collector::default()
                };
                collector.visit_file(&file);
                functions.extend(collector.functions);
            }
            Err(error) => parse_errors.push(format!("{relative}: {error}")),
        }
    }

    functions.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.name_line.cmp(&right.name_line))
    });
    let public = functions
        .iter()
        .filter(|item| item.visibility != "private")
        .collect::<Vec<_>>();
    let risk_public = public
        .iter()
        .copied()
        .filter(|item| risk_name.is_match(&item.name))
        .collect::<Vec<_>>();
    let missing_long = risk_public
        .iter()
        .copied()
        .filter(|item| {
            let length = item.end_line - item.name_line + 1;
            length >= 50 && !item.has_chinese_doc
        })
        .collect::<Vec<_>>();

    println!("files_parse_errors={}", parse_errors.len());
    println!("functions_total={}", functions.len());
    println!("public_total={}", public.len());
    println!("risk_public_total={}", risk_public.len());
    println!("risk_public_missing_chinese_doc_len_ge_50={}", missing_long.len());
    println!("| 文件 | 起始行 | 函数名 | 长度 | 当前中文 doc |");
    println!("|---|---:|---|---:|---|");
    for item in missing_long {
        let length = item.end_line - item.name_line + 1;
        println!(
            "| `{}` | {} | `{}` | {} | {} |",
            item.path,
            item.name_line,
            item.name,
            length,
            if item.has_chinese_doc { "是" } else { "否" }
        );
    }

    if !parse_errors.is_empty() {
        eprintln!("parse errors:");
        for error in parse_errors {
            eprintln!("- {error}");
        }
        std::process::exit(2);
    }
}
```

### 5. routes 层审计

#### 5.1 已达成

- 全部 routes 文件没有 `sqlx::query*`、`QueryBuilder` 或 `.begin(`；这比 PRD 初始描述显著改善。
- wallet handlers 多数为鉴权→取 ID/pool→application→Json，例如 `src/modules/wallet/routes.rs:84-197`；spot 路由创建/取消/成交亦主要调用 application（`src/modules/spot/routes.rs:59-195`）；margin 风险与仓位动作委托 application（`src/modules/margin/routes.rs:278-385`）。
- admin 的 108 个 handler 多数已很短，虽然 router 聚合函数 209 行，但具体 handler 主要转调 use case（如 `src/modules/admin/routes.rs:774-930`）。

#### 5.2 仍有越权/职责混杂

| 优先级 | 证据 | 结论 |
|---|---|---|
| P1 | `src/modules/events/routes.rs:8-18,58-99` | route 直接导入 `OutboxRecordRow`、`list_inbox_records`、`list_outbox_records`、`requeue_outbox_dead_letter`，直接构造 JSON shape，并自己提取 pool；应加 application use cases + presentation responses。 |
| P1 | `src/modules/auth/routes.rs:386-539` | Turnstile response DTO、cookie/IP 提取、环境读取、策略分支、Reqwest 请求、错误映射均在 routes；这是 provider adapter + application policy，不是 transport-only handler。 |
| P2 | `src/modules/admin/routes.rs:115,746-761`、`src/modules/user/routes.rs:27,99-109` | routes 直接调用 `admin::infrastructure::multipart_file_input`；multipart 字节读取可留 presentation adapter，但不应从 routes 依赖 infrastructure。 |
| P2 | `src/modules/countries/routes.rs:13-18`、`platform/routes.rs:13-18` | route 通过 `service::mysql_pool` 获取连接；service 文件自身依赖 `AppState` + SQLx（如 `countries/service.rs:6-23`），这是 transport plumbing 被放进 service 的层名漂移。 |
| P2 | `src/modules/admin/routes.rs:193-401` | 81 个 `.route(...)` 聚合在一个 209 行函数，横跨国家、新闻、用户、KYC、钱包、风控、行情、SMTP、上传、新币、代理、闪兑；具体 handler 薄，但路由注册职责仍应按 admin 子域拆 router 并 merge。 |

### 6. 名义层、空壳层与反向依赖

#### 6.1 纯空壳/marker（14 个）

- **domain 空壳（4）**：`src/modules/earn/domain.rs:1-4`、`prediction/domain.rs:1-4`、`quick_recharge/domain.rs:1-4`、`seconds_contract/domain.rs:1-4`。
- **repository 仅 marker（7）**：`countries/repository.rs:1-11`、`kyc/repository.rs:1-11`、`loan/repository.rs:1-11`、`margin/repository.rs:1-11`、`news/repository.rs:1-11`、`platform/repository.rs:1-11`、`security/repository.rs:1-11`。
- **service 仅 marker（2）**：`news/service.rs:1-11`、`security/service.rs:1-11`。
- **presentation 空壳（2）**：`risk/presentation.rs:1-4`、`security/presentation.rs:1-4`。
- `events/application.rs:1-23` 只有一个私有 WebSocket 鉴权用例，仍带“迁移锚点”文案，是**接近空壳的 partial**，而大部分事件编排留在 1,935 行 service。

> 上述纯 layer 文件总数为 15（security 同时命中 repository/service/presentation）；后续验收应以 15 为 baseline。

#### 6.2 “有代码但层义不纯”证据

- repository 规范要求不执行 SQL（`.trellis/spec/backend/directory-structure.md:58-66`），但 `src/modules/new_coin/repository.rs:11,94-199` 持有 `Pool<MySql>` 并直接执行 SQL；这是 concrete infrastructure 落在 repository。
- domain 不应依赖 transport/SQLx/Redis，但 `platform/domain.rs:8` 依赖 presentation request；`security/domain.rs:153,178` derive `sqlx::FromRow`；`convert/domain.rs:202-212` 直接把 `sqlx::Error`/`redis::RedisError` 转领域 repository error。
- service 不应 own SQL/provider/transport，但 `events/service.rs:6-45` 同时依赖 presentation、infrastructure、Axum WebSocket、Lapin、SQLx；`auth/service.rs:24,44-60` 直接持有 Redis manager（可视为 application service，但与当前 spec 的纯 service 定义冲突）。
- service→application/presentation 反向依赖：`margin/service.rs:12-13` 依赖 application 与 presentation；`spot/service.rs:13`、`earn/service.rs:10`、`seconds_contract/service.rs:10` 等依赖 presentation DTO。这使层图不是单向 domain/repository→service→application→presentation。
- presentation 直接绑定 SQL row：大量 `#[derive(sqlx::FromRow)]`，例 `admin/presentation.rs:76`、`margin/presentation.rs:146`、`prediction/presentation.rs:146`、`wallet/presentation.rs:102`。这减少 mapping，但与“DTO 与数据库 row 分离”的目标并不一致。

#### 6.3 架构 guard 只验证“存在”，不验证“有实现/依赖方向”

- `tests/backend_architecture.rs:3-20` 仅断言六个文件路径存在；空文件/marker 也会通过。
- route service guard 是符号白名单（`tests/backend_architecture.rs:64-73,168-223`），未禁止 route 直接依赖 infrastructure；因此 events/admin/user 上述泄漏不会被捕获。
- 建议新增依赖方向与实质性 guard：domain 禁 Axum/SQLx/Redis/presentation，repository 禁 concrete SQL，routes 禁 infrastructure/Reqwest/SQLx，纯 marker 文件数不得增加并逐批归零。

### 7. 超大文件、超长函数与职责混杂

#### 7.1 文件

- **>1,000 行：15 个；>1,500 行：9 个；>2,000 行：4 个；>500 行：54 个。**
- 最大文件及职责：
  - `src/modules/market/infrastructure.rs` **2,871** 行：约 224 个 AST 函数，provider adapter、解析、REST fallback、Kline/行情持久化集中。
  - `src/modules/wallet/infrastructure.rs` **2,605** 行：约 91 个 AST 函数、46 个 SQL query、16 个 QueryBuilder，混合账户、流水、充币地址、提现状态机、链事件和收益查询。
  - `src/modules/spot/infrastructure.rs` **2,251** 行：约 85 函数、41 个 SQL query、14 个 QueryBuilder，混合旧 repository adapter、订单查询、撮合/结算持久化与钱包写入。
  - `src/modules/admin/presentation.rs` **2,098** 行：163 个 DTO 类型，横跨所有 admin 子域；易造成重复 request/response 与 OpenAPI 漂移。
  - `src/modules/events/service.rs` **1,935** 行：100 个函数，WebSocket、outbox、inbox、RabbitMQ producer/consumer、retry 全混合。
  - `src/modules/margin/{infrastructure,application}.rs` **1,786/1,637** 行；`admin/routes.rs` **1,633** 行；`spot/application.rs` **1,518** 行；`prediction/infrastructure.rs` **1,440** 行。

#### 7.2 超长函数（>100 行，共 26 个）

| 行数 | 证据 | 函数 |
|---:|---|---|
| 264 | `src/main.rs:21-284` | `main`（连接、state、12 类 worker 启动、server 绑定混合） |
| 209 | `src/modules/admin/routes.rs:193-401` | `routes` |
| 182 | `src/workers/margin_liquidation.rs:544-725` | `liquidate_cross_account` |
| 160 | `src/modules/new_coin/infrastructure.rs:216-375` | `release_due_paid_unlock` |
| 159/159 | `src/modules/spot/application.rs:1053-1211,1213-1371` | triggered buy/sell 执行，结构高度对称 |
| 158 | `src/workers/unlock_scanner.rs:228-385` | `release_due_unlock_by_id` |
| 154 | `src/modules/quick_recharge/application.rs:372-525` | `handle_gmpay_notify` |
| 152 | `src/workers/wallet_chain.rs:98-249` | `run_once_with_gateway` |
| 145 | `src/modules/spot/application.rs:619-763` | `settle_spot_fill` |
| 140 | `src/modules/seconds_contract/application.rs:235-374` | `open_order` |
| 134 | `src/modules/prediction/infrastructure.rs:356-489` | `settle_market_in_tx` |
| 133 | `src/modules/wallet/application.rs:277-409` | `calculate_return_history` |
| 125 | `src/modules/margin/application.rs:96-220` | `open_margin_position` |
| 117 | `src/modules/admin/application/system_config.rs:302-418` | `save_admin_upload_config` |
| 116 | `src/modules/events/infrastructure.rs:368-483` | `claim_message` |
| 113 | `src/modules/prediction/infrastructure.rs:242-354` | `create_order_in_tx` |
| 112/112 | `src/modules/wallet/infrastructure.rs:1808-1919`; `market/infrastructure.rs:1449-1560` | 收益资产活动 / 行情解析 |
| 106 | `src/workers/margin_liquidation.rs:240-345` | worker 单轮编排 |
| 105 | `src/modules/admin/application/convert.rs:110-214` | 更新闪兑对 |
| 102/102/102 | `kyc/domain.rs:136-237`; `new_coin/domain.rs:190-291`; `margin/application.rs:533-634` | KYC 校验 / 解锁规则 / 保证金划转 |
| 101/101 | `admin/application/new_coin.rs:401-501`; `margin/infrastructure.rs:1239-1339` | 新币分配 / 开仓抵押扣款 |

### 8. 重复 DTO / SQL / 服务逻辑

#### 8.1 SQL 与资金写入

- 663 个归一化 SQL 字符串中，**61 组/177 次**是精确重复，**42 组跨文件**。
- `UPDATE wallet_accounts SET available = ? ...` 出现 **17 次**，分散于 earn/seconds/loan/new_coin/quick_recharge/margin/convert/admin/workers；钱包行锁 `SELECT available, frozen, locked ... FOR UPDATE` 出现 **14 次**，见 `workers/earn_auto_redemption.rs:339`、`prediction/infrastructure.rs:1378`、`spot/infrastructure.rs:1456`、`wallet/infrastructure.rs:2358` 等。
- available/frozen 更新精确重复 8 次，例 loan `src/modules/loan/infrastructure.rs:634-688`、prediction `src/modules/prediction/infrastructure.rs:1149-1223`、spot `src/modules/spot/infrastructure.rs:1469-1526`。这些路径均手写余额+流水，钱包 invariant 漂移风险高。
- auth/user 对验证码、邀请码、referral 的 SQL 成对重复：
  - cooldown/supersede/lock/attempt/verified：`auth/infrastructure.rs:785-936` 对 `user/infrastructure.rs:572-735`；
  - invite/agent/referral 锁与写：`auth/infrastructure.rs:1003-1085` 对 `user/infrastructure.rs:314-429`。
- unlock release 两套近似实现：worker `src/workers/unlock_scanner.rs:228-385` 与 new_coin repository implementation `src/modules/new_coin/infrastructure.rs:216-375`；两者均锁仓、钱包 available/locked、双流水和状态更新，行为漂移风险高。

#### 8.2 DTO

- `admin/presentation.rs` 有 163 个 DTO，OpenAPI 目录又维护同形 DTO。精确字段形状示例：
  - `status + reason` 形状出现 15 次（admin/news/earn/margin/seconds/OpenAPI）；
  - `limit + offset` 形状出现 8 次；
  - OpenAPI 和生产 DTO 成对重复：`SavePlatformBrandRequest`（`src/openapi/system_config.rs:31`、`src/modules/platform/presentation.rs:11`）、`ObserveDepositRequest`（`src/openapi/wallet.rs:96`、`src/modules/wallet/presentation.rs:164`）、security policy/payment structs（`src/openapi/user_security.rs:18,58`、`src/modules/security/domain.rs:94,123`）。
- repository row 与 response 也常完全同形：PredictionOrder（`prediction/repository.rs:86` vs `presentation.rs:230`）、QuickRechargeOrder（`quick_recharge/repository.rs:123` vs `presentation.rs:131`）、多组 new_coin read/response。若刻意直映射，应统一一个 transport/storage ownership 策略；否则字段变更易双改漏改。

#### 8.3 重复不应“一刀切”抽象

- 通用 `status/reason` DTO 语义可能不同，不建议仅因字段相同就全局合并。
- **优先抽象资金原语、验证码仓储、邀请关系仓储、unlock release 用例**，因为它们跨文件复制的不是形状，而是状态机/事务不变量。

### 9. PRD `[x]` 与代码真实状态

| PRD 项 | 复核 | 结论 |
|---|---|---|
| 52：目录规范已记录 | `.trellis/spec/backend/directory-structure.md:27-99` 有布局、职责、中文注释和测试规则。 | **准确**，但 `.trellis/spec/backend/index.md:17-20` 仍把 directory/database/error/quality 标成 `To fill`，索引状态过期。 |
| 53：所有 context 有六层 anchor | guard 的目录存在测试通过；22 context × 6 文件均存在。 | **字面准确、实质不足**：至少 15 个 layer 纯 marker/空壳，不能解读为“各层已落地”。 |
| 54：内嵌测试体全部外移 | `src/openapi/auth.rs:394-443` 仍内嵌测试。 | **不准确**。 |
| 55：层 anchor 有中文职责注释 | 空壳及多数层文件顶部确有中文层说明。 | **字面准确，但覆盖目标过弱**：函数中文 doc 仅 4.7%，公开职责中文 doc 6.5%。 |
| 56：guard 防 missing layer + 新 inline test | test 文件确有两类 guard（`tests/backend_architecture.rs:12-60`）。 | **guard 存在但当前红**；另只测存在，不测空壳/依赖方向。完成度不足。 |
| 57：现有 backend tests still pass | 本次 `backend_architecture` 2/4 失败。 | **当前不准确**；未重跑全量 suite，已有直接反例足够否定“仍全过”。 |
| 58：fmt/check 通过 | 本次 `cargo fmt --manifest-path Cargo.toml --check` 与 `cargo check --manifest-path Cargo.toml --all-targets` 均通过。 | **当前准确**。 |
| 59：countries/platform/news real extraction | route→application→infrastructure 基本存在；但 countries/platform repository 是 marker，service 只取 pool，news repository/service 纯 marker。 | **部分完成**；“real DDD layer extraction”若要求 repository/service 实质化则不足。 |
| 60：risk/security pure domain extraction | `risk/domain.rs` 有纯规则；security domain 有 TOTP/policy 规则。 | **部分完成**：`security/domain.rs:153,178` 绑定 `sqlx::FromRow`，security repository/service/presentation 均空壳，非完整纯边界。 |
| 61：security DB 在 infrastructure、编排在 application | DB/verification 主路径确在相应文件。 | **基本准确**，但 repository/service 空壳，application 直接调用 infrastructure，属于两层拆分而非完整端口适配器。 |
| 62：auth repository/service/infrastructure | `auth/repository.rs` 有 trait，`auth/infrastructure.rs` 有实现，service 有 token/session。 | **基本准确**；service 直接持有 Redis 是当前约定下的折中，应在 spec 明确。 |
| 63–74：auth/user 路由迁移 | routes 无 SQL，相关流程主要转 application。 | **总体准确**；但 auth route 的 Turnstile provider 工作流是新增/遗留越权，不在这些验收文字覆盖内。 |
| 75–77：KYC 分层 | presentation/domain/service/infrastructure 有真实内容。 | **基本准确**；repository 仍 marker。 |
| 78–80：wallet read/deposit/withdraw | routes 薄，SQL 在 infrastructure，事务在 application/infrastructure。 | **准确到已列流程**；但 wallet infrastructure 2,605 行，资金原语跨 context 重复，未达到全 context 清晰 ownership。 |
| 81、87–89：spot 列表/取消/价格/幂等 | route 已委托 application，相关 SQL 在 infrastructure、规则在 service。 | **准确到已列切片**；triggered buy/sell、settlement 仍超长且 service→presentation 耦合。 |
| 82–86：margin 指定切片 | route 已委托 application；持久化在 infrastructure。 | **准确到已列切片**；repository 仍 marker、service→application/presentation 反向依赖，application/infrastructure 仍各 >1,600 行。 |

### 10. 可执行分批方案（优先高风险、行为不变）

#### Batch 0 — 恢复审计基线（P0，0.5 天）

- 只移动 `src/openapi/auth.rs:394-443` 测试体到 `tests/unit_src`，不改 API/逻辑；让 architecture guard 全绿。
- 固化审计脚本/测试：函数计数、P0 中文 doc、routes 禁 SQL/Reqwest/infrastructure、纯 marker baseline。
- **验收门槛**：`backend_architecture` 4/4；fmt/check；公开路由 OpenAPI snapshot 不变；`src` 不存在 `mod tests {`。

#### Batch 1 — 只补 P0/P1 中文职责合同（P0，1–2 天）

- 不重构函数体，只给上述 P0 资金/结算/强平/鉴权入口加中文 `///`；同时给 worker public entrypoint、infra auth/secrets 加职责说明。
- **量化门槛**：本报告 P0 表 15 组入口 100% 有中文 doc；风险公开函数中长度 ≥50 行的 71 个，中文 doc 覆盖先达 **100%**；禁止用“设置变量/调用函数”类机械注释；API/SQL/测试行为零变更。

#### Batch 2 — 统一钱包写入端口，先迁一个最小切片（P0，3–5 天）

- 在 wallet repository/application 定义 transaction-scoped `lock account / apply bucket transfer / append ledger` 端口；先迁 prediction 的 open/settle/refund 三条资金路径，保留 SQL 与 ledger change_type 完全一致。
- 之后按 loan→spot→margin→earn/seconds/quick_recharge/workers 迁移，避免一次全仓替换。
- **验收门槛**：迁移切片 SQL snapshot/ledger 行数、change_type、ref_type/ref_id、amount/balance_after 完全一致；幂等重放不新增流水；失败时余额/流水/订单全部 rollback；每批精确重复钱包 SQL 组数下降，不新增 cross-context `UPDATE wallet_accounts`。

#### Batch 3 — Prediction + Seconds 高风险事务拆分（P0，3–5 天）

- 把空 `domain.rs` 变成真实领域规则：outcome/status transition、payout/refund policy、idempotency request match；infrastructure 只保留 row lock/CRUD；application 管 begin/commit。
- **门槛**：`prediction create_order_in_tx`、`settle_market_in_tx`、seconds `open_order` 均降至建议 ≤80 行；domain 无 SQLx/Redis/Axum；重复提交/settle/refund 与资产精度测试全过。

#### Batch 4 — Auth/User 验证码与 referral 仓储去重（P1，2–4 天）

- 提取共享 infrastructure repository，不改 SQL 文本/锁语义；auth registration/password-reset 与 user bind/reset 复用同一端口。
- Turnstile 从 route 迁 presentation DTO + provider adapter + application policy，route 只传 header/token。
- **门槛**：auth/user 上述成对 SQL 每组只剩一个 owner；cooldown、attempt、expiry、supersede、verified、invite usage count、ancestor disabled 测试一致；auth routes 不含 `reqwest`/env policy/provider DTO。

#### Batch 5 — Workers 与 unlock release（P1，3–5 天）

- 合并 unlock worker 与 new_coin release 的单一 application use case；worker 只扫描并调用。
- 将 wallet_chain、margin_liquidation、seconds settlement 的 SQL/事务逐步下沉对应 context application/infrastructure；workers 只做 schedule/batch/retry/metrics。
- **门槛**：worker 文件不直接写 `wallet_accounts`/`wallet_ledger`；同一 unlock 重放零二次入账；失败后可重试；事件只在 commit 成功后发布；每轮 batch limit/failed/skipped 语义不变。

#### Batch 6 — Events/Auth/Admin 结构减重与依赖 guard（P1/P2，持续）

- events service 拆 websocket/outbox/inbox/provider；events routes 加 application/presentation。
- admin routes 按 agents/system_config/wallet_assets/new_coin 等子 router merge；admin presentation DTO 同步按子域拆文件，稳定 re-export。
- main 抽 `spawn_background_workers`/每 worker 启动函数。
- **门槛**：routes 0 infrastructure/Reqwest/SQL；service 不依赖 application；domain 0 transport/storage SDK；单文件建议 ≤1,000 行、单函数建议 ≤80 行（例外需中文说明）；API 路径和 OpenAPI schema 不变。

#### Batch 7 — 空壳层逐批归零（P2）

- 不为“满六层”而保留无意义 marker。每个 context 选择：填充真实 contract/rule，或在架构规范允许 optional layer 后删除空壳；不要继续假装已完成。
- **门槛**：纯 marker/空壳 baseline 从 15 单调下降到 0；guard 验证依赖方向和实质，不再只测文件存在。

### 11. 建议总体验收门槛

1. **质量绿线**：fmt、all-target check、backend architecture、最贴近 context 测试全绿；全量测试若受外部 DB/端口限制，必须分清环境失败与行为失败并记录。
2. **routes**：0 raw SQL/transaction，0 direct infrastructure/provider HTTP；handler 建议 ≤25 行（router 注册函数例外，但按子域拆分）。
3. **风险注释**：所有公开资金/结算/强平/鉴权/幂等/事务入口必须有中文 `///`；长度 ≥50 的风险公开函数 100%；普通函数不追求机械 100%。
4. **复杂度**：新增函数 ≤80 行；遗留 >100 行数量每批单调下降（baseline 26），不得新增；新增文件建议 ≤1,000 行。
5. **DDD 依赖**：domain 禁 Axum/SQLx/Redis/Mongo/Reqwest/presentation；repository contract 禁 concrete SQL；service 禁 application/routes；presentation 不承载资金决策。
6. **资金一致性**：每个余额变更同事务写 ledger；锁顺序可证明；幂等重放不二次记账；commit 前不发布不可撤销事件。
7. **重复**：精确跨文件 SQL 重复 baseline 42 组，每批不得上升；钱包 mutation、验证码、referral、unlock release 必须优先归一 owner。
8. **API 稳定**：路由、HTTP 状态、JSON 字段、OpenAPI schema、ledger `change_type/ref_type/ref_id` 与历史行为保持不变。

### 12. Files found

- `.trellis/tasks/06-27-backend-ddd-architecture-refactor/prd.md` — 当前 DDD 重构目标、42 个 `[x]` 验收项与 out-of-scope。
- `.trellis/spec/backend/directory-structure.md` — 六层职责、中文风险注释与独立测试约定。
- `.trellis/spec/backend/quality-guidelines.md` — routes、SQL/Redis 层位、事务与验证命令约束。
- `.trellis/spec/backend/{wallet-amount-precision,spot-orders,margin-trading-actions,seconds-contracts,prediction-markets,auth-sessions,user-authentication}.md` — 本次风险函数应遵守的资金、幂等、结算和鉴权合同。
- `tests/backend_architecture.rs` — 目录存在、内嵌测试和 route→service 白名单 guard；本次 2/4 失败。
- `src/modules/*/{domain,repository,service,application,infrastructure,presentation,routes}.rs` — 22 个 bounded context 的审计主体。
- `src/workers/*.rs` — 12 类后台作业与 module root，共 5,382 行。
- `src/infra/*.rs` — cross-context auth/email/mysql/mongo/redis/rabbitmq/secrets，共 444 行。
- `src/openapi/auth.rs` — 当前唯一静态命中的生产源码内嵌测试体。
- 本报告第 4.5 节内嵌 `audit-backend-docs` 完整离线 AST 脚本 — 可复现 3,468/2,122/944/71 的函数审计结果。

### 13. Related specs

- `.trellis/spec/backend/directory-structure.md`
- `.trellis/spec/backend/quality-guidelines.md`
- `.trellis/spec/backend/database-guidelines.md`
- `.trellis/spec/backend/error-handling.md`
- `.trellis/spec/backend/wallet-amount-precision.md`
- `.trellis/spec/backend/spot-orders.md`
- `.trellis/spec/backend/margin-trading-actions.md`
- `.trellis/spec/backend/seconds-contracts.md`
- `.trellis/spec/backend/prediction-markets.md`
- `.trellis/spec/backend/auth-sessions.md`
- `.trellis/spec/backend/user-authentication.md`
- `.trellis/spec/guides/cross-layer-thinking-guide.md`
- `.trellis/spec/guides/code-reuse-thinking-guide.md`

### 14. External references

- 无。本次是代码库内部静态审计；Rust 解析使用本机离线缓存中的 `syn 2`，未查询外部网站、版本文档或网络资料。

## Caveats / Not Found

- `task.py current --source` 返回 `Current task: (none)`；本报告使用用户明确给出的 active task 路径，没有写入其他位置。
- 本报告只写 research 文件，未修改生产代码、`scripts/`、`tests/`、PRD、规范或进度文档；可复现脚本按角色限制内嵌于第 4.5 节。
- 函数覆盖是静态注释命中率，不衡量注释正确性；“任一覆盖”尤其是宽松上限。
- 风险函数分类靠命名启发式，不是完整 taint/call-graph 分析；私有但高风险的强平/结算函数已人工补入 P0。
- SQL 重复只统计字符串字面量的精确归一化重复，动态 SQL、同义改写和封装内重复未计，实际重复不会更少。
- 未执行全量 `cargo test`；执行了最直接的架构测试并得到确定失败。`cargo fmt --manifest-path Cargo.toml --check` 与 `cargo check --manifest-path Cargo.toml --all-targets` 已通过。
- 未发现 routes 中原始 SQL query 或 transaction；“routes 仍有 SQL”的结论是 **0 处**，但存在 direct infrastructure/provider 越权，已单独列证据。
