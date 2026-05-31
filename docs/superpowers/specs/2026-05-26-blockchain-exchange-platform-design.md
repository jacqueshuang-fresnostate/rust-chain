# 区块链交易所平台设计文档

## 1. 文档目标

本文档用于定义一个区块链交易所平台的一期 MVP 设计。平台采用混合模式：主流币交易对接入 Bitget 与 HTX 行情，新币使用内部交易对与策略行情。文档兼顾产品说明、技术架构、数据模型、后台权限、代理体系与验收标准，供后续研发拆分和实现使用。

## 2. 项目定位

### 2.1 平台模式

平台采用混合交易所模式：

- 平台发行的币在本文档中统一称为新币。
- 主流币交易对展示 Bitget / HTX 外部行情。
- 新币交易对使用内部策略行情。
- 外部行情、内部撮合、策略行情在数据模型和业务权限上明确隔离。
- 后台行情策略仅允许作用于新币或内部测试交易对。

### 2.2 一期 MVP 范围

一期实现：

- 用户与认证
- 资产账户与资产流水
- Bitget / HTX 行情适配
- K 线查询与聚合
- 现货交易
- 平台新币发行
- 新币策略行情
- 平台管理员后台
- 代理、邀请、代理后台
- 风控、审计、权限隔离

二期预留：

- 秒合约
- 杠杆交易
- 理财产品
- 做市机器人
- 链上充值提现
- 多级返佣结算

## 3. 技术架构

### 3.1 技术栈

| 层级 | 方案 |
|---|---|
| 后端语言 | Rust |
| Web 框架 | Axum 优先，Actix 作为可选替代 |
| 交易数据库 | MySQL |
| 行情文档库 | MongoDB |
| 缓存 | Redis |
| 消息队列 | RabbitMQ |
| 行情源 | Bitget + HTX |
| 部署形态 | 模块化单体服务 + MySQL + MongoDB + Redis + RabbitMQ |

### 3.2 架构方案

一期采用模块化单体 MVP：

- 一个 Rust 后端工程，按领域模块拆分。
- 用户、资产、行情、交易、后台、新币、代理模块保持清晰边界。
- RabbitMQ 用于关键异步事件分发。
- 撮合引擎、行情适配层、策略行情引擎按可独立服务的边界设计，后期可拆为微服务。

### 3.3 核心模块

| 模块 | 职责 |
|---|---|
| API Gateway / Web API | 用户 API、后台 API、代理 API、WebSocket、认证、限流、请求校验 |
| 用户与认证 | 注册、登录、JWT、refresh token、KYC 预留、安全配置 |
| 资产账户 | 可用余额、冻结余额、锁定余额、资产流水 |
| 行情模块 | Bitget / HTX REST 与 WebSocket 适配，行情标准化，缓存与分发 |
| 现货交易 | 下单、撤单、撮合、成交、订单查询 |
| K 线聚合 | 外部 K 线同步、内部 K 线聚合、多周期查询 |
| 新币发行 | 创建币种、发行信息、交易对、上线状态 |
| 策略行情 | 目标价、涨跌方向、波动率、成交量模拟、K 线生成 |
| 平台管理员后台 | 全平台运营、配置、风控、审计 |
| 代理后台 | 代理团队用户、邀请码、统计、返佣记录查看 |
| 风控审计 | 限流、限额、敏感操作审计、异常检测 |

## 4. 业务流程与数据流

### 4.1 全局时间字段边界

- 项目所有涉及时间的对外字段必须使用时间戳语义，默认采用 Unix milliseconds 数字。
- REST API、WebSocket、RabbitMQ 事件 payload、Redis 缓存 payload 和 MongoDB 行情文档不得输出本地化时间字符串。
- Rust 服务内部可使用 `DateTime<Utc>`，MySQL 可使用 `TIMESTAMP(6)`；进入或离开系统边界时必须转换为 Unix milliseconds。
- 新增模块、字段、测试和文档时必须显式检查时间戳要求，防止该需求被遗忘。

### 4.2 行情数据流

