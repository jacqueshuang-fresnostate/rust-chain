# Research: 后端真实已实现收益历史

- Query: 检查现有四类收益 SQL、相关表时间字段/索引、市场 K 线或历史价格持久化、动态聚合与快照选择、1/7/30/180 日性能、非稳定币历史估值和 partial 语义，并给出可审计实现、接口 DTO、风险与测试矩阵。
- Scope: mixed
- Date: 2026-08-09

## Findings

### 1. 结论摘要

1. **收益事实继续动态聚合，历史换价必须冻结快照。** 四类终态业务表已经是最接近事实源的记录，不建议首版再复制一份“用户每日收益真相”；但 Mongo K 线会按同一 `(interval, open_time)` 原地 upsert，历史估值若每次直接读取 Mongo 会随行情修订、来源切换或恢复任务而漂移，达不到可重复审计。因此推荐：
   - MySQL 四类业务事实按请求周期动态聚合为 `UTC 日 × 资产`；
   - 已结束 UTC 日使用**不可变的日终 USDT 价格快照**；
   - 当前 UTC 日继续沿用已实现的 60 秒新鲜 Redis ticker；
   - 仅当 180 日真实高频账户压测不达标时，再引入带水位和版本的用户日事实快照。
2. **历史非稳定币不得按当前价回算。** 过去日期应使用该 UTC 日已冻结的 `1d` 收盘价；当前日可使用当前 ticker。缺少历史收盘价时必须 `partial`，不得退化为当前价、最近任意价或零价。
3. **现有索引不匹配结算时间。** 四个分支都以 `user_id + 终态 + 结算时间范围` 过滤，但现有用户索引主要落在 `created_at`；1 日查询在老账户上也可能扫描该用户的全部历史，180 日风险最高。应新增结算时间复合索引，并用 MySQL 8.4 `EXPLAIN ANALYZE` 验证，而不是只看测试耗时。
4. **UTC 还有连接级缺口。** 所有相关列都是 `TIMESTAMP(6)`，但连接池没有把会话时区固定为 `+00:00`。范围绑定和 `DATE(event_at)` 分组会受 MySQL session time zone 影响；实现历史接口前应在 SQLx `after_connect` 固定 UTC，并增加非 UTC 服务端/会话回归。
5. **partial 应表示“报告币金额不存在”，而不是“已知小计”。** 若某日任一有金额或 basis 的非稳定币缺价，该日 `amount/basis_amount/rate` 应为 `null`，从该日起累计值也应为 `null`；响应顶层为 `partial`，Mobile 不绘制整条曲线。无活动日则是 `complete` 的真实零值。

### 2. 已实现四类收益事实与当前 SQL

现有查询是单条 `UNION ALL`，每个分支先按资产聚合，外层再按资产合并；只包含可由终态业务表审计的已实现收益，未读取充值、提现、内部划转、现货成本或未实现盈亏（`src/modules/wallet/infrastructure.rs:1653-1742`）。当前应用层再按 USDT/USDC/USD 平价或 Redis 当前价统一换算，并在缺价时跳过该资产、返回已知小计和 `partial`（`src/modules/wallet/application.rs:150-240`）。

