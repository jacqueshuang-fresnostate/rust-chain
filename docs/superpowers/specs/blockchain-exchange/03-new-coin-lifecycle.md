# 03. 新币生命周期、申购、派发与解禁设计

## 1. 术语

平台发行的币统一称为新币。

新币交易对使用内部策略行情。新币在预热、发行、派发阶段不展示交易 K 线；到达上市时间且交易对启用后，K 线开始生成和展示。上市后用户购买新币统一称为认购，认购所得新币仍按该币种解禁规则处理。

## 2. 生命周期阶段

新币生命周期分为 4 个业务阶段：预热、发行、派发、上市。

| 阶段 | 状态 | 用户可见性 | 用户操作 | 资产影响 | 行情 / K 线 |
|---|---|---|---|---|---|
| 0. 预热 | preheat | 可见 | 只能查看，不能购买或申购 | 无资产变化 | 不展示 K 线，可展示项目介绍和倒计时 |
| 1. 发行 | subscription | 可见 | 可以申购 | 冻结或扣减申购资金，生成申购记录，新币未到账 | 不展示交易 K 线 |
| 2. 派发 | distribution | 可见 | 查看申购结果和获配数量 | 新币进入用户账户，可用或锁定取决于解禁规则 | 不展示交易 K 线 |
| 3. 上市 | listed | 可见且可交易 | 可以按交易对买卖，也可以进行认购 | 认购所得按解禁规则进入 available 或 locked | K 线开始生成和展示 |

## 3. 生命周期流程

1. 后台创建新币：symbol、名称、精度、发行量、状态。
2. 配置预热开始时间、发行开始时间、发行结束时间、派发时间、上市时间。
3. 预热阶段用户只能看到新币项目，不能购买或申购。
4. 发行阶段用户提交申购订单，系统按规则冻结或扣减 USDT 等申购资产。
5. 发行结束后平台计算用户获配数量，未获配资金按规则退回。
6. 派发阶段新币进入用户账户，写入资产流水；如果有锁定周期，进入 locked 余额。
7. 上市阶段创建或启用交易对，例如 NEW/USDT，策略行情开始生成 ticker、trade、K 线和可选盘口快照。
8. 上市后前端交易页和 K 线图按统一行情接口展示。
9. 上市后用户购买新币称为认购；认购可以使用当前行情价或后台配置认购价。
10. 认购所得新币按该币种解禁规则进入 available 或 locked。
11. 到达解禁条件后，如后台配置了解禁矿工费，用户必须先支付矿工费才可解禁。

## 4. 解禁规则

| 解禁类型 | 字段值 | 说明 | 示例 |
|---|---|---|---|
| 上市立即解禁 | immediate_on_listing | 上市时派发的新币直接进入可用余额 | listed_at 到达后全部 available |
| 固定时间解禁 | fixed_time | 到指定时间后从 locked 转入 available | 2026-11-10 11:22:33 解禁 |
| 时间周期解禁 | relative_period | 从每笔申购获配、派发或认购来源时间起经过指定周期后解禁 | 1 天、3 周、3 月 |

解禁处理规则：

- 派发和上市后认购都根据 unlock_type 决定进入 available 或 locked。
- `wallet_accounts.locked` 只表示用户某币种锁定汇总余额，不作为解禁明细来源。
- 活跃锁定明细由 `asset_lock_positions` 记录；来源追踪由 `asset_lock_position_sources` 记录。
- `immediate_on_listing` 使用 listed_at 作为 unlock_at；上市前产生的锁定可按 user_id + asset_id + listed_at 聚合，上市后认购直接进入 available，但仍写流水和来源记录。
- `fixed_time` 使用固定 unlock_at；同一用户、同一币种、同一 unlock_at 的申购获配、派发和认购可聚合为一个锁定仓位。
- `relative_period` 以每笔来源订单或派发记录的 source_time + period 计算 unlock_at；每笔来源必须单独形成锁定订单，不聚合。
- 定时任务扫描到期锁定仓位；如果未启用矿工费，可自动将 locked 转入 available。
- 如果启用解禁矿工费，到期后只标记为可申请解禁，用户支付矿工费后才从 locked 转入 available。
- 每次锁定、解禁、退回、矿工费支付都必须写 wallet_ledger。
- 用户下单、闪兑和转出只能使用 available，不允许使用 locked。
- 后台修改解禁规则只能影响未派发、未认购或未生效批次，已产生记录如需调整必须走审计和修正流水。

## 5. 上市后认购与矿工费

上市后认购是指新币已经上市、有 K 线图后，用户通过认购入口购买新币。认购不是发行前申购，认购所得仍继承该币种解禁规则。

认购流程：

