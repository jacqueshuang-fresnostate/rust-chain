# Research: Rust 后端秒合约同用户多订单合同

- Query: Rust 后端是否限制同一用户并行持有多个 active/opened/pending 秒合约订单；检查迁移、仓储、应用服务、路由、结算 worker 与测试，并明确限制位置、移除或保留的风险与幂等合同。
- Scope: internal
- Date: 2026-08-08

## Findings

### 结论

Rust 后端当前**没有“同一用户只能有一个活动秒合约订单”的限制**。同一用户可以用不同的 `idempotency_key` 创建多个 `opened` 订单，包括同一产品、同一周期、同一方向或不同方向。并发请求可能因产品行锁及共享钱包行锁而串行等待，但不会因已有 `opened` 订单而被业务拒绝。

后端秒合约订单实际使用的持久化状态只有 `opened` 与 `settled`：

- `opened` 是订单插入时的状态（`migrations/0021_seconds_contracts.sql:30`，`src/modules/seconds_contract/infrastructure.rs:616-640`）。
- `settled` 是人工或 worker 结算后的状态（`src/modules/seconds_contract/infrastructure.rs:737-749`，`src/workers/seconds_contract_settlement.rs:378-389`）。
- `active` 属于秒合约**产品**状态，不是订单状态（`migrations/0021_seconds_contracts.sql:9`，`src/modules/seconds_contract/infrastructure.rs:530-549`）。
- 本上下文出现的 `pending` 是代理佣金记录状态，不是秒合约订单状态（`src/modules/agent/infrastructure.rs:86-104`）。

因此，如果移动端表现为只允许一个 active/pending 持仓，限制不在已检查的 Rust 秒合约后端中，更可能位于客户端状态建模、按钮禁用条件、订单数组被压成单个对象，或客户端复用了同一个幂等键。后端没有需要删除的单订单校验或单活动订单唯一索引。

### Files found

- `migrations/0021_seconds_contracts.sql` — 秒合约产品和订单主表；定义订单状态、用户幂等唯一键及基础索引。
- `migrations/0024_seconds_contract_entry_price.sql` — 增加开仓价和按状态/到期时间扫描的索引。
- `migrations/0025_seconds_contract_settlement_retry_at.sql` — 增加结算重试时间和 worker 扫描索引。
- `migrations/0066_seconds_contract_product_cycles.sql` — 增加产品周期及订单周期快照，不增加用户活动订单唯一约束。
- `migrations/0067_seconds_contract_order_settlement_price.sql` — 增加结算价快照。
- `migrations/0096_admin_time_ordered_list_indexes.sql` — 增加后台订单时间排序索引。
- `migrations/0003_assets_wallet_ledger_locks.sql` — 共享钱包账户唯一键、非负约束及钱包流水索引。
- `migrations/0028_agent_commission_source_id.sql` — 代理佣金按代理、来源类型、订单来源 ID 幂等。
- `src/modules/seconds_contract/domain.rs` — 当前仅为领域层锚点，没有“单活动订单”领域规则。
- `src/modules/seconds_contract/repository.rs` — 订单写入、钱包行与后台过滤结构；订单写入对象不携带活动订单门禁。
- `src/modules/seconds_contract/service.rs` — 请求规范化、幂等请求匹配、派彩及事件构建。
- `src/modules/seconds_contract/application.rs` — 开仓、人工结算、事务及事件编排；是确认并发/幂等行为的核心。
- `src/modules/seconds_contract/infrastructure.rs` — SQLx 查询、产品/订单/钱包行锁、插入及状态更新。
- `src/modules/seconds_contract/presentation.rs` — 用户开仓 DTO、订单响应及用户/后台列表查询 DTO。
- `src/modules/seconds_contract/routes.rs` — 用户与后台路由；处理器仅鉴权、解析并调用应用用例。
- `src/workers/seconds_contract_settlement.rs` — 批量扫描到期 `opened` 订单并逐单加锁结算。
- `src/main.rs` — 每个满足依赖条件的后端进程都会启动秒合约结算循环。
- `tests/seconds_contract_routes.rs` — 路由、钱包、事件、人工结算、幂等及产品禁用竞态集成测试。
- `tests/seconds_contract_settlement_worker.rs` — worker 结算、重试、批量前进及重复执行测试。
- `tests/unit_src/src_workers_seconds_contract_settlement_tests.rs` — 结算结果及批量上限单元测试。
- `src/modules/agent/infrastructure.rs` — 每个秒合约订单在同事务内按订单 ID 创建幂等佣金记录。

