# 05. 平台后台、代理后台与权限设计

## 1. 后台类型

| 后台 | 使用者 | 定位 |
|---|---|---|
| 平台管理员后台 | 平台运营、财务、风控、超级管理员 | 管理整个平台 |
| 代理后台 | 代理商、渠道负责人 | 管理自己名下用户与业绩 |
| 用户端 | 普通用户 | 交易、资产、邀请 |

## 2. 平台管理员后台功能

| 模块 | 能力 |
|---|---|
| 用户管理 | 查看全平台用户，冻结/解冻用户，调整状态，查看订单和资产 |
| 代理管理 | 创建代理、禁用代理、重置代理账号、调整代理归属 |
| 币种管理 | 创建币种、修改精度、上下架币种 |
| 新币发行管理 | 配置预热、发行申购、派发、上市、上市后认购、解禁规则、锁定仓位、解禁矿工费 |
| 交易对管理 | 创建交易对、上下架交易对、配置最小下单额和精度 |
| 行情源管理 | 配置 Bitget / HTX 主备源、启停行情源 |
| 新币策略行情 | 创建、修改、启动、停止新币策略行情 |
| 订单管理 | 查看全平台订单、成交、异常订单 |
| 资产管理 | 查看全平台用户资产、流水、充值提现记录 |
| 风控管理 | 配置限额、限频、交易对风控、用户风控 |
| 闪兑管理 | 配置闪兑开关、币对、手续费、点差、新币汇率、限额、订单查询 |
| 产品配置 | 二期配置秒合约、杠杆、理财 |
| 审计日志 | 查看管理员和代理关键操作 |
| 系统配置 | 手续费、交易时间、公告、参数配置 |
| 财务报表 | 平台交易额、手续费、资产统计、代理返佣统计 |

平台管理员高风险操作必须具备 RBAC、二次确认、审计日志和操作原因。

## 3. 代理关系模型

代理关系采用混合模式：

- 保存完整邀请树。
- 同时保存 root_agent_id。
- 一期代理后台按 root_agent_id 查看团队。
- 二期扩展多级返佣和团队业绩。

关系规则：

| 场景 | 归属 |
|---|---|
| 代理 A 邀请用户 B | B 的 direct_inviter_id = A，root_agent_id = A |
| 用户 B 邀请用户 C | C 的 direct_inviter_id = B，root_agent_id = A |
| 用户 C 邀请用户 D | D 的 direct_inviter_id = C，root_agent_id = A |
| 平台自然注册用户 | direct_inviter_id = null，root_agent_id = null |
| 后台手动分配代理 | 记录审计日志，可设置或迁移 root_agent_id |

## 4. 代理后台功能

| 模块 | 能力 |
|---|---|
| 代理仪表盘 | 团队用户数、注册量、交易量、手续费贡献、返佣预估 |
| 我的用户 | 只查看 root_agent_id = 当前代理 的用户 |
| 用户详情 | 查看团队用户基本信息、注册时间、状态、交易统计 |
| 邀请码管理 | 创建、禁用、查看自己的邀请码 |
| 邀请关系 | 查看团队邀请树、直属用户、间接用户 |
| 团队交易统计 | 查看团队现货交易额、订单数、手续费 |
| 团队闪兑统计 | 查看团队闪兑金额、手续费贡献、次数 |
| 团队资产统计 | 只读汇总统计，敏感明细可配置隐藏 |
| 返佣记录 | 查看返佣明细、待结算、已结算状态 |
| 账号安全 | 修改自己的登录密码、查看登录记录 |

代理后台禁止：

- 创建或修改币种。
- 创建或修改交易对。
- 启动或停止新币策略行情。
- 修改用户余额。
- 修改订单或成交结果。
- 修改闪兑汇率、手续费、限额或开关。
- 修改新币认购、解禁规则、锁定仓位或解禁矿工费配置。
- 查看非自己团队用户。
- 查看平台全局财务。
- 配置秒合约、杠杆、理财产品。
- 修改返佣规则。

## 5. 数据表

