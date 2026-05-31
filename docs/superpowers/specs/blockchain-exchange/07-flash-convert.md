# 07. 闪兑设计

## 1. 产品定位

闪兑是用户端快速兑换功能：

- 用户选择 from_asset 和 to_asset。
- 系统给出兑换报价、手续费、预计到账数量、报价有效期。
- 用户确认后，系统直接完成资产扣减和入账。
- 不展示订单簿，不要求用户理解买卖方向、限价、市价或盘口深度。

## 2. 设计模式

闪兑采用混合模式：

| 资产类型 | 报价来源 | 成交方式 |
|---|---|---|
| 主流币 → 主流币 | Bitget / HTX 行情 + 平台价差 | 平台报价成交 |
| 主流币 → USDT | 外部行情 + 平台价差 | 平台报价成交 |
| USDT → 主流币 | 外部行情 + 平台价差 | 平台报价成交 |
| 新币 ↔ USDT | 后台配置汇率 / 浮动规则 | 平台内部兑换 |
| 新币 ↔ 主流币 | 建议通过 USDT 中转 | 先换 USDT，再换目标币 |
| 禁用币种 | 不允许闪兑 | 返回不可兑换 |

## 3. 闪兑流程

1. 用户请求报价：from_asset、to_asset、from_amount。
2. 系统校验币种状态、闪兑开关、限额、用户状态。
3. 报价引擎获取价格：
   - 主流币：读取 Redis 最新 ticker 或外部行情适配层。
   - 新币：读取后台配置的闪兑汇率规则。
4. 系统计算基础兑换数量、平台点差、手续费、最终到账数量和报价过期时间。
5. 报价写入 Redis，设置短 TTL，例如 10 秒。
6. 用户确认兑换。
7. 系统校验报价未过期、金额未变、用户余额足够。
8. MySQL 事务内扣减 from_asset、增加 to_asset、写两边资产流水、写闪兑记录。
9. 发布 RabbitMQ `convert.completed` 事件。
10. WebSocket 推送资产变化。

## 4. 数据模型

新增 MySQL 表：

| 表 | 作用 | 关键字段 |
|---|---|---|
| convert_pairs | 闪兑交易对配置 | from_asset, to_asset, enabled, min_amount, max_amount, fee_rate, spread_rate, quote_source |
| convert_quotes | 闪兑报价记录 | quote_id, user_id, from_asset, to_asset, from_amount, to_amount, rate, fee, expires_at, status |
| convert_orders | 闪兑成交记录 | id, quote_id, user_id, from_asset, to_asset, from_amount, to_amount, rate, fee, status, created_at |
| new_coin_convert_rules | 新币闪兑规则 | asset_id, base_asset_id, rate, rate_mode, min_amount, max_amount, enabled, effective_at |

Redis Key：

- convert:quote:{quote_id}
- convert:limit:user:{user_id}
- convert:rate:{from}:{to}

## 5. 资产处理规则

- 闪兑只允许使用 available 余额。
- locked 余额不能参与闪兑。
- 确认闪兑必须在 MySQL 事务内完成扣减、入账、流水和订单记录。
- 同一 quote_id 只能成交一次。
- from_asset 扣减和 to_asset 增加必须分别写 wallet_ledger。
- 闪兑失败不能产生半完成资产状态。

## 6. 报价规则

主流币报价：

- 优先读取 Redis 最新 ticker。
- Redis 缺失时可从行情适配层主动刷新。
- 行情过期或偏离阈值过大时拒绝报价。
- 平台可配置 spread_rate 和 fee_rate。

新币报价：

- 读取 new_coin_convert_rules。
- 支持固定汇率或后台配置的浮动规则。
- 新币未上市时默认不能闪兑，除非后台单独开启。
- 新币汇率修改必须审计。

## 7. 平台后台管理

| 功能 | 说明 |
|---|---|
| 闪兑开关 | 全局启停闪兑 |
| 闪兑交易对 | 配置哪些币可以互换 |
| 手续费配置 | 每个兑换对配置 fee_rate |
| 点差配置 | 每个兑换对配置 spread_rate |
| 新币汇率配置 | 配置新币和 USDT 的兑换汇率 |
| 单笔限额 | 最小 / 最大兑换金额 |
| 用户限额 | 单用户每日 / 每小时额度 |
| 风控开关 | 禁止异常用户使用闪兑 |
| 闪兑记录 | 查询报价、成交、失败原因 |

代理后台：

- 默认只能查看团队用户闪兑统计。
- 不能修改汇率、手续费、限额。
- 闪兑手续费可以进入二期代理返佣统计。

## 8. API

用户端：

| API | 说明 |
|---|---|
| GET /api/v1/convert/pairs | 可闪兑币对 |
| POST /api/v1/convert/quote | 获取报价 |
| POST /api/v1/convert/confirm | 确认闪兑 |
| GET /api/v1/convert/orders | 我的闪兑记录 |

平台后台：

| API | 说明 |
|---|---|
| GET /admin/api/v1/convert/pairs | 闪兑币对配置 |
| POST /admin/api/v1/convert/pairs | 新增闪兑币对 |
| PATCH /admin/api/v1/convert/pairs/{id} | 修改开关、手续费、限额 |
| POST /admin/api/v1/convert/new-coin-rules | 配置新币汇率 |
| GET /admin/api/v1/convert/orders | 闪兑订单记录 |

代理后台：

| API | 说明 |
|---|---|
| GET /agent/api/v1/convert/stats | 团队闪兑统计 |

## 9. RabbitMQ 事件

| Exchange | Routing Key | 说明 |
|---|---|---|
| convert.events | convert.quoted | 报价生成，可选 |
| convert.events | convert.completed | 闪兑成交 |
| convert.events | convert.failed | 闪兑失败 |
| wallet.events | wallet.balance_changed | 资产变化 |

## 10. 风控边界

| 风险 | 规则 |
|---|---|
| 报价过期 | 报价 TTL，例如 10 秒 |
| 重复提交 | quote_id 幂等，只能成交一次 |
| 余额不足 | 确认时再次校验 available |
| 汇率异常 | 主流币价格偏离外部行情阈值则拒绝 |
| 新币汇率 | 只能由平台后台配置，必须审计 |
| 限额 | 单笔、单用户、单币种、全局限额 |
| 库存 | 平台需要有足够兑换库存或内部账户 |
| 锁定资产 | locked 余额不能参与闪兑 |
| 生命周期 | 新币未上市时默认不能闪兑，除非后台单独开启 |
| 审计 | 汇率、手续费、限额、开关修改必须审计 |

## 11. 验收标准

| 场景 | 验收 |
|---|---|
| 主流币闪兑 | 可按外部行情生成有效期报价并成交 |
| 新币闪兑 | 可按后台配置汇率生成报价并成交 |
| 报价过期 | 过期报价不能成交 |
| 重复确认 | 同一 quote_id 只能成交一次 |
| 余额变动 | from_asset 扣减、to_asset 增加、流水完整 |
| 锁定资产 | locked 余额不能兑换 |
| 后台配置 | 修改汇率、手续费、限额必须审计 |
| 权限隔离 | 代理后台不能修改闪兑配置 |