| 来源 | 事实表与终态时间 | 当前收益公式 | basis | 现有索引与风险 |
| --- | --- | --- | --- | --- |
| 秒合约 | `seconds_contract_orders.settled_at`；worker 结算时同时写 `status='settled'`、`result`、`settlement_price`、`settled_at`（`src/workers/seconds_contract_settlement.rs:378-401`） | win=`stake_amount * payout_rate`；loss=`-stake_amount`（`src/modules/wallet/infrastructure.rs:1667-1682`） | `stake_amount` | 仅有 `(user_id, created_at)`、`(product_id,status)` 及 worker 状态/到期索引（`migrations/0021_seconds_contracts.sql:21-45`、`migrations/0024_seconds_contract_entry_price.sql:1-3`、`migrations/0025_seconds_contract_settlement_retry_at.sql:1-3`）；没有 user/status/settled_at。未知 result 当前收益为 0 但仍计 basis，需把终态完整性作为数据约束或 partial 风险。 |
| 预测 | `prediction_orders.settled_at`；结算/退款路径均在同一事务写终态时间（`src/modules/prediction/infrastructure.rs:416-455`） | `payout_amount + refund_amount + fee_refund_amount - stake_amount - fee_amount`（`src/modules/wallet/infrastructure.rs:1686-1697`） | `stake_amount + fee_amount` | 仅有 `(user_id, created_at)`、market/status 及全局 `(created_at,id)`（`migrations/0075_prediction_markets.sql:116-148`、`migrations/0096_admin_time_ordered_list_indexes.sql:11-12`）；没有 user/status/settled_at。 |
| 杠杆 | `margin_positions.closed_at`；人工平仓写 closed，强平同时写 closed_at/liquidated_at（`src/modules/margin/infrastructure.rs:1528-1554`、`src/workers/margin_liquidation.rs:504-515`） | `COALESCE(realized_pnl,0) - interest_amount`（`src/modules/wallet/infrastructure.rs:1701-1710`）；`realized_pnl` 本身是价格 PnL，利息在返还权益时另扣（`src/modules/margin/application.rs:739-752`） | `margin_amount` | 仅有 `(user_id, created_at)`、产品/状态、清算/计息 worker 索引及 cross account 索引（`migrations/0022_margin_trading.sql:21-47`、`migrations/0026_margin_liquidation_fields.sql:1-8`、`migrations/0031_margin_borrow_interest.sql:5-11`）；没有 user/status/closed_at。`COALESCE(NULL,0)` 会掩盖终态缺少 PnL 的坏数据。 |
| 理财 | 实际入账事实为 `wallet_ledger.created_at` 的 `earn_redeem`；订阅另有 `redeemed_at`，二者在同一事务写入（`src/modules/earn/infrastructure.rs:674-716`） | 最早权威 redeem ledger `amount - subscription.amount`（`src/modules/wallet/infrastructure.rs:1714-1739`） | subscription principal | `wallet_ledger` 只有 `(user_id,asset_id,created_at)` 和 `(ref_type,ref_id)`（`migrations/0003_assets_wallet_ledger_locks.sql:25-42`）；未指定 asset 时前者不能直接按日期做连续范围。当前反连接通过更小 id 去重是正确语义，但会增加扫描/探测成本。`earn_subscriptions` 也只有 `(user_id,created_at)`，没有 user/status/redeemed_at（`migrations/0023_earn_products.sql:20-43`）。 |

当前集成测试已经覆盖四类公式、用户隔离、昨日排除、预测两种退款、人工/强平、重复理财流水、充值排除、真实零值和缺价 partial（`tests/wallet_routes.rs:441-839`）；但无 `DATABASE_URL` 时整个真实 SQL 分支直接返回，测试会显示通过但未执行数据库断言（`tests/wallet_routes.rs:443-445`）。

### 3. 时间边界与历史 SQL 形状

推荐在同一条查询中把四分支扩展为 `activity_day + asset_id/symbol + amount + basis_amount`，每个分支按 UTC 日和资产聚合，外层再次合并同日同资产；Rust 生成连续日期并补零，不在 SQL 中构造日历表。关键合同：

- 查询参数只接受 `days ∈ {1,7,30,180}`；`period_start_at` 为“当前 UTC 日零点减去 days-1 日”，`calculated_at` 在用例开始时只捕获一次。
- 所有分支使用半开区间 `event_at >= period_start_at AND event_at < calculated_at`。跨日分桶使用 UTC；不能依赖服务器默认时区。
- 结果按 `activity_day ASC, asset_symbol ASC`；Rust 恰好生成 `days` 个日点。
- 秒合约、预测、杠杆分别使用 `settled_at`、`settled_at`、`closed_at`。理财建议从 `earn_subscriptions` 以 `redeemed_at` 做范围驱动，再通过 `(ref_type,ref_id)` 取最早 `earn_redeem` ledger 的真实到账金额；这样事件日与业务终态一致，也避免先扫描用户全部钱包流水。
- 仍以业务事实表为来源，不从 wallet ledger 反推秒合约/预测/杠杆，避免不同 change type 或补偿流水重复计算。
- 连接池应在每个新连接执行 `SET time_zone = '+00:00'`。当前 `src/infra/mysql.rs:5-12` 只配置连接数、超时和 URL，没有会话初始化。
- 金额全部保持 `BigDecimal`；每个日点在完成全部资产换价后向零截断到 18 位。累计值应累加已经量化的每日值，summary 也取每日值之和，保证最后完整点严格等于 summary；现有今日接口的 18 位截断与 rate 规则见 `src/modules/wallet/application.rs:214-224` 和 `.trellis/spec/backend/wallet-amount-precision.md:97-100`。

建议的逻辑形状（不是可直接提交的最终 SQL）为：

```sql
SELECT activity_day, asset_id, asset_symbol,
       SUM(amount) AS amount,
       SUM(basis_amount) AS basis_amount
FROM (
  -- seconds: GROUP BY DATE(settled_at), stake_asset
  -- prediction: GROUP BY DATE(settled_at), asset_id
  -- margin: GROUP BY DATE(closed_at), margin_asset
  -- earn: GROUP BY DATE(redeemed_at), asset_id, joined earliest redeem ledger
) activity
GROUP BY activity_day, asset_id, asset_symbol
ORDER BY activity_day ASC, asset_symbol ASC;
```