1. Market Adapter 连接 Bitget / HTX REST 与 WebSocket。
2. 将外部 ticker、depth、trade、kline 标准化为平台内部 MarketEvent。
3. 最新 ticker、盘口、短周期 K 线写入 Redis。
4. 标准化行情事件发布到 RabbitMQ。
5. K 线聚合器、WebSocket 推送、交易模块订阅行情事件。
6. 历史 K 线、策略行情结果、可选 ticker / trade 归档写入 MongoDB。
7. MySQL 仅保存行情源、交易对、策略配置等低频业务配置。

### 4.3 现货下单流程

1. 用户提交限价单或市价单。
2. API 校验登录态、交易对状态、价格精度、数量精度、最小下单额。
3. 资产模块冻结对应资产。
4. 订单进入撮合模块。
5. 撮合成功后生成成交记录。
6. 资产模块按成交结果解冻、扣减、入账。
7. 写入订单、成交、资产流水。
8. RabbitMQ 发布 OrderMatched / BalanceChanged 事件。
9. WebSocket 推送订单状态、成交和资产变化。

### 4.4 新币生命周期流程

新币生命周期分为 4 个业务阶段：预热、发行、派发、上市。

| 阶段 | 状态 | 用户可见性 | 用户操作 | 资产影响 | 行情 / K 线 |
|---|---|---|---|---|---|
| 0. 预热 | preheat | 可见 | 只能查看，不能购买或申购 | 无资产变化 | 不展示 K 线，可展示项目介绍和倒计时 |
| 1. 发行 | subscription | 可见 | 可以申购 | 冻结或扣减申购资金，生成申购记录，新币未到账 | 不展示交易 K 线 |
| 2. 派发 | distribution | 可见 | 查看申购结果和获配数量 | 新币进入用户账户，可用或锁定取决于解禁规则 | 不展示交易 K 线 |
| 3. 上市 | listed | 可见且可交易 | 可以按交易对买卖 | 解禁部分可交易，锁定部分不可交易 | K 线开始生成和展示 |

流程说明：

1. 后台创建新币：symbol、名称、精度、发行量、状态。
2. 配置预热开始时间、发行开始时间、发行结束时间、派发时间、上市时间。
3. 预热阶段用户只能看到新币项目，不能购买或申购。
4. 发行阶段用户提交申购订单，系统按规则冻结或扣减 USDT 等申购资产。
5. 发行结束后平台计算用户获配数量，未获配资金按规则退回。
6. 派发阶段新币进入用户账户，写入资产流水；如果有锁定周期，进入 locked 余额。
7. 上市阶段创建或启用交易对，例如 NEW/USDT，策略行情开始生成 ticker、trade、K 线和可选盘口快照。
8. 上市后前端交易页和 K 线图按统一行情接口展示。

新币解禁规则：

| 解禁类型 | 说明 | 示例 |
|---|---|---|
| 上市立即解禁 | 上市时派发的新币直接进入可用余额 | listed_at 到达后全部 available |
| 固定时间解禁 | 到指定时间后从 locked 转入 available | 2026-11-10 11:22:33 解禁 |
| 时间周期解禁 | 从派发或上市时间起经过指定周期后解禁 | 1 天、3 周、3 月 |

解禁处理规则：

- 派发时根据 unlock_type 决定进入 available 或 locked。
- 定时任务扫描到期锁定记录，将 locked 转入 available。
- 每次锁定、解禁、退回都必须写 wallet_ledger。
- 用户下单只能使用 available，不允许使用 locked。
- 后台修改解禁规则只能影响未派发或未生效批次，已派发记录如需调整必须走审计和修正流水。

### 4.5 策略行情数据流

| 模块 | 职责 |
|---|---|
| Admin Strategy Config | 管理目标价、涨跌方向、时间窗口、波动率、成交量 |
| Strategy Engine | 生成连续价格路径、模拟成交、K 线基础数据 |
| Market Event Bus | 将策略行情包装成统一 MarketEvent |
| Kline Aggregator | 聚合 1m、5m、15m、1h、1d K 线 |
| Risk Guard | 限制异常价格跳变、策略冲突、越界参数 |
| Audit Log | 记录策略创建、修改、启停 |

### 4.6 新币 K 线连续性与停机补偿

平台新币不能只依赖服务运行时的内存定时器生成 K 线，否则项目更新、服务重启或停机 30 分钟会导致 MongoDB 中缺少对应时间段的 K 线。根因是行情生成绑定了服务在线状态，而不是绑定策略配置、交易对时间轴和持久化检查点。

