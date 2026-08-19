# 手机端杠杆市价与限价选择及真实下单

## Goal

让手机端杠杆交易的订单类型控件像其他配置项一样通过底部弹窗选择“市价单 / 限价单”，并补齐后端真实限价挂单、持久化、行情触发成交、撤单与风险隔离，避免出现只切换界面但接口仍固定发送市价单的假能力。

## What I already know

- `mobile/src/views/TradeView.vue` 当前把合约订单类型强制为 `market`，控件处于 `disabled`，价格输入始终只读。
- 现货已有可访问、可关闭、支持 Escape/焦点恢复的订单类型底部弹窗，可复用交互规范；杠杆已有 `ContractTradeSheets.vue` 统一承载交易对、杠杆倍数和保证金模式弹窗。
- `GET /margin/products` 当前能力集只返回 `order_types=["market"]`，移动端适配器也未保留该能力。
- `POST /margin/positions` 当前显式拒绝限价语义并从 Redis 新鲜行情即时取得入场价。
- `margin_positions.entry_price IS NULL`、单笔/批量撤单和移动端订单页已经具备“未成交杠杆挂单”语义，但缺少真实创建和行情触发成交链路。
- 行情摄取层已经在 Redis CAS 接受最新 ticker 后触发现货限价单；杠杆限价单可复用同一可信行情驱动边界，而不依赖客户端价格或仅内存定时器。

## Requirements

- 杠杆订单类型控件可点击并打开独立底部弹窗，弹窗包含后端能力允许的市价单与限价单；打开、遮罩关闭、关闭按钮和 Escape 不改变当前选择，只有点击明确选项才提交选择并关闭。
- 弹窗、触发器和选中态遵循现有高质感移动端样式、Lucide 图标、44px 触控目标、浅色/深色主题、320–448px 无横向溢出和 reduced-motion 约束。
- 市价单价格字段保持只读并显示实时市价；限价单价格字段变为可编辑，支持通过 BBO/最新价按钮回填，限价价格必须为正且符合交易对价格精度。
- 移动端从 `/margin/products` 保留 `order_types` 能力，绝不自行宣称后端未返回的订单类型；默认优先使用市价单，产品/能力刷新后若当前类型失效则回落到首个真实能力。
- 确认弹窗冻结订单类型、限价价格、参考价格、保证金、杠杆、模式和幂等键；重试必须复用完全相同的冻结请求，异步行情变化不得改写已打开的确认内容。
- `POST /margin/positions` 支持：
  - `market`：不得带 `price`/`trigger_price`，按新鲜服务端行情即时成交；
  - `limit`：必须带正数 `price`、不得带 `trigger_price`；达到触发条件时按当前可信市场价即时成交，否则以 `entry_price=NULL` 持久化为可撤挂单并占用保证金。
- 限价做多在市场价不高于限价时触发；限价做空在市场价不低于限价时触发。成交价取触发该订单的可信服务端行情价，不得取客户端伪造价格。
- 未成交限价单持久化 `order_type` 与 `limit_price`；幂等重放必须把它们纳入同请求判定，同键异参返回冲突且不重复扣款。
- Redis CAS 接受新的 ticker 后触发杠杆限价挂单；每笔挂单独立事务、先锁仓位再更新，撤单与成交竞争只能成功一个。服务重启后挂单仍由数据库保留，并在下一次接受的 ticker 上继续触发。
- 未成交挂单不得计息、不得进入逐仓或全仓强平风险集合、不得产生代理返佣；成交事务才重置计息起点、建立所需全仓账户、登记一次返佣并发布成交事件。
- 用户仓位/API/移动端映射返回并展示真实 `order_type` 与 `limit_price`；首页交易区的“持仓”只统计已成交仓位，“委托”统计未成交挂单，订单页继续按可撤/可平仓分流。
- 保持现有现货下单、杠杆市价下单、钱包扣款/退款、批量撤单/平仓和历史数据兼容。

## Acceptance Criteria