`DATE(...)` 只放在 SELECT/GROUP BY；WHERE 必须保留原始时间列范围，不能写 `DATE(event_at) BETWEEN ...`，否则破坏时间索引的 sargability。

### 4. 行情持久化与历史估值能力

现有行情能力足以作为**价格快照的输入**，但不足以直接作为每次请求的最终审计值：

- 支持 `1m/5m/15m/1h/1d`（`src/modules/market/domain.rs:70-90`）；部署示例默认订阅全部五档（`docker-compose.example.yml:16-19`）。后台配置只要求周期列表非空，并未强制包含 `1d`（`src/modules/admin/service/market_feed.rs:36-49`）。
- 每个规范化 symbol 一个 Mongo collection，K 线唯一键为 `{interval:1, open_time:1}`（`src/infra/mongo.rs:15-36`）；读取按 interval 等值、open_time 范围、open_time 升序，索引形状匹配（`src/modules/market/infrastructure.rs:541-563`）。
- ingestion 同时写 Redis 最新 K 线和 Mongo，并对相同键执行 upsert（`src/modules/market/infrastructure.rs:746-763`）。Mongo 文档实际存 `source` 和 `updated_at`（`src/modules/market/infrastructure.rs:1996-2057`），但当前读模型只反序列化 OHLCV，不读来源和更新时间（`src/modules/market/repository.rs:26-37`）。
- K 线没有 `closed/final` 标记；当前日 `1d` candle 会继续被覆盖。公开 K 线查询还把 limit 限制为最多 100，不能直接复用来拿 180 根日线（`src/modules/market/domain.rs:116-130`）。内部历史估值应直接使用专用 repository 或分批，不走公开 100 条限制。
- K 线唯一键不包含 provider。外部 provider 变更会覆盖原文档；策略恢复任务也按相同键写价格字段，且恢复写不带 `source/updated_at`（`src/workers/kline_recovery.rs:336-354`），所以“请求时读取最新 Mongo close”并非不可变审计证据。
- AppState 中 Mongo 和 Redis 都是 optional（`src/state.rs:11-20`）；历史接口必须明确缺依赖时是服务错误还是价格 partial，不能悄悄改用当前余额/零价。

#### 推荐估值政策 v1

1. USDT、USDC、USD 延续既有 1:1 业务政策；这是一条显式、可版本化政策，不代表历史市场脱锚价。
2. 对已结束 UTC 日 D 的非稳定币 A，只接受 `AUSDT`、`interval=1d`、`open_time=D 00:00:00Z`、正十进制 close 且来源属于允许的外部 provider 的最终 candle。
3. 在 D+1 的固定宽限期后，将 price 和来源元数据冻结到 MySQL，不在 GET 请求中写快照。Mongo 晚到时 worker/backfill 可重试；一旦完成，不原位修订。
4. 当前 UTC 日使用现有 fresh Redis ticker 规则：symbol 必须匹配、价格正、`observed_at ∈ [calculated_at-60s, calculated_at]`（`src/modules/wallet/infrastructure.rs:1762-1810`）。当前日不提前冻结。
5. 过去日期缺快照时返回 `partial`；不使用当前 ticker，不使用相邻日，不使用未标明来源的 strategy recovery candle。
6. 上线历史接口前至少回填最近 180 个已结束 UTC 日；未回填资产按日明确 partial。

建议价格快照最小字段：

```text
valuation_day DATE
asset_id BIGINT UNSIGNED
reporting_asset VARCHAR(32)              -- USDT
policy_version SMALLINT UNSIGNED          -- 1
price DECIMAL(38,18)
source_symbol VARCHAR(32)
source_interval VARCHAR(8)                -- 1d
source_open_time TIMESTAMP(6)
source_observed_at TIMESTAMP(6)
source_provider VARCHAR(32)
source_document_id VARCHAR(24)
source_payload_hash CHAR(64)
created_at TIMESTAMP(6)
UNIQUE (valuation_day, asset_id, reporting_asset, policy_version)
```

若发生合法重估，应新增 policy/version 并保留旧行，不能覆盖 v1。GET 响应返回所用 policy version，才能复现历史结果。

### 5. 动态聚合与每日收益快照选择

#### 推荐首版：动态事实 + 价格快照

