# 首页真实收益历史曲线

## Goal

将手机端首页 `portfolio-chart` 从本次页面会话内的总资产估值采样，改为由后端
真实收益历史驱动。首页 1/7/30/180 日周期必须展示当前用户按 UTC 日聚合的已实现
收益累计曲线，不得使用前端模拟点、随机点或刷新时余额波动替代历史收益。

## What I already know

- 当前 `portfolioSamples` 仅在页面运行期间监听总资产估值并保留最近 32 个样本。
- 周期标签 1/7/30/180 当前不可交互，且始终把 1 日标记为 active。
- 现有 `GET /wallet/today-return` 只返回 UTC 今日单点，无法支持历史折线。
- 今日收益业务口径已覆盖秒合约、预测、杠杆和理财，并排除充值、提现和内部划转。
- 后端和 Mobile 均已有严格金额、缺价、会话隔离及隐私展示边界。

## Decisions

- 新历史接口为 `GET /wallet/return-history?days=1|7|30|180`，沿用
  `realized` 口径、USDT 报告资产、UTC 自然日和严格白名单；非法或缺失周期返回 400。
- 收益事实继续从秒合约、预测、杠杆和理财终态表动态聚合，不复制第二份用户收益
  真相；新增结算时间复合索引保障 180 日范围查询。
- USDT/USDC/USD 按既有政策 1:1；已结束 UTC 日的非稳定币使用 Mongo 已持久化的
  对应 `AUSDT` 1d K 线收盘价，当前 UTC 日使用 60 秒内 Redis ticker。历史缺 K 线或
  当前行情无效时该日为 `partial`，不得使用当前价回填历史、邻日价或零价。
- 接口恰好返回 N 个日点并由后端补完整零活动日；金额保留 BigDecimal 18 位。
  任一点缺价时该点 amount/basis/rate 为 null，顶层 summary 为 null，Mobile 不绘制
  部分曲线。
- 图表绘制 `[0, ...daily cumulative_amount]`，因此 1 日有日初零基线与当前点；
  y 轴始终包含零，全零曲线位于中线。
- 周期切换只请求 1/7/30/180，复用精确 token + generation 生命周期；访客不请求，
  隐私关闭不渲染路径、端点或可访问数值描述。

## Requirements (evolving)

1. 新增受 `UserAuth` 保护的真实收益历史接口，周期只允许 1/7/30/180 日。
2. 返回 UTC 日序列、每日收益、累计收益、成本基础、状态和按日缺价信息；
   十进制使用精确字符串、时间使用 Unix 毫秒。
3. 缺失日补真实零值；缺价日与后续未知累计不得伪装完整，前端不得绘制部分金额。
4. 首页周期标签改为可访问按钮并驱动对应历史请求，默认 1 日。
5. `portfolio-chart` 只消费历史收益响应；删除本次会话总资产估值采样。
6. 隐私关闭、访客、加载、失败、partial 和 token 切换不泄露旧会话或部分收益。
7. 保持现有 SVG 视觉、Pencil 布局、主题和 320–448px 响应式行为。

## API Contract

```text
GET /api/v1/wallet/return-history?days=1|7|30|180

scope: realized
reporting_asset: USDT
period_days: 1 | 7 | 30 | 180
period_start_at / calculated_at: Unix ms
status: complete | partial
summary: { amount, basis_amount, rate } // partial 时均为 null
missing_prices: [{ day_start_at, asset_symbol }]
points[N]: {
  day_start_at, valued_at,
  amount, basis_amount, rate, cumulative_amount,
  status, missing_price_assets
}
```

- `points.length === period_days`，按日严格升序且相邻 86,400,000ms。
- complete 点的金额字段为十进制字符串；partial 点为 null。
- 第一个 partial 之前可保留已知累计；从第一个 partial 起累计为 null。
- 顶层 complete 时 summary 等于最后一点累计；partial 时 summary 全部为 null。
- 无活动周期返回 N 个 complete 零点，不返回空数组。

## Acceptance Criteria (evolving)

- [x] 源码中不再使用总资产估值 watcher 生成 `portfolioSamples`。
- [x] 1/7/30/180 日均可切换并请求严格白名单周期。
- [x] 图表点全部来自当前用户后端真实收益历史。
- [x] 1 日零基线、空活动、正负零收益、缺价、错误和会话竞态均有测试。
- [x] 过去非稳定币使用对应 UTC 日 1d close，当前日使用严格新鲜 ticker；缺价为 partial。
- [ ] Rust 与 Mobile 定向/全量质量门禁、PWA/Tauri 构建通过。

## Implementation Status

- Rust 已完成受保护路由、严格周期参数、四类 UTC 日聚合、Mongo 历史
  `1d` close、Redis 当前 ticker、partial/null 累计传播、18 位响应和 0101
  结算范围索引。
- Mobile 已完成严格 DTO/mapper、累计一致性、纯几何、真实接口、周期按钮、
  token/周期 ABA/卸载隔离、隐私状态、重试、无障碍摘要/表格和双语文案。
- 已通过 Rust wallet 35 项定向单测、Mobile 51 项聚焦测试与 Mobile
  type-check；MySQL 路由真实分支因未设置 `DATABASE_URL` 明确 skip。
- trellis-check 复核时修复 Mongo 历史 K 线损坏 BSON 会把单日缺价升级为
  5xx 的问题；读取改为容错 `Document`，字段类型错误按该日 `partial` 传播。
- 复核阶段已通过 Rust 格式检查、wallet 35 项定向单测、Mobile 全量
  332 项测试与 Mobile type-check；上述 Mongo 容错补丁按停止指令未再复跑。
- 根据用户停止指令，本轮未继续执行 Mobile 全量测试、PWA/Tauri 构建、
  最终 cargo check、task validate 或 `git diff --check`，因此最终质量门禁
  保持未勾选。

## Definition of Done

- 后端路由、应用、基础设施、响应 DTO 及测试完成。
- Mobile API/适配器/生命周期、首页图表与可访问周期控件完成。
- 规范、PRD、进度记录和差异检查完成。

## Out of Scope

- 未实现收益或现货持仓成本盈亏。
- 总资产历史快照、充值净值曲线或前端推测历史。
- 更换当前 SVG 图表为外部图表服务。

## Technical Notes

- 当前页面：`mobile/src/views/HomeView.vue`
- 今日收益后端：`src/modules/wallet/{routes,application,infrastructure,presentation}.rs`
- 今日收益移动端：`mobile/src/{api/wallet.ts,core/todayReturn.ts}`
- 研究结果写入 `research/` 后收敛最终接口和估值方案。