| 表 | 作用 | 关键字段 |
|---|---|---|
| users | 用户基础信息 | id, email, phone, password_hash, status, kyc_level, created_at |
| user_security | 用户安全配置 | user_id, fund_password_hash, totp_enabled, anti_phishing_code |
| admin_users | 平台管理员 | id, username, password_hash, role_id, status |
| admin_roles | 平台后台角色 | id, name, permissions |
| admin_audit_logs | 平台后台审计 | admin_id, action, target_type, target_id, before_json, after_json, ip, created_at |
| agents | 代理主体 | id, user_id, agent_code, level, status, created_at |
| invite_codes | 邀请码 | id, owner_type, owner_id, code, usage_limit, used_count, status |
| user_referrals | 用户邀请关系 | user_id, direct_inviter_id, direct_inviter_type, root_agent_id, depth, path, created_at |
| agent_admin_users | 代理后台账号 | id, agent_id, username, password_hash, status, last_login_at |
| agent_audit_logs | 代理后台审计 | agent_id, agent_admin_id, action, target_type, target_id, ip, created_at |
| agent_commission_rules | 返佣规则预留 | agent_id, product_type, commission_rate, status |
| agent_commission_records | 返佣记录预留 | agent_id, user_id, source_type, source_amount, commission_amount, status |
| convert_pairs | 闪兑交易对配置 | from_asset, to_asset, enabled, min_amount, max_amount, fee_rate, spread_rate, quote_source |
| convert_orders | 闪兑成交记录 | id, quote_id, user_id, from_asset, to_asset, from_amount, to_amount, rate, fee, status, created_at |
| new_coin_purchase_orders | 上市后认购记录 | id, user_id, asset_id, pay_asset_id, pay_amount, quantity, purchase_price, purchase_cost, lock_status, unlock_rule_id, status, created_at |
| asset_lock_positions | 新币锁定仓位 / 锁定订单 | id, user_id, asset_id, unlock_type, unlock_at, locked_amount, released_amount, remaining_amount, merge_key, status, created_at |
| asset_lock_position_sources | 锁定仓位来源 | lock_position_id, source_type, source_id, source_amount, source_time, created_at |
| asset_unlock_records | 新币解禁记录 | id, user_id, asset_id, lock_position_id, unlock_quantity, unlock_fee_basis, unlock_fee_asset, unlock_fee_amount, fee_paid_status, status |

## 6. 认证域与数据范围

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

平台管理员可查看用户当前锁定仓位、来源明细、解禁矿工费、释放记录和修正历史，但禁止直接修改 `wallet_accounts.locked`。如需修正，必须在同一事务内写审计日志、wallet_ledger，并调整锁定仓位和来源记录。

## 7. 后台 API 前缀

| 后台 | 路由前缀 | 示例 |
|---|---|---|
| 平台管理员后台 | /admin/api/v1/* | /admin/api/v1/users, /admin/api/v1/market-strategies |
| 代理后台 | /agent/api/v1/* | /agent/api/v1/users, /agent/api/v1/team-tree |
| 用户端 | /api/v1/* | /api/v1/spot/orders, /api/v1/wallet/accounts |

## 8. 代理后台 API

| API | 说明 |
|---|---|
| POST /agent/api/v1/auth/login | 代理后台登录 |
| GET /agent/api/v1/dashboard | 代理仪表盘 |
| GET /agent/api/v1/users | 团队用户 |
| GET /agent/api/v1/invite-codes | 邀请码列表 |
| POST /agent/api/v1/invite-codes | 创建邀请码 |
| PATCH /agent/api/v1/invite-codes/{id}/status | 启用/禁用邀请码 |
| GET /agent/api/v1/commissions | 返佣记录 |
| GET /agent/api/v1/convert/stats | 团队闪兑统计 |
| GET /agent/api/v1/team-tree | 团队邀请树 |

## 9. 平台管理员新币 API

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
| GET /admin/api/v1/new-coins/lock-positions | 查看锁定仓位与来源明细 |
| GET /admin/api/v1/new-coins/unlocks | 查看解禁记录 |

## 10. 平台管理员闪兑 API

| API | 说明 |
|---|---|
| GET /admin/api/v1/convert/pairs | 闪兑币对配置 |
| POST /admin/api/v1/convert/pairs | 新增闪兑币对 |
| PATCH /admin/api/v1/convert/pairs/{id} | 修改开关、手续费、限额 |
| POST /admin/api/v1/convert/new-coin-rules | 配置新币汇率 |
| GET /admin/api/v1/convert/orders | 闪兑订单记录 |

## 11. 平台管理员代理 API

| API | 说明 |
|---|---|
| POST /admin/api/v1/agents | 创建代理 |
| PATCH /admin/api/v1/agents/{id}/status | 启用/禁用代理 |
| GET /admin/api/v1/agents/{id}/users | 查看代理团队用户 |
| PATCH /admin/api/v1/users/{id}/agent | 调整用户代理归属 |
| GET /admin/api/v1/agent-commissions | 代理返佣统计 |

## 12. 用户端邀请 API

| API | 说明 |
|---|---|
| GET /api/v1/referral/my-code | 我的邀请码 |
| POST /api/v1/referral/bind | 绑定邀请码 |
| GET /api/v1/referral/my-invites | 我的邀请记录 |

## 13. 审计边界

| 操作来源 | 审计表/字段 |
|---|---|
| 平台管理员操作 | admin_audit_logs |
| 代理后台操作 | agent_audit_logs 或 audit_logs.actor_type = agent_admin |
| 用户操作 | 关键安全行为进入 user_security_logs |

代理后台审计重点：

- 登录/退出
- 创建邀请码
- 禁用邀请码
- 查看闪兑统计
- 查看敏感报表
- 导出数据
- 修改代理后台账号密码