- **优点**：四类终态表仍是唯一收益事实；业务修正可立即反映；无需处理 snapshot worker 与结算事务的双写、迟到事件、水位回退和重算竞态。
- **成本**：每次最多扫描用户 180 日内匹配终态事件；加正确索引后，响应大小固定为最多 180 日点，聚合规模由该用户在周期内的结算量决定。
- **可审计性**：事实可回到业务行；换价可回到不可变 price snapshot 与来源 hash；API 暴露 policy version、计算截止和逐日缺价。

#### 不推荐首版直接落“最终用户日收益快照”

当前没有 return/portfolio/valuation snapshot 表或通用日结 worker（仓库全局搜索未找到）。新建最终收益快照会引入：UTC 日关闭、历史回填、业务行修订、快照幂等、当前日增量、跨库价格可用性、版本升级和审计水位等第二套一致性问题。首版在没有真实压测证据时收益不抵风险。

#### 何时升级为用户日事实快照

只有在新增索引后，180 日高频账户的 `EXPLAIN ANALYZE`/压测仍不满足既定 SLO，才考虑表：

```text
user_id, activity_day, source_kind, asset_id,
native_amount, native_basis_amount, event_count,
source_max_id/source_watermark, snapshot_version, generated_at
```

该表只缓存 native fact，不直接存不可解释的 USDT 最终值；换价仍引用版本化 price snapshot。当前日继续动态，已结束日才读快照。

### 6. 推荐接口与 DTO

接口：`GET /api/v1/wallet/return-history?days=1|7|30|180`

- 与 `GET /api/v1/wallet/today-return` 相同，必须 `UserAuth`，user_id 只从 token 提取；当前路由模式见 `src/modules/wallet/routes.rs:140-148`，全局 `/api/v1` nest 见 `src/lib.rs:18-24,55-62`。
- 非白名单值、重复/非法 query 或缺少 days 返回 400；建议客户端默认显式发送 1，不让服务端对非法值静默 clamp。
- 原始 JSON 保持 snake_case、十进制字符串和 Unix 毫秒，延续现有 TodayReturn DTO（`src/modules/wallet/presentation.rs:193-213`）。

```ts
type ReturnHistoryStatus = 'complete' | 'partial'
type MissingPriceReason =
  | 'missing_historical_close'
  | 'invalid_historical_close'
  | 'missing_current_ticker'
  | 'invalid_current_ticker'

interface ReturnHistoryResponseDto {
  scope: 'realized'
  reporting_asset: 'USDT'
  period_days: 1 | 7 | 30 | 180
  period_start_at: number       // UTC first day 00:00, Unix ms
  period_end_at: number         // captured calculated_at, upper bound
  calculated_at: number
  valuation_policy: {
    version: 1
    historical: 'utc_day_close_snapshot'
    current_day: 'fresh_current_ticker'
    stablecoin: 'usdt_usdc_usd_parity'
  }
  baseline: {
    at: number                  // period_start_at
    cumulative_amount: string  // exact "0.000000000000000000"
  }
  status: ReturnHistoryStatus
  summary: {
    amount: string | null
    basis_amount: string | null
    rate: string | null
  }
  missing_prices: Array<{
    day_start_at: number
    asset_symbol: string
    reason: MissingPriceReason
  }>
  points: Array<{
    day_start_at: number
    valued_at: number           // past day next UTC midnight; today calculated_at
    amount: string | null       // this day's realized return
    basis_amount: string | null
    rate: string | null
    cumulative_amount: string | null
    status: ReturnHistoryStatus
    missing_price_assets: string[]
  }>
}
```

DTO 不把零基线伪装成一条业务日记录：`baseline` 独立，`points.length === period_days`。1 日图由 baseline 和当天 point 组成；7/30/180 日同样从 baseline 开始绘制累计曲线。

### 7. complete / partial 精确定义

- **无活动日**：amount/basis/rate/cumulative 都是精确零或前一累计，`complete`，missing 为空。
- **需要价格的条件**：某资产当日聚合后 `amount != 0 OR basis_amount != 0` 且不是 USDT/USDC/USD；与当前实现一致（`src/modules/wallet/application.rs:158-167,194-208`）。
- **单日 partial**：任一需要价格的资产没有该日有效 price snapshot（当前日则无 fresh ticker）。该日 amount/basis/rate 全部为 null；missing 资产去空白、转大写、去重、升序。
- **累计传播**：第一个 partial 日之前的 cumulative 可保留用于审计；从该日开始 cumulative 全为 null，因为未知贡献会影响所有后续累计。
- **顶层 partial**：任一日 partial，则 response status=partial、summary 三字段为 null，并汇总 `day + asset + reason`。Mobile 对顶层 partial 不画任何金额曲线，不显示已知小计；隐私关闭时 missing 详情也不可见。现有 Mobile 规范已经要求 today-return 的 partial/错误/加载/访客/隐私态均不展示部分值（`.trellis/spec/mobile/backend-integration.md:615-655`）。
- **依赖错误与业务 partial 区分**：MySQL 不可用是 5xx；Redis 仅在当前日有非稳定币活动时缺失才产生该日 partial；Mongo 不应在 GET 热路径读取（历史只查 MySQL price snapshot）。price snapshot backfill/worker 故障表现为具体历史日 partial，并有独立运维告警。
- **不要沿用当前“partial 仍返回已知小计”的 API 形状**：现有 today-return 确实如此（`tests/wallet_routes.rs:825-839`），但历史累计一旦缺一日，小计极易被误画为完整趋势；新接口用 nullable 强制表达“报告金额不存在”。