解决原则：策略行情必须按交易对时间轴确定性生成，服务重启后可从检查点补齐缺口。

| 机制 | 设计 |
|---|---|
| 持久化策略 | market_strategies 保存策略参数、时间窗口、随机种子、是否允许停机补偿 |
| 策略版本 | 每次修改策略生成新版本，记录 effective_time，补偿时按版本时间段生成 |
| 生成检查点 | strategy_runs 保存 last_generated_at、last_kline_open_time、current_price、recovery_status |
| 启动扫描 | 服务启动后扫描 active/running 策略，比较 MongoDB 最新 K 线时间与当前时间 |
| 缺口计算 | 按 symbol + interval 找到缺失 open_time 区间，例如 10:00 至 10:30 |
| 确定性补偿 | 使用 strategy_id、version、symbol、timestamp、seed 生成同一时间点的稳定价格路径 |
| 幂等写入 | 写入 market_klines_{symbol} 时使用 unique(interval, open_time) 和 upsert |
| 分布式锁 | 同一交易对同一策略补偿任务使用 Redis 锁，避免多实例重复生成 |
| 补偿状态 | 补偿期间策略状态为 catching_up，补齐后切回 live |
| 推送规则 | 历史补偿 K 线写 MongoDB，不按实时行情逐条推送；补齐完成后再推送最新行情 |

停机恢复流程：

1. 服务启动后加载所有运行中的新币策略。
2. 对每个交易对读取 MongoDB 中 market_klines_{symbol} 的最大 open_time。
3. 读取 strategy_runs.last_generated_at，取两者中更可信的时间作为补偿起点。
4. 计算补偿起点到当前时间之间缺失的 K 线周期。
5. 按策略版本、时间段、随机种子生成缺失 K 线。
6. 通过 upsert 写入 market_klines_{symbol}，更新 strategy_runs 检查点。
7. 补偿完成后恢复实时策略行情生成。

如果后台明确暂停策略，系统不补偿暂停期间 K 线；如果是服务维护、重启、异常宕机，系统按策略配置自动补偿。

### 4.7 一期与二期边界

| 模块 | 一期 | 二期 |
|---|---|---|
| 现货 | 支持 | 深化撮合性能、做市 |
| 秒合约 | 预留接口 | 完整产品 |
| 杠杆 | 预留账户结构 | 借贷、强平、风险率 |
| 理财 | 预留资产流水类型 | 产品、申购、赎回、收益 |
| 新币行情 | 策略行情 | 做市机器人 |
| 链上充值提现 | 可先人工/后台入账 | 节点监听、自动归集、风控 |

## 5. 数据模型

### 5.1 用户与权限

| 表 | 作用 | 关键字段 |
|---|---|---|
| users | 用户基础信息 | id, email, phone, password_hash, status, kyc_level, created_at |
| user_security | 用户安全配置 | user_id, fund_password_hash, totp_enabled, anti_phishing_code |
| admin_users | 平台管理员 | id, username, password_hash, role_id, status |
| admin_roles | 平台后台角色 | id, name, permissions |
| admin_audit_logs | 平台后台审计 | admin_id, action, target_type, target_id, before_json, after_json, ip, created_at |

### 5.2 币种与交易对

| 表 | 作用 | 关键字段 |
|---|---|---|
| assets | 币种定义 | id, symbol, name, precision, asset_type, status |
| asset_issuances | 新币发行信息 | asset_id, total_supply, circulating_supply, issue_price, issuer, lifecycle_status, preheat_start_at, subscription_start_at, subscription_end_at, distribution_at, listed_at, unlock_type, unlock_at, unlock_period_value, unlock_period_unit, status |
| new_coin_subscriptions | 新币申购记录 | id, user_id, asset_id, pay_asset_id, pay_amount, subscribed_quantity, allocated_quantity, refund_amount, status, created_at |
| new_coin_distributions | 新币派发记录 | id, user_id, asset_id, subscription_id, amount, locked_amount, available_amount, distributed_at, unlock_rule_id, status |
| asset_unlock_records | 新币解禁记录 | id, user_id, asset_id, source_type, source_id, locked_amount, unlocked_amount, unlock_type, unlock_at, status |
| trading_pairs | 交易对 | id, base_asset, quote_asset, price_precision, qty_precision, min_order_value, status, market_type |

