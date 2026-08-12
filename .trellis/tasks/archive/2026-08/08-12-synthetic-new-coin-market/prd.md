# 新币确定性模拟行情与后台手动 K 线补偿

## Goal

在现有 Rust 行情策略基础上，为 `market_type=strategy/internal` 的新币交易对提供可审计、可重放的实时模拟行情。管理员可以配置多个未来目标节点（指定时间达到指定价格或相对涨跌幅），服务在线时持续生成 ticker 与 1 分钟 K 线并通过现有 Redis、MongoDB、WebSocket 链路发布；服务停机造成的历史缺口只在后台管理员预览并确认后补偿，不再由启动循环自动补写。

## What I Already Know

- 数据库已有 `market_strategies`、`strategy_runs`、`strategy_versions`、`strategy_events`，并保存价格、时间、波动率、成交量、版本 seed 与运行检查点。
- 当前 `src/workers/kline_recovery.rs` 只按当前价到目标价线性插值，自动每 30 秒扫描补线，不使用版本 seed，不同步 ticker/实时 WebSocket，也不生成实时 K 线。
- 行情统一摄取入口 `MarketIngestionService` 已支持 Redis ticker/Kline、Mongo Kline、现货订单触发和公开 WebSocket 事件。
- 后台已有行情策略 CRUD/状态操作页，适合扩展节点编辑、缺口预览、手动补偿与补偿历史。
- 用户已确定：历史 K 线补偿必须由后台手动触发。

## Requirements

### 策略与目标节点

- 行情策略创建/修改支持有序 `nodes`，每个节点包含目标时间、目标类型、目标值、执行模式、容差、局部波动率和可选成交量区间。
- MVP 支持目标类型：`absolute_price`、`percent_from_start`、`percent_from_previous`。
- MVP 支持执行模式：`hard`、`soft`、`range`；硬节点对应分钟的闭合价精确命中计算后的目标价，软/范围节点在容差内。
- 节点时间严格递增、位于策略起止时间内并对齐 UTC 分钟；首节点之前由起始价连接，最后节点之后由最后节点连接到旧 `target_price/end_time` 兼容终点。
- 节点以关系表保存，完整节点快照同时进入 `strategy_versions.config_json`；策略更新仍要求先暂停或禁用。

### 确定性生成

- 参考 market-data-emulator 的 seeded OU/场景思想和 PriceGenerator 的实体、影线、异常值、成交量参数，但使用 Rust 原生实现，不引入 Python/Node/Swift 运行时。
- 每个分钟槽位随机性由 `strategy seed + version + symbol + open_time` 派生，跨重启、重试、批次划分及多实例竞争时输出一致。
- 价格由目标桥接趋势、确定性噪声、均值回归和受控影线组成；必须满足价格为正、`high >= max(open, close)`、`low <= min(open, close)`、volume 非负及交易对价格精度。
- 只生成权威 1m K 线；5m/15m/1h/4h/1d 从 1m 聚合，避免各周期独立随机导致不一致。

### 实时运行

- 新增模拟行情实时 worker，只扫描 active 策略并生成当前分钟的实时更新与上一分钟的闭合 K 线。
- 实时生成通过现有 Market ingestion 与 MarketFeedEvent 广播合同写入 Redis ticker/Kline、Mongo 历史并发布 ticker/kline WebSocket。
- ticker 最新价必须等于模拟行情当前价，24h high/low/volume/change 从策略 1m 历史计算或维护一致快照。
- 服务重启时不自动补写停机期间历史 K 线；worker 从当前分钟恢复实时行情，历史缺口保持可检测状态。
- 同一策略只允许一个实例拥有短租约；所有写入以时间槽幂等，旧时间数据不得倒退当前 Redis ticker。

### 后台手动补偿

- 删除 `main.rs` 中自动 `kline_recovery::run_loop` 启动；保留配置兼容但不再自动扫描历史缺口。
- 提供管理员接口：缺口检测、补偿预览、确认执行和补偿任务历史。
- 预览返回缺口范围、1m 根数、聚合影响范围、首尾价格及有限样本，不写 Mongo/Redis/检查点。
- 执行必须带审计原因和预览令牌（配置版本/范围摘要），防止预览后策略变化造成盲写。
- 补偿只写历史闭合 1m 与聚合周期，不覆盖或倒退实时 ticker；重复执行按 interval+open_time 幂等收敛。
- 补偿任务保存操作者、范围、状态、预计/实际根数、配置版本、错误和时间；所有执行进入策略事件和管理员审计。

### 后台界面

- 创建与修改行情策略共用节点编辑组件，使用中文标签、增删行、目标类型/执行模式下拉框和日期时间输入。
- 行操作新增“检测缺口/补偿K线”，SideSheet 显示缺口、预览样本、确认原因和任务历史。
- 操作按钮保持现有小尺寸、单行和可访问名称；继续使用 Semi Design 与共享管理后台视觉合同。