### 8. SQL / 索引风险与候选索引

#### 现有风险

1. `created_at` 用户索引不能替代 `settled_at/closed_at/redeemed_at`。`migrations/0096_admin_time_ordered_list_indexes.sql:1-18` 新增的是后台 created_at 排序索引，对用户收益范围无帮助。
2. 四分支和外层 GROUP BY/UNION 可能创建临时表；这是最多 `days × asset` 的可控汇总，但基表扫描必须先由复合索引收窄。
3. `wallet_ledger (user_id,asset_id,created_at)` 在未指定 asset 时只能稳定利用 user 前缀；理财若继续从 ledger 驱动会扫描该用户大量无关充值、提现、交易和划转流水。
4. `NOT EXISTS earlier_ledger` 的正确性依赖“最小 id 是权威流水”；DB 没有针对 earn_redeem 的条件唯一约束。建议改为订阅驱动 + 显式最小 ledger id，并保留重复数据测试。
5. 所有终态时间列可空，表上没有“终态必须有终态时间/result/PnL”的 CHECK；坏行会被范围过滤静默丢失，或被 `COALESCE` 当零。迁移前先做数据审计，再决定 CHECK/修复。
6. 在 WHERE 对时间列包 `DATE()`、`CONVERT_TZ()` 会削弱索引；应固定 session UTC，并直接比较裸列。
7. MySQL TIMESTAMP 读写按 session time zone 与 UTC 转换；当前连接未固定时区，日边界结果会因部署环境变化。
8. 价格快照索引需要唯一键；否则同一日/资产/policy 多行会重复换价。更新旧 SQLx migration 被项目规范禁止，所有索引/表必须新增 migration（`.trellis/spec/backend/database-guidelines.md` 的 Immutable SQLx Migrations 场景）。

#### 候选最小索引

```sql
CREATE INDEX idx_seconds_return_user_status_settled
    ON seconds_contract_orders (user_id, status, settled_at);

CREATE INDEX idx_prediction_return_user_status_settled
    ON prediction_orders (user_id, status, settled_at);

CREATE INDEX idx_margin_return_user_status_closed
    ON margin_positions (user_id, status, closed_at);

CREATE INDEX idx_earn_return_user_status_redeemed
    ON earn_subscriptions (user_id, status, redeemed_at);
```

理财从订阅驱动时，现有 `wallet_ledger(ref_type,ref_id)` 可先用于逐订阅取最早 redeem ledger；若 `EXPLAIN ANALYZE` 显示回表/过滤过大，再评估更窄用途的 `(ref_type, ref_id, change_type, user_id, asset_id, id)`。不要未经计划验证就建立把多个金额列塞入索引的超宽 covering index；秒合约是高写入表，索引写放大会直接影响下单/结算。

索引列顺序遵循 equality (`user_id`,`status`) 后 range (`event_at`)；prediction 的两个终态可由 `status IN (...)` 形成多个范围。最终必须根据真实基数验证 optimizer 计划，候选名字和列并非迁移前的最终结论。

### 9. 1/7/30/180 日性能判断

当前环境未设置 `DATABASE_URL`/`MONGODB_URI`，无法取得表基数、执行计划或真实耗时；以下是基于查询和索引的复杂度判断，不是基准成绩。

| 周期 | 响应点 | 当前索引下 | 候选索引 + 动态事实下 | 额外风险 |
| --- | ---: | --- | --- | --- |
| 1 日 | 1 + baseline | 老账户仍可能按 user 扫描全部历史再过滤终态时间 | 四个短范围；通常最轻 | 当前非稳定币依赖 Redis 60 秒新鲜价 |
| 7 日 | 7 + baseline | 同上，日期选择性难以进入 access path | 扫描用户 7 日匹配终态事件 | 无活动日由 Rust 补零，不应产生 7 次 SQL |
| 30 日 | 30 + baseline | 高频秒合约/钱包流水开始明显放大 | 与周期内结算量线性 | 按资产批量取 price snapshot，不能按日 N+1 |
| 180 日 | 180 + baseline | 最高风险；四表用户历史扫描、临时聚合 | 与用户 180 日结算量线性；响应仍只有 180 点 | 公开 Kline limit=100 不可复用；必须提前回填 180 日 price snapshot |

