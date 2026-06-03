# 04. 资产账户与现货交易设计

## 1. 资产账户设计

资产账户负责用户币种余额、冻结余额、锁定余额和资产流水。

| 表 | 作用 | 关键字段 |
|---|---|---|
| wallet_accounts | 用户币种账户汇总 | user_id, asset_id, available, frozen, locked, updated_at |
| asset_lock_positions | 新币锁定仓位 / 锁定订单 | id, user_id, asset_id, unlock_type, unlock_at, locked_amount, released_amount, remaining_amount, merge_key, status, created_at |
| asset_lock_position_sources | 锁定仓位来源 | lock_position_id, source_type, source_id, source_amount, source_time, created_at |
| wallet_ledger | 资产流水 | id, user_id, asset_id, change_type, amount, balance_type, balance_after, ref_type, ref_id, created_at |
| deposit_records | 充值记录预留 | user_id, asset_id, amount, tx_hash, status |
| withdraw_records | 提现记录预留 | user_id, asset_id, amount, fee, address, status |

资产原则：

- 禁止直接改余额不写流水。
- 下单先冻结，撤单解冻，成交后结算。
- `wallet_accounts.locked` 只保存锁定汇总余额，解禁排程和来源追踪以 `asset_lock_positions` 与 `asset_lock_position_sources` 为准。
- fixed_time 锁定可按 user_id + asset_id + unlock_at 聚合。
- relative_period 锁定必须按每笔申购、派发或认购来源拆分，因为每笔 unlock_at 不同。
- `wallet_accounts.locked` 必须等于该用户该币种 active 锁定仓位 remaining_amount 汇总。
- 新币派发、认购、锁定、解禁、矿工费、退款都必须写 wallet_ledger。
- wallet_ledger 必须能区分 affected bucket，例如 available、frozen、locked，或记录 available/frozen/locked 快照。
- 闪兑扣减 from_asset、增加 to_asset 都必须写 wallet_ledger。
- 用户下单、闪兑和转出只能使用 available，不允许使用 locked。
- 所有资产变动必须具备 ref_type / ref_id。

## 2. 现货下单流程

1. 用户提交限价单或市价单。
2. API 校验登录态、交易对状态、价格精度、数量精度、最小下单额。
3. 资产模块冻结对应资产。
4. 订单进入撮合模块。
5. 撮合成功后生成成交记录。
6. 资产模块按成交结果解冻、扣减、入账。
7. 写入订单、成交、资产流水。
8. RabbitMQ 发布 OrderMatched / BalanceChanged 事件。
9. WebSocket 推送订单状态、成交和资产变化。

## 3. 新币认购与解禁资产处理流程

1. 用户在新币 listed 状态下提交认购。
2. 系统校验认购开关、用户状态、支付资产余额、认购限额和价格来源。
3. MySQL 事务内扣减 pay_asset available，生成 new_coin_purchase_orders。
4. 根据币种 unlock_type，将认购的新币写入 available 或 locked。
5. 如果进入 locked，必须同步创建或合并 asset_lock_positions，并写入 asset_lock_position_sources。
6. fixed_time 按 user_id + asset_id + unlock_at 合并锁定仓位；relative_period 按每笔 purchase_order 单独创建锁定订单。
7. 写入 new_coin_purchase、锁定相关 wallet_ledger，并更新 wallet_accounts.locked 汇总。
8. 到达解禁时间后，如果未启用矿工费，系统可自动将 locked 转入 available，更新锁定仓位 remaining_amount。
9. 如果启用矿工费，用户先支付 unlock_fee_asset，写入 unlock_fee 流水。
10. 支付完成后执行解禁，将 locked 转入 available，更新 asset_lock_positions，写入 unlock_release 流水。
11. 支付矿工费和执行解禁必须幂等，避免重复扣费或重复释放。

## 4. 闪兑资产处理流程

1. 用户请求闪兑报价。
2. 系统校验闪兑币对、用户状态、余额、限额和 locked 余额限制。
3. 报价写入 Redis 并设置短 TTL。
4. 用户确认报价。
5. MySQL 事务内扣减 from_asset available，增加 to_asset available。
6. 分别写入 from_asset 和 to_asset 的 wallet_ledger。
7. 写入 convert_orders。
8. 发布 convert.completed 和 wallet.balance_changed 事件。
9. WebSocket 推送资产变化。

闪兑详细设计见 `07-flash-convert.md`。

## 5. 订单与成交表

| 表 | 作用 | 关键字段 |
|---|---|---|
| spot_orders | 现货订单 | id, user_id, pair_id, side, order_type, price, quantity, filled_quantity, status, created_at |
| spot_trades | 成交记录 | id, pair_id, buy_order_id, sell_order_id, price, quantity, fee, created_at |
| order_events | 订单事件 | order_id, event_type, payload_json, created_at |

订单状态：