1. 新币进入 listed 状态。
2. 前端展示 K 线图、行情、认购入口。
3. 用户提交认购数量或支付金额。
4. 系统按当前新币价格或后台认购价计算应付金额。
5. 用户支付 USDT 或后台指定资产。
6. 系统生成 new_coin_purchase_orders 认购记录。
7. 如果币种配置为立即解禁且当前已上市，认购新币进入 available。
8. 如果币种配置为锁定，认购新币进入 locked，并按 unlock_type 创建或合并锁定仓位。
9. fixed_time 认购并入同一 unlock_at 的锁定仓位；relative_period 认购按每笔订单单独生成锁定订单。
10. 到达解禁时间后，用户可申请解禁。
11. 如果配置了解禁矿工费，用户支付矿工费后才可解禁。
12. 系统将 locked 转为 available，并写矿工费流水和解禁流水。

解禁矿工费配置：

| 配置 | 说明 |
|---|---|
| unlock_fee_enabled | 是否启用解禁矿工费 |
| unlock_fee_rate | 矿工费比例，例如 4% |
| unlock_fee_basis | 计费基础：market_value 或 profit |
| unlock_fee_asset | 支付币种，例如 USDT 或平台币 |
| unlock_fee_pay_timing | 支付时机，默认申请解禁时支付 |
| unlock_without_fee_allowed | 是否允许不付费延迟解禁，默认不允许 |

计算方式：

| 计费方式 | 公式 |
|---|---|
| 按市值 | miner_fee = unlock_quantity × unlock_price × fee_rate |
| 按收益 | miner_fee = max(unlock_quantity × unlock_price - purchase_cost, 0) × fee_rate |

unlock_price 来源：

- 优先取解禁时 Redis 最新价格。
- Redis 不可用时取 MongoDB 最近 K 线 close。
- 价格不可用时拒绝解禁并提示稍后重试。

## 6. MySQL 数据表

| 表 | 作用 | 关键字段 |
|---|---|---|
| assets | 币种定义 | id, symbol, name, precision, asset_type, status |
| asset_issuances | 新币发行信息 | asset_id, total_supply, circulating_supply, issue_price, issuer, lifecycle_status, preheat_start_at, subscription_start_at, subscription_end_at, distribution_at, listed_at, unlock_type, unlock_at, unlock_period_value, unlock_period_unit, post_listing_purchase_enabled, unlock_fee_enabled, unlock_fee_rate, unlock_fee_basis, unlock_fee_asset, status |
| new_coin_subscriptions | 新币申购记录 | id, user_id, asset_id, pay_asset_id, pay_amount, subscribed_quantity, allocated_quantity, refund_amount, status, created_at |
| new_coin_distributions | 新币派发记录 | id, user_id, asset_id, subscription_id, amount, locked_amount, available_amount, distributed_at, unlock_rule_id, status |
| new_coin_purchase_orders | 上市后认购记录 | id, user_id, asset_id, pay_asset_id, pay_amount, quantity, purchase_price, purchase_cost, lock_status, unlock_rule_id, status, created_at |
| asset_lock_positions | 新币锁定仓位 / 锁定订单 | id, user_id, asset_id, unlock_type, unlock_at, locked_amount, released_amount, remaining_amount, merge_key, status, created_at |
| asset_lock_position_sources | 锁定仓位来源 | lock_position_id, source_type, source_id, source_amount, source_time, created_at |
| asset_unlock_records | 新币解禁、矿工费和释放历史 | id, user_id, asset_id, lock_position_id, unlock_quantity, unlock_price, unlock_fee_enabled, unlock_fee_rate, unlock_fee_basis, unlock_fee_asset, unlock_fee_amount, fee_paid_status, status |
| trading_pairs | 交易对 | id, base_asset, quote_asset, price_precision, qty_precision, min_order_value, status, market_type |

`asset_lock_positions` 是活跃锁定明细来源，`asset_unlock_records` 只记录解禁、矿工费和释放历史，不作为当前锁定余额来源。
| market_strategies | 策略配置 | id, pair_id, strategy_type, start_price, target_price, start_time, end_time, volatility, volume_min, volume_max, status |
| strategy_runs | 策略运行记录 | strategy_id, run_status, current_price, last_tick_at, last_generated_at, last_kline_open_time, recovery_status, error_message |
| strategy_versions | 策略版本记录 | strategy_id, version, effective_time, config_json, seed, created_by, created_at |

`lifecycle_status`：

- preheat：预热，用户只能查看。
- subscription：发行申购，用户可申购但新币未到账。
- distribution：派发，新币进入用户账户。
- listed：上市，交易对开放，K 线开始展示。

`unlock_type`：

- immediate_on_listing：上市立即解禁。
- fixed_time：固定时间解禁。
- relative_period：按时间周期解禁。

`unlock_fee_basis`：

- market_value：按解禁市值收取。
- profit：按收益收取。

wallet_ledger 新增 change_type：

- new_coin_purchase：上市后认购。
- unlock_fee：解禁矿工费。
- unlock_release：解禁释放。