### 迁移与数据库约束

1. 订单表唯一约束只有 `(user_id, idempotency_key)`，不是 `(user_id, status)`、`(user_id, product_id, status)` 或任何“活动槽位”约束（`migrations/0021_seconds_contracts.sql:21-45`）。这允许同一用户存在任意数量的不同幂等键订单。
2. 后续所有秒合约订单迁移只增加开仓价、重试时间、周期、结算价和排序/扫描索引，没有新增单活动订单限制（`migrations/0024_seconds_contract_entry_price.sql:1-3`，`migrations/0025_seconds_contract_settlement_retry_at.sql:1-3`，`migrations/0066_seconds_contract_product_cycles.sql:24-34`，`migrations/0067_seconds_contract_order_settlement_price.sql:1-2`，`migrations/0096_admin_time_ordered_list_indexes.sql:14-15`）。
3. 数据库没有订单状态 `CHECK` 约束；应用路径只写 `opened`/`settled`，但手工 SQL 或未来代码可写入 worker 不识别的状态。这不是多订单限制，却是状态合同漂移风险。
4. `wallet_accounts` 以 `(user_id, asset_id)` 唯一并要求余额非负（`migrations/0003_assets_wallet_ledger_locks.sql:1-22`）。多个订单共享同一资产钱包，而不是共享一个“活动订单槽位”。
5. `wallet_ledger` 的 `(ref_type, ref_id)` 只有普通索引，没有唯一约束（`migrations/0003_assets_wallet_ledger_locks.sql:25-42`）；避免重复扣款/派奖主要依赖订单幂等键、订单行锁、状态检查和事务边界，不能删除这些应用层合同。

### 仓储与查询行为

1. `SecondsContractOrderInsert` 包含每个订单自己的用户、产品、方向、金额、周期、赔率、开仓价、幂等键和到期时间，没有“当前活动订单”字段或锁槽（`src/modules/seconds_contract/repository.rs:72-85`）。
2. 用户订单查询按 `user_id` 返回所有状态，按 `created_at DESC, id DESC` 排序；没有 `status = 'opened'`、分组、`LIMIT 1` 或每用户去重（`src/modules/seconds_contract/infrastructure.rs:371-394`）。
3. 后台查询的 `status` 只是可选筛选条件，同样不限制每个用户的行数（`src/modules/seconds_contract/infrastructure.rs:397-444`）。
4. 幂等查询只按 `(user_id, idempotency_key)` 定位订单（`src/modules/seconds_contract/infrastructure.rs:457-504`）。因此不同用户可使用同样的键；同一用户每个“新订单意图”必须使用新键，重试同一意图必须复用原键。
5. `insert_open_order` 直接插入一行 `opened` 订单，没有查询或计数该用户已有多少 `opened` 订单（`src/modules/seconds_contract/infrastructure.rs:616-640`）。

### 应用服务与并发开仓

开仓流程为：规范化请求 → 按幂等键只读回放 → 开事务并锁产品 → 读取新鲜行情 → 插入订单占用幂等键 → 锁共享钱包 → 扣余额并写流水 → 写代理佣金 → 读回订单 → 提交（`src/modules/seconds_contract/application.rs:235-373`）。

关键结论：