验证要求：

- 对每个分支单独和完整 UNION 执行 `EXPLAIN ANALYZE`，覆盖空账户、普通账户和高频秒合约账户；记录 `actual rows`、loops、临时表和总时间。
- 断言 access path 使用新增索引，并且 base rows examined 与周期内匹配事件同阶，而不是与用户全历史或全表同阶。
- 构造至少 180 天、每天多资产和大量 seconds rows 的隔离数据；分别测 1/7/30/180，记录 p50/p95，不把 CI 墙钟断言当唯一性能门禁。
- MySQL 聚合一次返回 `day × asset`；price snapshot 一次按周期/资产集合读取；禁止 `days × sources` SQL 和逐日 Mongo 查询。
- 可加短 TTL 只读响应缓存，但 key 必须含精确 user/session、days、UTC day、policy version；当前日结算会变化，缓存不能成为事实源。

### 10. 测试矩阵

| 类别 | 必测场景 | 关键断言 |
| --- | --- | --- |
| 鉴权/参数 | 无 token、他人 token、days 缺失、0/2/181/字符串、1/7/30/180 | 401/400 正确；只使用 token user_id；白名单不 clamp |
| UTC 边界 | period start 恰好命中、前 1 微秒排除、calculated_at 后排除、跨月/闰日、MySQL session 非 UTC | 半开区间；同一事实始终落在 UTC 同一天；连接初始化后 `@@session.time_zone='+00:00'` |
| 秒合约 | win/loss、opened/canceled 排除、unknown/null result 终态坏数据 | 公式与 basis；坏终态不得伪装 complete |
| 预测 | win/loss、全额退款、只退本金、open 排除 | payout/refund/fee_refund 净额；basis 永远 stake+fee |
| 杠杆 | closed、liquidated、interest、canceled/open 排除、终态 realized_pnl null | `realized_pnl-interest`；closed_at 分桶；坏终态被发现 |
| 理财 | 到期/提前赎回、负收益/费用、重复 redeem ledger、redeemed_at 与 ledger created_at 边界 | 只取最早权威到账；事件日合同稳定；本金只扣一次 |
| 排除现金流 | deposit、withdraw、spot transfer、margin transfer、spot trade | 四类之外均不进入收益 |
| 日序列 | 空账户、周期内空洞、全零、正负交错、days=1 | 恰好 N 日升序；空洞 complete 零；baseline 独立；最终累计=summary |
| 精度 | 18 位、小数乘价超过 18 位、混合资产、负零、basis=0 | 向零截断；十进制字符串；无指数；负零归一；rate 规则一致 |
| 稳定币 | USDT/USDC/USD | 按 policy v1 1:1，不访问价格存储 |
| 历史价格 | 正常 1d snapshot、无 candle、非正/非法 close、时间不对齐、source 缺失、provider 切换、snapshot 后 Mongo 被覆盖 | 仅 exact UTC 日 close；快照不漂移；缺价 partial；strategy recovery 不冒充外部价格 |
| 当前日价格 | fresh/stale/future/mismatch/malformed/zero Redis ticker、Redis 缺失 | 沿用 60 秒规则；有需要时 partial，无活动时 complete zero |
| partial | 首日/中间/末日缺价、同日多资产缺价、多日同资产缺价 | 日金额 nullable；累计从首个 partial 起 nullable；顶层 summary null；missing 按 day+asset 排序去重 |
| 快照 worker/backfill | 重跑、并发、晚到 Kline、180 日回填中断、policy v2 | 唯一键幂等；旧 policy 不覆盖；失败可续跑并告警 |
| SQL 计划 | 1/7/30/180、高频用户、钱包流水远多于 earn | 使用候选复合索引；无按用户全历史扫描；无逐日 N+1 |
| 依赖故障 | MySQL down、Redis down、snapshot 缺口、Mongo/backfill down | MySQL 为 5xx；当前日/历史价格缺口按明确定义 partial；不回退模拟数据 |
| 序列化/合同 | 时间戳、状态、nullability、完整响应、不完整响应 | Unix ms safe integer；complete 无 missing/null；partial 的金额 null；字段顺序不作为合同 |