- [ ] 点击杠杆订单类型控件会打开底部弹窗，市价/限价可明确选择，取消路径不修改值，焦点与滚动锁行为正确。
- [ ] 选择限价单后价格字段可编辑且请求包含 `order_type=limit` 与冻结的 `price`；选择市价单后请求不包含价格。
- [ ] 移动端仅使用后端 `order_types` 能力，默认与能力失效回退行为有测试。
- [ ] 限价做多/做空触发边界（等于、未达到、超过）有纯函数测试。
- [ ] 未触发限价单落库后 `entry_price=NULL`、可取消并原路退回保证金；未计息、未强平、未返佣。
- [ ] 新行情满足条件后挂单只成交一次、写入真实 `entry_price`、重置计息起点、返佣一次并发送一次成交事件；重复行情不重复处理。
- [ ] 同幂等键同参数重放不重复扣款；订单类型或限价价格不同返回冲突。
- [ ] 服务端能力变更为 `order_types=["market","limit"]`，PC 现有市价调用保持兼容。
- [ ] Rust 格式、架构检查、相关单元/集成测试以及移动端 type-check、全量测试、PWA/Tauri 构建通过。
- [ ] 浏览器在 390px 浅色/深色和 320px 窄屏验证弹窗、价格字段、确认层，无横向溢出、遮挡或不可见操作。

## Definition of Done

- 后端迁移、领域规则、应用事务、基础设施查询、行情触发、响应字段及测试完整闭环。
- 移动端能力适配、订单类型弹窗、价格输入、确认快照、订单/持仓分类、i18n 与测试完整闭环。
- 相关 Trellis 规范与 `docs/superpowers/PROGRESS.md` 更新。
- 不修改或纳入无关的 `mobile/pencil/docs/` 未跟踪目录。

## Technical Approach

1. 新增不可变迁移 `0106_margin_limit_orders.sql`，为 `margin_positions` 增加 `order_type`、`limit_price` 和触发索引，历史记录默认迁移为 `market`。
2. 在 margin domain/application 中建立唯一的订单类型与限价触发规则；开仓事务按 market/limit 分流，但共用产品、资金、幂等和钱包锁序。
3. 新增杠杆限价挂单查询和逐单成交事务，由 Market ingestion 在接受 ticker 后调用；成交事务负责更新入场价、计息起点、全仓账户、返佣和私有事件。
4. 所有计息与强平候选查询显式要求 `entry_price IS NOT NULL`，把 pending limit 与真实持仓隔离。
5. `MarginPositionResponse` 和所有读取 SQL 增加订单字段；能力改为 market+limit。
6. 移动端把 `orderTypes` 映射到 `MarginProduct`，扩展 `ContractTradeSheets` 的订单类型 sheet，并把选择值/限价写入冻结的 `MarginOrderReview` 与 API 请求。
7. 更新 Orders/Trade 分类显示、i18n、源合同测试、业务纯函数和后端集成测试。

## Decision (ADR-lite)

**Context**: 只新增前端弹窗会与当前 market-only 接口冲突；禁用限价选项也不满足“让用户选择”的要求。仓库已有未成交仓位、撤单以及行情触发现货限价单基础。

**Decision**: 同步交付真实杠杆限价单，采用数据库持久化挂单 + 权威 ticker 驱动整笔成交；不建立客户端撮合或内存计时器。

**Consequences**: 改动跨数据库、Rust 后端和移动端，但用户选择与真实能力一致；挂单可跨重启恢复。当前为整笔触发成交，不模拟订单簿部分成交。

## Out of Scope

- 止损单、止损限价、OCO、止盈止损联动。
- 部分成交、外部交易所真实委托回报或订单簿排队优先级。
- PC 端新增限价选择界面（现有 PC 市价下单继续兼容）。
- 后台按产品单独开关订单类型；本次能力为后端全局实现事实。

## Technical Notes

- 相关规范：`.trellis/spec/backend/margin-trading-actions.md`、`.trellis/spec/backend/quality-guidelines.md`、`.trellis/spec/mobile/backend-integration.md`、`.trellis/spec/mobile/pwa-and-shell.md`。
- 关键现有文件：`src/modules/margin/application/open_position.rs`、`src/modules/margin/infrastructure/positions.rs`、`src/modules/market/infrastructure/adapters/ingestion.rs`、`mobile/src/views/TradeView.vue`、`mobile/src/components/ContractTradeSheets.vue`、`mobile/src/core/marginOrderConfirmation.ts`。
- 必须保留上一任务已经实现的确认快照、同键重试和产品上下限竞态处理。