`market_type`：

- external：外部行情交易对，如 BTC/USDT。
- internal：平台内部交易对，如 NEW/USDT。
- strategy：平台策略行情交易对。

`lifecycle_status`：

- preheat：预热，用户只能查看。
- subscription：发行申购，用户可申购但新币未到账。
- distribution：派发，新币进入用户账户。
- listed：上市，交易对开放，K 线开始展示。

`unlock_type`：

- immediate_on_listing：上市立即解禁。
- fixed_time：固定时间解禁。
- relative_period：按时间周期解禁。

### 5.3 资产账户

| 表 | 作用 | 关键字段 |
|---|---|---|
| wallet_accounts | 用户币种账户 | user_id, asset_id, available, frozen, locked, updated_at |
| wallet_ledger | 资产流水 | id, user_id, asset_id, change_type, amount, balance_after, ref_type, ref_id, created_at |
| deposit_records | 充值记录预留 | user_id, asset_id, amount, tx_hash, status |
| withdraw_records | 提现记录预留 | user_id, asset_id, amount, fee, address, status |

资产原则：

- 禁止直接改余额不写流水。
- 下单先冻结，撤单解冻，成交后结算。
- 所有资产变动必须具备 ref_type / ref_id。

### 5.4 订单与成交

| 表 | 作用 | 关键字段 |
|---|---|---|
| spot_orders | 现货订单 | id, user_id, pair_id, side, order_type, price, quantity, filled_quantity, status, created_at |
| spot_trades | 成交记录 | id, pair_id, buy_order_id, sell_order_id, price, quantity, fee, created_at |
| order_events | 订单事件 | order_id, event_type, payload_json, created_at |

订单状态：pending、open、partially_filled、filled、cancelled、rejected。

### 5.5 行情与 K 线

行情数据按存储职责拆分：

- MySQL 只保存行情源、交易对、策略配置等低频业务配置。
- Redis 保存最新 ticker、最新盘口、近期 K 线和 WebSocket 推送缓存。
- MongoDB 保存历史 K 线、平台新币策略行情结果、可选 ticker / trade / depth 归档。
- RabbitMQ 负责行情事件分发，不作为长期存储。

MySQL 表：

| 表 | 作用 | 关键字段 |
|---|---|---|
| market_sources | 行情源配置 | name, priority, enabled, rest_base_url, ws_url |

MongoDB 按交易对拆分 Collection，避免所有交易对混写到同一个大集合。集合命名统一使用标准化交易对名，例如 `BTCUSDT`、`ETHUSDT`、`NEWUSDT`。

| Collection 模式 | 作用 | 示例 |
|---|---|---|
| market_klines_{symbol} | 单交易对历史 K 线 | market_klines_BTCUSDT, market_klines_NEWUSDT |
| market_tickers_{symbol} | 单交易对 ticker 快照归档，可 TTL | market_tickers_BTCUSDT |
| market_trades_{symbol} | 单交易对成交流归档，可 TTL | market_trades_NEWUSDT |
| market_depth_snapshots_{symbol} | 单交易对盘口快照归档，可 TTL | market_depth_snapshots_BTCUSDT |
| strategy_market_events_{symbol} | 单交易对新币策略行情事件 | strategy_market_events_NEWUSDT |

文档中的 `market_klines`、`market_tickers`、`market_trades`、`market_depth_snapshots`、`strategy_market_events` 表示逻辑集合族，不表示所有交易对共用一个物理集合。

MongoDB 文档字段：

| Collection 模式 | 关键字段 |
|---|---|
| market_klines_{symbol} | interval, open_time, close_time, open, high, low, close, volume, source |
| market_tickers_{symbol} | last_price, volume_24h, source, ts |
| market_trades_{symbol} | price, quantity, side, source, ts |
| market_depth_snapshots_{symbol} | bids, asks, source, ts |
| strategy_market_events_{symbol} | strategy_id, event_type, price, volume, ts, payload |

MongoDB 索引：

- market_klines_{symbol}：unique(interval, open_time)
- market_tickers_{symbol}：index(ts)，可按 ts 设置 TTL
- market_trades_{symbol}：index(ts)，可按 ts 设置 TTL
- market_depth_snapshots_{symbol}：index(ts)，可按 ts 设置 TTL
- strategy_market_events_{symbol}：index(strategy_id, ts)，index(ts)