## Acceptance Criteria

- [x] 同一 seed/version/symbol/open_time 在实时、预览和执行路径生成完全一致的 OHLCV。
- [x] 硬目标节点分钟收盘价精确命中目标；百分比节点按正确基准计算。
- [x] 生成结果满足 OHLCV 不变量、价格精度和正数约束。
- [x] 实时 worker 写 Redis ticker/Kline、Mongo 1m 历史并发布现有 ticker/kline WebSocket。
- [x] 5m/15m/1h/4h/1d 均由 1m 聚合且跨重启一致。
- [x] 服务重启不会自动补写缺失历史，只恢复当前实时槽；后台能够检测缺口。
- [x] 未预览、令牌过期、策略版本变化、无缺口、非法范围及并发重复执行具有明确错误或幂等结果。
- [x] 手动补偿不倒退当前 ticker，不重复插入 K 线，并保存任务、策略事件和管理员审计。
- [x] 后台可以创建/编辑节点、预览缺口、确认补偿和查看任务状态。
- [x] 现有无节点策略 API 与旧数据保持兼容。
- [x] 后端与 web 聚焦测试、全量类型检查/构建、架构及中文文档门禁通过。

## Definition of Done

- 追加不可变 SQLx 迁移并覆盖 MySQL 元数据/约束测试。
- 后端纯算法单元测试、worker/路由集成测试和 Web 管理界面测试齐全。
- Rust 公开/可见方法具备详细中文职责、事务、幂等和副作用注释。
- Trellis 后端与后台契约更新，`docs/superpowers/PROGRESS.md` 更新。
- 验证命令全部成功，保留并不覆盖当前工作区无关的 Mobile Lightweight Charts 改动。

## Technical Approach

1. 在 `market` bounded context 新增纯领域模拟器：解析版本配置/节点，按槽位派生 SHA-256 seed，生成确定性价格路径与 OHLCV。
2. 新增迁移保存 `market_strategy_nodes`、`kline_recovery_jobs`，并扩展运行检查点/租约字段；后台 CRUD 在单事务内同步节点与版本快照。
3. 新增实时 `synthetic_market` worker，复用 `MarketIngestionService` 和 `MarketFeedEvent`，避免建立第二套缓存/广播协议。
4. 将现有 `kline_recovery` 改为显式范围的预览/执行服务；执行只处理历史槽并重建受影响聚合周期。
5. 扩展 admin routes/application/infrastructure/presentation 及 Web `MarketStrategyActions`。

## Decision (ADR-lite)

**Context**：项目已具备 Rust 行情摄取、策略表与恢复检查点，额外 Python 服务会增加部署和双协议一致性成本；停机缺口是否补写需要管理员控制。

**Decision**：采用 Rust 原生确定性槽位生成器；每个时间槽独立派生随机性，目标节点作为分段桥接锚点；实时与补偿共用同一生成函数；历史补偿只允许管理员预览后执行。

**Consequences**：结果可重放且容易审计，服务重启不会改变历史；模拟真实性优先满足产品展示与测试，而不是 GAN 级市场统计拟合。后台增加节点和补偿任务管理复杂度，但不会引入新运行时。

## Out of Scope

- GAN/VAE 训练、外部 Python 推理服务和真实历史数据拟合。
- 模拟完整 L2 订单簿或伪造平台真实成交记录；MVP 发布 ticker/Kline，现货真实用户成交仍走现有撮合逻辑。
- 管理员直接拖动画布操纵每一根蜡烛。
- 自动补偿定时任务；所有停机历史缺口由管理员确认。
- 在本任务中修改 Mobile Lightweight Charts 的在途未提交改动。

## Research References

- [PriceGenerator](https://github.com/Tim55667757/PriceGenerator) — 参数化蜡烛实体、影线、离群值和成交量。
- [market-data-emulator](https://github.com/elriseio/market-data-emulator) — seeded OU、场景、多周期聚合、确定性与不变量。
- [trade-data-generator](https://github.com/monupareeklg/trade-data-generator) — ticker/candle/depth 统一事件源结构。

## Technical Notes

- 现有策略迁移：`migrations/0004_market_pairs_strategy.sql`。
- 现有恢复实现：`src/workers/kline_recovery.rs`。
- 统一摄取：`src/modules/market/infrastructure/adapters/ingestion.rs`。
- 后台策略接口：`src/modules/admin/{presentation,application,infrastructure,service}/market.rs` 与 `src/modules/admin/routes/market_trading.rs`。
- Web 策略操作：`web/src/admin/resources/actions/market.tsx`。
- 当前 git 工作区已有独立 Mobile Lightweight Charts 改动，本任务不得重置或混入。