1. 应用服务从未查询“该用户是否已有 opened 订单”。唯一会返回旧订单的分支由同一幂等键触发（`src/modules/seconds_contract/application.rs:241-258`）。
2. 不同幂等键会分别执行插入与扣款。只要钱包余额覆盖每笔 stake，都会成功成为独立 `opened` 订单。
3. 同产品并发开仓会先竞争产品 `FOR UPDATE` 行锁（`src/modules/seconds_contract/infrastructure.rs:530-549`）；这保留了产品启停/配置快照的一致性，并由竞态测试证明产品禁用提交后等待中的开仓会失败且不扣款（`tests/seconds_contract_routes.rs:2660-2743`）。它限制吞吐但不限制持仓数量。
4. 同一用户、同一 stake asset 的不同订单随后会竞争钱包 `FOR UPDATE` 行锁（`src/modules/seconds_contract/infrastructure.rs:642-661`），串行计算余额并防止超卖。不同产品/资产可有更高并行度。
5. 订单、钱包扣款、钱包流水和订单来源佣金都在同一事务中（`src/modules/seconds_contract/application.rs:306-373`）。每个订单 ID 作为佣金 `source_id`，代理佣金唯一键为 `(agent_id, source_type, source_id)`，所以多订单会产生多组相互独立且可重放的佣金记录（`migrations/0028_agent_commission_source_id.sql:8-10`，`src/modules/agent/infrastructure.rs:86-104`）。
6. `open_order_with_events` 只在 `is_new_order` 时发布 `seconds_contract.order.opened`；同键回放不会重复发事件（`src/modules/seconds_contract/application.rs:376-386`，`src/modules/seconds_contract/service.rs:108-120`）。事件在数据库提交后以进程内广播发送，不是持久 outbox；崩溃窗口可能丢事件，客户端必须以订单列表作最终校准。

### 路由与 API 合同

1. 用户路由只有：列产品、`GET /seconds-contracts/orders`、`POST /seconds-contracts/orders`（`src/modules/seconds_contract/routes.rs:40-47`）。没有“检查是否已有活动订单”端点，也没有取消/替换现有活动订单的隐式操作。
2. `POST` 请求 DTO 要求 `product_id`、方向、金额和 `idempotency_key`，可选周期；不存在 `replace_active`、`single_position` 或相似字段（`src/modules/seconds_contract/presentation.rs:31-38`）。
3. 用户 `GET` 查询 DTO 只有 `limit`（`src/modules/seconds_contract/presentation.rs:11-14`）；默认 50、最大 100（`src/modules/seconds_contract/service.rs:539-545`）。在多订单增长后，这会产生一个真实风险：接口混合返回 opened/settled，若最近的 settled 行占满窗口，较旧但仍 `opened` 的订单可能不在响应中。后端若要成为“全部活动持仓”的可靠来源，应单独研究状态筛选/分页合同及相应 `(user_id, status, created_at, id)` 索引，而不是恢复单订单限制。
4. 订单响应是一行一对象并包含唯一 `id`、产品、方向、金额、状态和到期时间，天然可承载数组式多持仓（`src/modules/seconds_contract/presentation.rs:143-166`）。

### 结算 worker 与多订单