查询规则：

- 服务层根据交易对 symbol 路由到对应 Collection。
- 禁止 API 直接拼接任意 Collection 名，必须通过交易对白名单和命名规范生成。
- 新交易对上线时，由后台创建交易对配置，并初始化或延迟创建对应 MongoDB Collection 与索引。

Redis Key：

- market:ticker:{pair}
- market:depth:{pair}
- market:kline:{pair}:{interval}
- user:session:{token_id}
- risk:limit:{user_id}

### 5.6 新币策略行情

| 表 | 作用 | 关键字段 |
|---|---|---|
| market_strategies | 策略配置 | id, pair_id, strategy_type, start_price, target_price, start_time, end_time, volatility, volume_min, volume_max, status |
| strategy_runs | 策略运行记录 | strategy_id, run_status, current_price, last_tick_at, last_generated_at, last_kline_open_time, recovery_status, error_message |
| strategy_versions | 策略版本记录 | strategy_id, version, effective_time, config_json, seed, created_by, created_at |
| strategy_events | 策略事件 | strategy_id, event_type, payload_json, created_at |

策略边界：

- 只能绑定 internal / strategy 交易对。
- 修改、启停必须写审计日志。
- 策略修改必须生成 strategy_versions，不能覆盖历史版本。
- 运行状态必须保存检查点，支持服务重启后补齐缺失 K 线。
- 参数必须经过 Risk Guard 校验。

### 5.7 代理与邀请

| 表 | 作用 | 关键字段 |
|---|---|---|
| agents | 代理主体 | id, user_id, agent_code, level, status, created_at |
| invite_codes | 邀请码 | id, owner_type, owner_id, code, usage_limit, used_count, status |
| user_referrals | 用户邀请关系 | user_id, direct_inviter_id, direct_inviter_type, root_agent_id, depth, path, created_at |
| agent_admin_users | 代理后台账号 | id, agent_id, username, password_hash, status, last_login_at |
| agent_audit_logs | 代理后台审计 | agent_id, agent_admin_id, action, target_type, target_id, ip, created_at |
| agent_commission_rules | 返佣规则预留 | agent_id, product_type, commission_rate, status |
| agent_commission_records | 返佣记录预留 | agent_id, user_id, source_type, source_amount, commission_amount, status |

代理关系采用混合模式：

- 保存完整邀请树。
- 同时保存 root_agent_id。
- 一期代理后台按 root_agent_id 查看团队。
- 二期扩展多级返佣和团队业绩。

### 5.8 二期预留表

| 模块 | 预留表 |
|---|---|
| 秒合约 | contract_products, contract_orders, contract_settlements |
| 杠杆 | margin_accounts, margin_loans, margin_risk_snapshots, liquidation_records |
| 理财 | earn_products, earn_orders, earn_income_records |
| 做市机器人 | market_maker_configs, market_maker_orders |

## 6. API 与 WebSocket

### 6.1 用户端 REST API

| 模块 | API | 说明 |
|---|---|---|
| 认证 | POST /api/v1/auth/register | 注册 |
| 认证 | POST /api/v1/auth/login | 登录 |
| 认证 | POST /api/v1/auth/refresh | 刷新 token |
| 用户 | GET /api/v1/user/profile | 用户信息 |
| 资产 | GET /api/v1/wallet/accounts | 用户资产列表 |
| 资产 | GET /api/v1/wallet/ledger | 资产流水 |
| 行情 | GET /api/v1/markets | 交易对列表 |
| 行情 | GET /api/v1/markets/{symbol}/ticker | 最新行情 |
| 行情 | GET /api/v1/markets/{symbol}/klines | K 线 |
| 新币 | GET /api/v1/new-coins | 新币列表 |
| 新币 | GET /api/v1/new-coins/{symbol} | 新币详情、阶段、时间线、解禁规则 |
| 新币 | POST /api/v1/new-coins/{symbol}/subscriptions | 发行阶段申购 |
| 新币 | GET /api/v1/new-coins/subscriptions | 我的申购记录 |
| 新币 | GET /api/v1/new-coins/distributions | 我的派发与锁定记录 |
| 现货 | POST /api/v1/spot/orders | 下单 |
| 现货 | DELETE /api/v1/spot/orders/{id} | 撤单 |
| 现货 | GET /api/v1/spot/orders | 订单列表 |
| 现货 | GET /api/v1/spot/trades | 成交记录 |
| 邀请 | GET /api/v1/referral/my-code | 我的邀请码 |
| 邀请 | POST /api/v1/referral/bind | 绑定邀请码 |
| 邀请 | GET /api/v1/referral/my-invites | 我的邀请记录 |