## 7. 新币策略行情

| 模块 | 职责 |
|---|---|
| Admin Strategy Config | 管理目标价、涨跌方向、时间窗口、波动率、成交量 |
| Strategy Engine | 生成连续价格路径、模拟成交、K 线基础数据 |
| Market Event Bus | 将策略行情包装成统一 MarketEvent |
| Kline Aggregator | 聚合 1m、5m、15m、1h、1d K 线 |
| Risk Guard | 限制异常价格跳变、策略冲突、越界参数 |
| Audit Log | 记录策略创建、修改、启停 |

策略边界：

- 只能绑定 internal / strategy 交易对。
- 修改、启停必须写审计日志。
- 策略修改必须生成 strategy_versions，不能覆盖历史版本。
- 运行状态必须保存检查点，支持服务重启后补齐缺失 K 线。
- 参数必须经过 Risk Guard 校验。

## 8. K 线连续性

新币不能只依赖服务运行时的内存定时器生成 K 线。策略行情必须按交易对时间轴确定性生成，服务重启后可从检查点补齐缺口。

核心机制：

- market_strategies 保存策略参数、时间窗口、随机种子、是否允许停机补偿。
- strategy_versions 记录每次策略修改及 effective_time。
- strategy_runs 保存 last_generated_at、last_kline_open_time、current_price、recovery_status。
- MongoDB market_klines_{symbol} 使用 unique(interval, open_time) 与 upsert。
- Redis 锁避免多实例重复补偿。

## 9. 用户端 API

| API | 说明 |
|---|---|
| GET /api/v1/new-coins | 新币列表 |
| GET /api/v1/new-coins/{symbol} | 新币详情、阶段、时间线、解禁规则 |
| POST /api/v1/new-coins/{symbol}/subscriptions | 发行阶段申购 |
| GET /api/v1/new-coins/subscriptions | 我的申购记录 |
| GET /api/v1/new-coins/distributions | 我的派发与锁定记录 |
| POST /api/v1/new-coins/{symbol}/purchase | 上市后认购 |
| GET /api/v1/new-coins/purchases | 我的认购记录 |
| GET /api/v1/new-coins/unlocks | 我的待解禁记录 |
| POST /api/v1/new-coins/unlocks/{id}/pay-fee | 支付解禁矿工费 |
| POST /api/v1/new-coins/unlocks/{id}/release | 执行解禁 |

## 10. 平台后台 API

| API | 说明 |
|---|---|
| POST /admin/api/v1/new-coins | 创建新币 |
| PATCH /admin/api/v1/new-coins/{id}/lifecycle | 调整预热、发行、派发、上市阶段 |
| POST /admin/api/v1/new-coins/{id}/distribute | 执行派发 |
| PATCH /admin/api/v1/new-coins/{id}/unlock-rule | 配置解禁规则 |
| PATCH /admin/api/v1/new-coins/{id}/post-listing-purchase | 配置上市后认购 |
| PATCH /admin/api/v1/new-coins/{id}/unlock-fee-rule | 配置解禁矿工费 |
| GET /admin/api/v1/new-coins/{id}/subscriptions | 查看申购记录 |
| GET /admin/api/v1/new-coins/{id}/distributions | 查看派发记录 |
| GET /admin/api/v1/new-coins/purchases | 查看认购记录 |
| GET /admin/api/v1/new-coins/unlocks | 查看解禁记录 |

## 11. 风控与验收

风控规则：

- 新币发行阶段申购仅发行阶段开放。
- 上市后认购仅 listed 状态开放，并可由后台单独开关控制。
- 限制申购/认购额度、重复提交和资金不足。
- 派发和认购必须幂等。
- fixed_time 锁定必须按 user_id + asset_id + unlock_at 聚合，relative_period 锁定必须按来源订单拆分。
- wallet_accounts.locked 必须等于该用户该币种 active 锁定仓位 remaining_amount 汇总。
- 获配、退款、认购、锁定、矿工费、解禁均写资产流水。
- 启用解禁矿工费时，未支付矿工费不能解禁。
- 解禁价格不可用时禁止计算矿工费。
- 新币在预热、发行、派发阶段不展示交易 K 线。
- 新币到达上市时间且交易对启用后，K 线开始生成和展示。

验收条件：

- 预热不可申购。
- 发行可申购。
- 派发到账。
- 上市后 K 线出现。
- 上市后可通过认购入口购买新币。
- 认购所得按币种解禁规则进入 locked 或 available。
- 矿工费可按市值或收益计算。
- 矿工费支付币种可配置。
- 未支付矿工费不能解禁。
- 支付矿工费后 locked 转 available。
- fixed_time 锁定可聚合，relative_period 锁定按订单拆分。
- wallet_accounts.locked 与活跃锁定仓位汇总一致。
- 锁定与解禁正确。
- 服务重启后可补齐策略 K 线缺口。