1. worker 扫描所有到期且可重试的 `opened` 行，按到期时间和订单 ID 排序并逐单处理，不按用户、产品或交易对分组（`src/workers/seconds_contract_settlement.rs:240-264`）。同一用户多个到期订单会各自结算。
2. 每笔结算先锁订单行，再检查 `settled`/`opened`，赢家时再锁该用户对应资产的钱包行，写入一条以订单 ID 为引用的流水，最后以 `WHERE id = ? AND status = 'opened'` 更新订单（`src/workers/seconds_contract_settlement.rs:309-405`）。这使多订单共享钱包时按订单逐笔安全入账。
3. worker 重跑时已结算订单不再被 due 查询选中；即使发生人工结算/多 worker 竞态，订单行锁和状态条件也会使后来者跳过，避免第二次派奖（`src/workers/seconds_contract_settlement.rs:315-332`、`378-389`）。
4. 缺行情、旧行情、缺开仓价或结算失败的订单会保留 `opened` 并把下次尝试推迟 60 秒，不会永久阻塞后续订单（`src/workers/seconds_contract_settlement.rs:169-216`、`266-279`）。多行测试证明坏订单重排后健康订单仍可继续结算（`tests/seconds_contract_settlement_worker.rs:462-520`、`630-691`、`695-745`）。
5. 启动逻辑会在每个启用且同时配置 MySQL/Redis 的服务进程内启动 worker（`src/main.rs:124-141`）。due 查询本身没有 `FOR UPDATE SKIP LOCKED` 或领取标记；水平扩容时多个进程可能重复扫描同一批订单，随后在逐单行锁处串行并跳过。资金幂等仍成立，但会造成重复 Redis 读取、锁竞争和批次吞吐下降；订单量增大后应把“多实例领取/分片”作为独立扩展议题。
6. 默认 5 秒轮询、批量上限 100（`src/config.rs:62-67`、`231-240`，`src/workers/seconds_contract_settlement.rs:476-482`）。允许大量并行订单后必须监控到期积压、最长结算延迟、失败/跳过率及钱包热点锁等待。

### 必须保留的幂等与资金合同

1. **保留 `(user_id, idempotency_key)` 唯一键。** 它只防止同一订单意图重复创建，不是单活动订单限制（`migrations/0021_seconds_contracts.sql:38`）。
2. **新订单新键、重试复用旧键。** 客户端若长期复用一个键，会被正确回放成同一订单，看起来像“只能开一单”；修复多订单时应调整客户端键生命周期，不应删除后端唯一键。
3. **保留请求指纹校验。** 同键但产品、显式周期、方向或金额不同必须返回冲突（`src/modules/seconds_contract/service.rs:164-182`）。
4. **保留禁用产品后的成功回放。** 已成功订单即使产品后来禁用，同键同请求仍返回原订单；不同请求仍冲突（`src/modules/seconds_contract/application.rs:246-284`，`tests/seconds_contract_routes.rs:2747-2855`）。
5. **保留先占用幂等键、再锁钱包及同事务提交。** 并发同键只有一条扣款路径；不同键则在钱包锁下逐笔验资（`src/modules/seconds_contract/application.rs:306-355`）。
6. **保留每订单独立引用。** 开仓流水、结算流水、佣金和 websocket 事件都必须携带真实 `order_id`，不能使用 `user_id` 或“当前活动订单”作为引用，否则多订单会互相覆盖。
7. **保留结算订单行锁、状态检查和条件更新。** `wallet_ledger` 没有来源唯一约束；重复派奖保护主要来自这些事务合同（`src/workers/seconds_contract_settlement.rs:315-389`）。
8. **保留事件的“仅新建/仅新结算发送”条件。** 重放不得产生重复 opened/settled 事件（`src/modules/seconds_contract/service.rs:108-120`、`150-162`）。

### 可移除项与不应移除项

- Rust 后端中没有单用户单活动订单门禁可移除，也不需要为“允许多订单”删除/修改现有迁移。
- 若上层存在 `hasActiveOrder`、单一 `activeOrder`、已有订单时禁用提交、或固定幂等键等限制，可在上层移除/改为数组模型；后端合同仍应保持每次新订单使用新键。
- 不应删除用户幂等唯一键、产品行锁、钱包行锁、订单/钱包/流水/佣金同事务、结算订单行锁及状态条件。
- 不应把数据库唯一键改为只含 `user_id`，也不应新增 `(user_id, status)` 唯一索引；两者都会重新引入单活动订单限制，并且 MySQL 普通唯一键无法只约束 `opened` 而放行任意历史 `settled` 行。

### Tests found and recommended contract gaps

已有覆盖：