MySQL/Mongo 集成测试必须在真实隔离数据库执行；现有 `wallet_today_return...` 测试会在无 DATABASE_URL 时 skip，历史接口需要在 CI 中有一条明确不可 skip 的数据库 job。Mongo 快照测试还应证明原 K 线文档被二次 upsert 后，已冻结 price snapshot 与历史接口结果不变。

### 11. Files found

- `.trellis/tasks/08-09-mobile-home-return-history-chart/prd.md` — 首页真实收益历史目标、周期与缺价开放问题。
- `.trellis/tasks/08-09-mobile-home-today-return/research/today-return-contract.md` — 今日已实现收益口径的前置研究。
- `src/modules/wallet/infrastructure.rs` — 四分支收益 SQL、当前 ticker 读取与 freshness 校验。
- `src/modules/wallet/application.rs` — UTC 今日边界、稳定币政策、换价、精度和 partial 计算。
- `src/modules/wallet/presentation.rs` — 当前 TodayReturn 响应 DTO 与毫秒序列化。
- `src/modules/wallet/routes.rs` — UserAuth 路由与 token user_id 提取。
- `src/infra/mysql.rs` — SQLx MySQL pool；当前没有 UTC session 初始化。
- `src/modules/seconds_contract/infrastructure.rs`、`src/workers/seconds_contract_settlement.rs` — 秒合约终态写入和 settlement time。
- `src/modules/prediction/infrastructure.rs` — 预测结算/退款金额与 settled_at 写入。
- `src/modules/margin/application.rs`、`src/modules/margin/infrastructure.rs`、`src/workers/margin_liquidation.rs` — 杠杆 PnL/利息和人工/强平终态时间。
- `src/modules/earn/application.rs`、`src/modules/earn/infrastructure.rs` — 理财赎回到账、ledger 与 redeemed_at 同事务写入。
- `migrations/0003_assets_wallet_ledger_locks.sql` — wallet_ledger 时间字段和现有索引。
- `migrations/0021_seconds_contracts.sql`、`migrations/0024_seconds_contract_entry_price.sql`、`migrations/0025_seconds_contract_settlement_retry_at.sql` — 秒合约表与 worker 索引。
- `migrations/0022_margin_trading.sql`、`migrations/0026_margin_liquidation_fields.sql`、`migrations/0031_margin_borrow_interest.sql` — 杠杆表、closed/liquidated/PnL/interest 字段和索引。
- `migrations/0023_earn_products.sql` — 理财订阅时间字段与索引。
- `migrations/0075_prediction_markets.sql` — 预测订单终态金额、settled_at 与索引。
- `migrations/0090_admin_list_pagination_indexes.sql`、`migrations/0096_admin_time_ordered_list_indexes.sql` — created_at 后台索引；不能覆盖收益结算时间范围。
- `src/modules/market/domain.rs` — K 线支持周期与公开查询 100 条上限。
- `src/modules/market/infrastructure.rs` — Mongo K 线读取、Redis/Mongo ingestion、upsert 与来源元数据。
- `src/modules/market/repository.rs` — 当前 K 线读模型未包含 source/updated_at。
- `src/infra/mongo.rs` — symbol collection 和 `(interval,open_time)` 唯一索引。
- `src/workers/kline_recovery.rs` — strategy K 线恢复会写同一 Mongo K 线键。
- `src/state.rs`、`src/main.rs`、`src/config.rs` — MySQL/Mongo/Redis optional 依赖与行情/恢复 worker 启动条件。
- `tests/wallet_routes.rs` — 现有四类 SQL、隔离、退款、重复 ledger、零值与 partial 集成覆盖。
- `tests/unit_src/src_modules_wallet_application_tests.rs` — 稳定币、非稳定币、负值、零值、partial 与精确序列化单测。
- `tests/unit_src/src_modules_wallet_infrastructure_tests.rs` — ticker 新鲜度、symbol、正价和畸形 payload 单测。
- `tests/market_ingestion.rs`、`tests/market_adapters.rs`、`tests/kline_recovery.rs` — K 线持久化、来源字段与恢复 upsert 测试。
- `docker-compose.yml`、`docker-compose.example.yml` — MySQL 8.4、Mongo 7 与默认行情周期。

### 12. Code patterns