### 6.2 用户端 WebSocket

入口：

- WS /ws/public
- WS /ws/private

公共订阅：

- ticker.BTCUSDT
- depth.BTCUSDT
- trade.BTCUSDT
- kline.BTCUSDT.1m
- kline.BTCUSDT.5m

私有订阅：

- orders
- balances
- notifications

消息格式：

```json
{
  "topic": "ticker.BTCUSDT",
  "type": "snapshot",
  "ts": 1710000000000,
  "data": {}
}
```

### 6.3 RabbitMQ 事件

| Exchange | Routing Key | 说明 |
|---|---|---|
| market.events | ticker.BTCUSDT | 行情事件 |
| order.events | order.created | 订单创建 |
| order.events | order.matched | 订单成交 |
| wallet.events | wallet.balance_changed | 资产变化 |
| strategy.events | strategy.started | 策略状态 |
| audit.events | admin.action | 审计事件 |

## 7. 平台管理员后台与代理后台

### 7.1 后台类型

| 后台 | 使用者 | 定位 |
|---|---|---|
| 平台管理员后台 | 平台运营、财务、风控、超级管理员 | 管理整个平台 |
| 代理后台 | 代理商、渠道负责人 | 管理自己名下用户与业绩 |
| 用户端 | 普通用户 | 交易、资产、邀请 |

### 7.2 平台管理员后台功能

| 模块 | 能力 |
|---|---|
| 用户管理 | 查看全平台用户，冻结/解冻用户，调整状态，查看订单和资产 |
| 代理管理 | 创建代理、禁用代理、重置代理账号、调整代理归属 |
| 币种管理 | 创建币种、修改精度、上下架币种 |
| 新币发行管理 | 配置预热、发行申购、派发、上市、解禁规则 |
| 交易对管理 | 创建交易对、上下架交易对、配置最小下单额和精度 |
| 行情源管理 | 配置 Bitget / HTX 主备源、启停行情源 |
| 新币策略行情 | 创建、修改、启动、停止平台新币策略行情 |
| 订单管理 | 查看全平台订单、成交、异常订单 |
| 资产管理 | 查看全平台用户资产、流水、充值提现记录 |
| 风控管理 | 配置限额、限频、交易对风控、用户风控 |
| 产品配置 | 二期配置秒合约、杠杆、理财 |
| 审计日志 | 查看管理员和代理关键操作 |
| 系统配置 | 手续费、交易时间、公告、参数配置 |
| 财务报表 | 平台交易额、手续费、资产统计、代理返佣统计 |

平台管理员高风险操作必须具备 RBAC、二次确认、审计日志和操作原因。

### 7.3 代理后台功能

| 模块 | 能力 |
|---|---|
| 代理仪表盘 | 团队用户数、注册量、交易量、手续费贡献、返佣预估 |
| 我的用户 | 只查看 root_agent_id = 当前代理 的用户 |
| 用户详情 | 查看团队用户基本信息、注册时间、状态、交易统计 |
| 邀请码管理 | 创建、禁用、查看自己的邀请码 |
| 邀请关系 | 查看团队邀请树、直属用户、间接用户 |
| 团队交易统计 | 查看团队现货交易额、订单数、手续费 |
| 团队资产统计 | 只读汇总统计，敏感明细可配置隐藏 |
| 返佣记录 | 查看返佣明细、待结算、已结算状态 |
| 账号安全 | 修改自己的登录密码、查看登录记录 |

代理后台禁止：

- 创建或修改币种。
- 创建或修改交易对。
- 启动或停止新币策略行情。
- 修改用户余额。
- 修改订单或成交结果。
- 查看非自己团队用户。
- 查看平台全局财务。
- 配置秒合约、杠杆、理财产品。
- 修改返佣规则。

### 7.4 认证域与数据范围

