# 杠杆持仓风险指标数据链路审计

## 现有链路

1. 后端 `GET /api/v1/margin/positions/{id}/risk` 返回 `MarginRiskSnapshotResponse.risk`。
2. `mobile/src/api/trading.ts` 的 `fetchMarginPositionRisk` 已正确适配：
   - `maintenance_margin_rate -> maintenanceMarginRate`
   - `estimated_liquidation_price -> estimatedLiquidationPrice`
   - `margin_ratio -> marginRatio`
3. `TradeView.vue` 每 5 秒按已成交仓位请求风险快照，并以 `position.id` 保存。
4. 持仓卡片当前把 `positionMarginRatio(position)` 放到“维持保证金率”字段；该函数返回的是 `marginRatio`，语义错误。
5. 预估强平价只读取风险快照。风险接口依赖 60 秒内的新鲜 Redis ticker，请求失败会被 `Promise.allSettled` 静默保留为缺失状态，因此卡片显示 `--`。

## 后端契约核对

- `src/modules/margin/application/queries.rs` 复用强平 worker 的风险状态，并调用 `margin_position_display_metrics`。
- `src/modules/margin/domain.rs` 的逐仓强平估算只依赖：方向、保证金、名义价值、利息、入场价与产品维持保证金率；标记价只用于计算距离率。
- 全仓仓位的 `estimated_liquidation_price` 和 `liquidation_distance_rate` 按契约返回 `null`，因为强平属于共享的 `(user_id, margin_asset)` 账户。
- `tests/margin_routes.rs` 已覆盖风险响应中的维持保证金率和逐仓预估强平价。

## 线上产品能力核对（2026-08-20）

匿名访问 `https://hipoex.cllbmz.kdns.fr/api/v1/margin/products` 返回 200；BTC-USDT 产品包含：

- `maintenance_margin_rate = "0.00800000"`
- `capabilities.position_risk = true`
- `margin_modes = ["isolated", "cross"]`

因此本次不是产品能力开关导致的隐藏，而是手机端展示口径错误和风险请求失败时缺少安全回退。

## 根因分类

- **主要根因：字段语义映射错误。** “维持保证金率”读取 `marginRatio`。
- **次要根因：显示可用性不足。** 静态可推导的逐仓强平价被绑定到需要新鲜行情的整包风险请求。
- **非根因：后端字段缺失。** 风险接口和产品接口均已返回所需数据。

## 修复边界

- 在移动端提取纯函数，集中校验和解析两个展示值。
- 服务端风险值始终优先；只在逐仓服务端值缺失时按后端公式回退。
- 全仓保持账户级语义，不复制逐仓公式。
- 不修改后端资金或强平逻辑。