- `src/modules/wallet/infrastructure.rs:1662-1757` — 单条 UNION ALL 聚合四类终态收益，绑定同一用户与时间窗。
- `src/modules/wallet/application.rs:183-241` — 以 BigDecimal 汇总、缺价收集、18 位截断和 complete/partial 判定。
- `src/modules/wallet/infrastructure.rs:1791-1810` — 当前价必须匹配 symbol、正值、非未来且不超过 60 秒。
- `src/modules/market/infrastructure.rs:547-555` — Mongo `{interval, open_time range}` 查询和升序排序。
- `src/modules/market/infrastructure.rs:2038-2057` — Mongo K 线按 interval/open_time 原地覆盖，包括 source/updated_at。
- `src/infra/mongo.rs:29-37` — 与历史价格范围查询相容的唯一复合索引。
- `src/modules/market/domain.rs:116-130` — 通用 KlineQuery 把 limit clamp 到 100，180 日内部用途需要独立边界。
- `src/infra/mysql.rs:5-12` — pool 没有 `after_connect`，UTC 聚合需补连接合同。
- `tests/wallet_routes.rs:539-599` — 今日测试明确覆盖 UTC 日初前 1 微秒与其他用户隔离。
- `tests/wallet_routes.rs:801-839` — complete 精确金额与当前 partial 已知小计行为。

### 13. External references

- 项目运行版本：MySQL 8.4（`docker-compose.yml:2-11`）、MongoDB 7（`docker-compose.yml:18-24`）；Rust lock 为 sqlx 0.8.6、mongodb 3.7.0、chrono 0.4.44、bigdecimal 0.4.10（`Cargo.lock`）。
- [MySQL 8.4: How MySQL Uses Indexes](https://dev.mysql.com/doc/refman/8.4/en/mysql-indexes.html) — 复合索引使用左前缀，索引可服务 WHERE/range/group/sort；支持 equality 后 range 的候选顺序判断。
- [MySQL 8.4: Indexed Lookups from TIMESTAMP Columns](https://dev.mysql.com/doc/refman/8.4/en/timestamp-lookups.html) — TIMESTAMP 以 UTC 存储，但插入/读取会在 session time zone 与 UTC 间转换；UTC session 可消除该转换差异。
- [MySQL 8.4: Optimization and Indexes](https://dev.mysql.com/doc/refman/8.4/en/optimization-indexes.html) — 索引改善读性能但增加写入维护成本，支持避免无依据的超宽 covering index。
- [MongoDB: Compound Indexes](https://www.mongodb.com/docs/manual/core/indexes/index-types/index-compound/) — compound index prefix 与 ESR 原则，支持现有 interval equality + open_time range 读取。
- [SQLx 0.8.6 PoolOptions::after_connect](https://docs.rs/sqlx/0.8.6/sqlx/pool/struct.PoolOptions.html#method.after_connect) — 可在每次新连接后执行 session 初始化参数，适合固定 MySQL `time_zone`。

### 14. Related specs

- `.trellis/spec/backend/wallet-amount-precision.md:71-107` — UTC 已实现今日收益四类公式、basis、稳定币、ticker、partial、去重、精度与测试合同。
- `.trellis/spec/backend/database-guidelines.md` — SQLx migration 不可变；新表/索引必须新增 migration，并在真实 MySQL 验证。
- `.trellis/spec/backend/platform-display-and-chart.md` — 平台 K 线与图表数据源边界。
- `.trellis/spec/backend/earn-products.md` — 理财赎回费用快照和结算合同。
- `.trellis/spec/mobile/backend-integration.md:615-662` — Mobile today-return 严格适配、complete/partial、隐私和会话隔离边界。
- `.trellis/spec/guides/cross-layer-thinking-guide.md` — 数据源、存储、API、适配器和显示边界映射。

## Caveats / Not Found

- `python3 ./.trellis/scripts/task.py current --source` 返回 `Current task: (none)`；本次依据用户明确给出的任务目录写入，没有更改任务状态或其他文件。
- 当前环境 `DATABASE_URL`、`MONGODB_URI` 均未设置，未运行真实 `EXPLAIN ANALYZE`、表基数统计、MySQL/Mongo 集成查询或 1/7/30/180 基准；性能结论是静态审查，候选索引必须在隔离 MySQL 8.4 数据集验证。
- 仓库中未找到现成 return history、portfolio snapshot、daily return 或 valuation snapshot 表/接口/worker；也未找到历史价格的不可变 final 标记。
- Mongo K 线文档当前可变、source 不属于唯一键、读模型不含 source/updated_at，且 strategy recovery 可写同键；它可以作为快照输入，不能直接视为不可变审计账本。
- 后台行情配置未强制包含 `1d`。若产品决定采用日终估值，应在配置/健康检查中要求所有可能产生非稳定币收益的 `{ASSET}USDT` 订阅 `1d`，否则按日 partial。
- USDT/USDC/USD 1:1 是既有业务政策，不反映稳定币历史脱锚；若未来要求市场真实估值，应升 valuation policy version，而不是静默改变旧曲线。