| 认证域 | 登录入口 | Token Scope | 数据范围 |
|---|---|---|---|
| 用户端 | /api/v1/auth/login | user | 当前用户本人 |
| 平台后台 | /admin/api/v1/auth/login | admin | 按 RBAC 查看全平台 |
| 代理后台 | /agent/api/v1/auth/login | agent | 当前代理团队 |

代理后台所有用户查询必须限制：

```sql
WHERE user_referrals.root_agent_id = current_agent_id
```

平台管理员后台查询默认不加 root_agent_id 限制，但受 RBAC 权限约束。

### 7.5 后台 API 前缀

| 后台 | 路由前缀 | 示例 |
|---|---|---|
| 平台管理员后台 | /admin/api/v1/* | /admin/api/v1/users, /admin/api/v1/market-strategies |
| 代理后台 | /agent/api/v1/* | /agent/api/v1/users, /agent/api/v1/team-tree |
| 用户端 | /api/v1/* | /api/v1/spot/orders, /api/v1/wallet/accounts |

### 7.6 代理后台 API

| API | 说明 |
|---|---|
| POST /agent/api/v1/auth/login | 代理后台登录 |
| GET /agent/api/v1/dashboard | 代理仪表盘 |
| GET /agent/api/v1/users | 团队用户 |
| GET /agent/api/v1/invite-codes | 邀请码列表 |
| POST /agent/api/v1/invite-codes | 创建邀请码 |
| PATCH /agent/api/v1/invite-codes/{id}/status | 启用/禁用邀请码 |
| GET /agent/api/v1/commissions | 返佣记录 |
| GET /agent/api/v1/team-tree | 团队邀请树 |

### 7.7 平台管理员新币 API

| API | 说明 |
|---|---|
| POST /admin/api/v1/new-coins | 创建新币 |
| PATCH /admin/api/v1/new-coins/{id}/lifecycle | 调整预热、发行、派发、上市阶段 |
| POST /admin/api/v1/new-coins/{id}/distribute | 执行派发 |
| PATCH /admin/api/v1/new-coins/{id}/unlock-rule | 配置解禁规则 |
| GET /admin/api/v1/new-coins/{id}/subscriptions | 查看申购记录 |
| GET /admin/api/v1/new-coins/{id}/distributions | 查看派发记录 |

### 7.8 平台管理员代理 API

| API | 说明 |
|---|---|
| POST /admin/api/v1/agents | 创建代理 |
| PATCH /admin/api/v1/agents/{id}/status | 启用/禁用代理 |
| GET /admin/api/v1/agents/{id}/users | 查看代理团队用户 |
| PATCH /admin/api/v1/users/{id}/agent | 调整用户代理归属 |
| GET /admin/api/v1/agent-commissions | 代理返佣统计 |

## 8. 风控、安全与合规边界

### 8.1 风控规则

| 场景 | 规则 |
|---|---|
| 注册/登录 | IP 频率限制、设备指纹预留、异常登录提醒 |
| 下单 | 最小下单额、价格偏离限制、数量精度、频率限制 |
| 撤单 | 高频撤单限制、异常撤单监控 |
| 资产 | 余额变动必须有流水，禁止无来源改余额 |
| 新币申购 | 仅发行阶段开放；限制申购额度、重复申购和资金不足 |
| 新币派发 | 派发必须幂等；获配、退款、锁定、解禁均写资产流水 |
| 提现 | 一期预留；二期支持地址白名单、审核、冷热钱包 |
| 后台操作 | 敏感操作二次确认、RBAC、全量审计日志 |
| 策略行情 | 仅内部交易对可用，限制价格跳变和参数越界 |
| 代理后台 | 只读团队数据，禁止修改用户资产、订单、行情策略 |

### 8.2 安全设计

| 层级 | 设计 |
|---|---|
| 密码 | Argon2 或 bcrypt 哈希 |
| 用户认证 | JWT access token + refresh token |
| 后台认证 | 独立管理员登录体系 + RBAC |
| 代理认证 | 独立 agent admin 登录体系 |
| API | 请求限流、参数校验、统一错误码 |
| WebSocket | 公共频道限流，私有频道鉴权 |
| MySQL | 关键字段索引、事务控制、资产变更行级锁 |
| MongoDB | K 线唯一索引、行情归档 TTL、按 symbol 与时间建立查询索引 |
| Redis | 设置 TTL，避免无界增长 |
| RabbitMQ | 事件幂等 key、失败重试、死信队列 |
| 审计 | 后台、新币策略、代理归属变更必须审计 |

### 8.3 合规边界

外部行情交易对：

- BTC/USDT、ETH/USDT 等主流币只展示 Bitget / HTX 行情。
- 不允许后台控制外部真实行情。

平台新币交易对：

- 使用内部策略行情。
- 新币在预热、发行、派发阶段不展示交易 K 线。
- 新币到达上市时间且交易对启用后，K 线开始生成和展示。
- 一期可生成 ticker、trade、K 线、成交量模拟。
- 系统必须区分 external、internal、strategy 市场。

后台行情策略：

- 仅用于新币或内部测试交易对。
- 启停和参数修改必须审计。
- 必须经过 Risk Guard 校验。

代理后台：

- 代理只能查看和统计归属用户。
- 代理不具备平台总后台权限。
- 返佣结算必须保留流水和审核状态。

## 9. 测试与验收

### 9.1 测试设计

| 类型 | 覆盖内容 |
|---|---|
| 单元测试 | 价格精度、下单校验、资产冻结/解冻、邀请归属计算 |
| 集成测试 | 下单、撮合、成交、资产流水完整链路 |
| 行情测试 | Bitget / HTX adapter 标准化、断线重连、主备切换 |
| 新币生命周期测试 | 预热不可申购、发行可申购、派发到账、上市后 K 线出现、锁定与解禁正确 |
| 策略行情测试 | 目标价生成、波动率限制、K 线聚合、越界拒绝、停机后缺口补偿 |
| 权限测试 | 用户、平台后台、代理后台权限隔离 |
| 安全测试 | JWT 过期、重复请求、限流、敏感操作审计 |
| 数据一致性测试 | 钱包余额与流水汇总一致、订单状态一致 |
| WebSocket 测试 | 订阅、取消订阅、私有频道鉴权、断线恢复 |

### 9.2 一期验收标准

| 模块 | 验收条件 |
|---|---|
| 用户 | 可注册、登录、获取个人信息 |
| 资产 | 可查看账户；下单冻结、撤单解冻、成交结算正确 |
| 行情 | 可接入 Bitget / HTX；Redis 有实时 ticker / K 线缓存；MongoDB 可保存行情归档 |
| K 线 | 支持 1m、5m、15m、1h、1d 查询；历史 K 线从 MongoDB 读取 |
| 现货 | 可下限价单、市价单、撤单、查看订单与成交 |
| 新币 | 后台可配置预热、发行申购、派发、上市、解禁规则和策略行情；上市后 K 线出现；服务重启后可补齐策略 K 线缺口 |
| 平台后台 | 可管理用户、币种、交易对、策略、审计 |
| 代理 | 可创建代理、邀请码；用户注册绑定；代理后台查看团队用户 |
| 权限 | 用户端、平台后台、代理后台权限隔离 |
| 审计 | 敏感后台操作可追溯 |
| 稳定性 | 行情断线可重连；RabbitMQ 消费失败可重试 |

## 10. 路线图

| 阶段 | 范围 |
|---|---|
| 一期 MVP | 用户、资产、行情、K 线、现货、新币策略行情、平台后台、代理后台 |
| 二期交易增强 | 秒合约、杠杆、理财、做市机器人 |
| 三期链上能力 | 充值提现、钱包归集、链上风控 |
| 四期高并发拆分 | 撮合、行情、资产、风控拆分为独立服务 |
| 五期运营增强 | 多级返佣、代理等级、活动、数据报表 |

## 11. 关键约束

- 一期不做微服务，先采用模块化单体。
- 平台新币策略行情必须与外部真实行情隔离。
- 平台新币 K 线生成必须基于持久化策略、版本和检查点，不能只依赖内存定时器。
- 用户资产变动必须有流水。
- MySQL 保存核心交易业务数据，MongoDB 保存行情与 K 线历史，Redis 保存实时行情缓存。
- 平台管理员后台与代理后台必须使用不同认证域和权限边界。
- 代理后台默认只读团队数据，不能修改用户资产、订单或行情。
- 高风险后台操作必须审计。
- 后续扩展模块不得破坏一期核心账户、订单、行情边界。