- 同一用户可直接持有并列出两个 `opened` 行，且不泄漏另一用户订单（`tests/seconds_contract_routes.rs:1744-1854`）。该测试从数据库直接插入，证明 schema 和列表支持多行。
- 单次开仓扣款一次、写一条流水、发一个 opened 事件并创建订单来源佣金（`tests/seconds_contract_routes.rs:2123-2245`）。
- 顺序和并发的**同键**请求返回同一订单，只扣款/记账一次（`tests/seconds_contract_routes.rs:2648-2657`、`2968-3060`）。
- 人工结算重复同结果不会重复入账或重复发事件；不同结果重放冲突（`tests/seconds_contract_routes.rs:2367-2504`、`2522-2644`、`2858-2965`）。
- worker 重跑不重复派奖/事件/流水（`tests/seconds_contract_settlement_worker.rs:287-379`）。
- worker 可越过缺行情、缺钱包和缺开仓价的坏行继续处理健康行（`tests/seconds_contract_settlement_worker.rs:462-520`、`630-745`）。

缺口：

1. 没有通过 `POST /seconds-contracts/orders` 为同一用户使用**两个不同幂等键**连续或并发创建两单的明确回归测试；当前多 `opened` 测试使用直接 SQL。
2. 没有断言同一用户不同键并发时：两条订单、两次精确扣款、两条 open 流水、两组佣金、两个 opened 事件及列表返回两项。
3. 没有同一用户多个到期订单共享同一钱包的 worker 测试，也没有人工结算与 worker 同时抢同一订单的集成测试。
4. 没有多 worker 实例并行执行同一批 due rows 的测试；当前幂等推断来自逐单行锁和条件更新。
5. 没有用户列表超过 50/100 且混合 opened/settled 时仍能完整获取活动订单的合同；现有 API 结构本身无法保证。

### Related specs

- `.trellis/spec/backend/seconds-contracts.md` — 定义开仓行情、产品周期、共享现货钱包、订单/钱包/流水同事务、同键回放及人工/worker 结算一致性合同。
- `.trellis/spec/backend/database-guidelines.md` — 迁移不可变；若未来补索引或领取机制，必须新增迁移而不是修改既有秒合约迁移。
- `.trellis/spec/backend/quality-guidelines.md` — 金融行为显式、钱包/流水同事务、SQLx 属于 infrastructure、路由保持薄层。
- `.trellis/spec/backend/order-identifiers.md` — 多订单 UI 应以独立业务订单号展示，动作仍使用真实内部 ID；不能把多订单折叠成用户级单一引用。
- `.trellis/spec/guides/cross-layer-thinking-guide.md` — 多订单必须贯通 API 数组、客户端状态、实时事件及持久列表校准。

### External references

- 未使用外部资料；本结论基于仓库内实际 SQL、Rust 和测试。
- 相关依赖声明：SQLx `0.8`（MySQL）、Redis crate `0.27`、Tokio `1`（`Cargo.toml:32-46`）。

## Caveats / Not Found

- `python3 ./.trellis/scripts/task.py current --source` 返回 `Current task: (none)`；研究目标路径由用户消息明确指定，因此只写入该任务的 `research/` 目录。
- 未运行数据库/Redis 集成测试；本任务要求只读研究，结论来自静态代码、迁移和既有测试断言。
- 未检查移动端/PC 端的单订单状态实现；“限制更可能在客户端”是从后端不存在限制推导出的定位方向，不是对客户端代码的已验证结论。
- 秒合约领域层目前没有实体或状态机规则（`src/modules/seconds_contract/domain.rs:1-4`）；状态合法性和多订单合同分散在迁移、应用层及 worker 中。
- 人工结算路径与 worker 的结算价行为存在既有差异：人工 `mark_order_settled` 不写 `settlement_price`（`src/modules/seconds_contract/infrastructure.rs:737-749`），worker 会写入（`src/workers/seconds_contract_settlement.rs:378-385`）。该问题不构成单订单限制，但会影响多订单结算展示的一致性，宜另立任务处理。