- pending
- open
- partially_filled
- filled
- cancelled
- rejected

## 6. 闪兑表

| 表 | 作用 | 关键字段 |
|---|---|---|
| convert_pairs | 闪兑交易对配置 | from_asset, to_asset, enabled, min_amount, max_amount, fee_rate, spread_rate, quote_source |
| convert_quotes | 闪兑报价记录 | quote_id, user_id, from_asset, to_asset, from_amount, to_amount, rate, fee, expires_at, status |
| convert_orders | 闪兑成交记录 | id, quote_id, user_id, from_asset, to_asset, from_amount, to_amount, rate, fee, status, created_at |
| new_coin_convert_rules | 新币闪兑规则 | asset_id, base_asset_id, rate, rate_mode, min_amount, max_amount, enabled, effective_at |

## 7. 交易对表

| 表 | 作用 | 关键字段 |
|---|---|---|
| assets | 币种定义 | id, symbol, name, precision, asset_type, status |
| trading_pairs | 交易对 | id, base_asset, quote_asset, price_precision, qty_precision, min_order_value, status, market_type |

`market_type`：

- external：外部行情交易对，如 BTC/USDT。
- internal：平台内部交易对，如 NEW/USDT。
- strategy：平台策略行情交易对。

## 8. 用户端 API

| 模块 | API | 说明 |
|---|---|---|
| 认证 | POST /api/v1/auth/register | 注册，详见 `08-user-auth-security-api.md` |
| 认证 | POST /api/v1/auth/login | 登录，详见 `08-user-auth-security-api.md` |
| 认证 | POST /api/v1/auth/refresh | 刷新 token，详见 `08-user-auth-security-api.md` |
| 用户 | GET /api/v1/user/profile | 用户信息，含邮箱验证和资金密码状态，详见 `08-user-auth-security-api.md` |
| 用户 | POST /api/v1/user/email/bind-code | 发送绑定邮箱验证码，详见 `08-user-auth-security-api.md` |
| 用户 | POST /api/v1/user/email/bind | 绑定并验证邮箱，详见 `08-user-auth-security-api.md` |
| 用户 | PATCH /api/v1/user/password | 修改登录密码，详见 `08-user-auth-security-api.md` |
| 用户 | POST /api/v1/user/fund-password | 新建 6 位数字资金密码，详见 `08-user-auth-security-api.md` |
| 用户 | PATCH /api/v1/user/fund-password | 修改 6 位数字资金密码，详见 `08-user-auth-security-api.md` |
| 资产 | GET /api/v1/wallet/accounts | 用户资产列表 |
| 资产 | GET /api/v1/wallet/ledger | 资产流水 |
| 闪兑 | GET /api/v1/convert/pairs | 可闪兑币对 |
| 闪兑 | POST /api/v1/convert/quote | 获取报价 |
| 闪兑 | POST /api/v1/convert/confirm | 确认闪兑 |
| 闪兑 | GET /api/v1/convert/orders | 我的闪兑记录 |
| 现货 | POST /api/v1/spot/orders | 下单 |
| 现货 | DELETE /api/v1/spot/orders/{id} | 撤单 |
| 现货 | GET /api/v1/spot/orders | 订单列表 |
| 现货 | GET /api/v1/spot/trades | 成交记录 |

## 9. RabbitMQ 事件

| Exchange | Routing Key | 说明 |
|---|---|---|
| order.events | order.created | 订单创建 |
| order.events | order.matched | 订单成交 |
| convert.events | convert.completed | 闪兑成交 |
| wallet.events | wallet.balance_changed | 资产变化 |

## 10. 风控规则

| 场景 | 规则 |
|---|---|
| 下单 | 最小下单额、价格偏离限制、数量精度、频率限制 |
| 撤单 | 高频撤单限制、异常撤单监控 |
| 资产 | 余额变动必须有流水，禁止无来源改余额 |
| 新币认购 | listed 状态才可认购；认购所得按币种解禁规则进入 available 或 locked |
| 解禁矿工费 | 启用后必须支付矿工费才可从 locked 转 available；支付和释放必须幂等 |
| 闪兑 | 报价 TTL、quote_id 幂等、available 余额校验、locked 不可兑换、单笔与单日限额 |
| 提现 | 一期预留；二期支持地址白名单、审核、冷热钱包 |

## 11. 验收条件

| 模块 | 验收条件 |
|---|---|
| 用户 | 可注册、登录、获取个人信息 |
| 资产 | 可查看账户；下单冻结、撤单解冻、成交结算正确 |
| 现货 | 可下限价单、市价单、撤单、查看订单与成交 |
| 新币认购 | listed 状态可认购；认购、锁定、矿工费、解禁流水完整 |
| 闪兑 | 可获取报价、确认兑换、扣减 from_asset、增加 to_asset、流水完整 |
| 数据一致性 | 钱包余额与流水汇总一致、订单状态一致 |
