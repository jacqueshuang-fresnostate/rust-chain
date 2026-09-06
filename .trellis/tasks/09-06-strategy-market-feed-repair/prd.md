# 策略行情深度、成交与多周期 K 线修复

## Goal
让启用的策略行情在手机端提供一致的价格、模拟订单簿、最新成交和全部支持周期 K 线，手机端默认 1m。

## Requirements
- 后端生成并经已有 REST 缓存 / WebSocket 链路提供策略模拟深度及成交；保留 strategy 来源，不写真实订单、成交或资金流水。
- 修复 1m 形成与闭合，以及高周期实时聚合 / 推送；所有周期从权威 1m 聚合，不独立生成不同价格路径。
- 手机端初次进入行情图使用 1m；保留手动切换、重连与请求隔离。
- 不自动填补停机历史缺口；既有管理员手动历史恢复职责保持不变。

## Acceptance Criteria
- [x] 相同版本与时间输入生成稳定、有序、不交叉的模拟深度及可去重成交。
- [x] ticker、最新成交与形成中 1m 使用一致价格，重复轮次不重复成交。
- [x] 支持周期均有当前形成中 K 线，历史缺口不伪装成完整闭合蜡烛。
- [x] 1m 的 REST 与实时更新正确合并，时间戳和 OHLCV 连续语义有回归测试。
- [x] 手机端默认 1m 且切换周期仍正常。
- [x] 运行后端相关测试、架构检查及 mobile release:gate，明确外部依赖验证情况。

## Out of Scope
- 新币申购、派发、账本与真实撮合规则；交易所外部行情源；自动回补历史；部署生产。

## Plan
1. 追踪生成器→worker→缓存/存储→REST/WS→移动端图表，记录根因。
2. 先补行为回归，修复模拟深度/成交与多周期推送、默认周期。
3. 执行针对性和完整质量门禁，更新规范和 PROGRESS。


## User expansion (2026-09-06)
- 用户要求进一步从后端到后台排查并修复行情策略设计不合理与缺失项。
- 增补检查范围：过期策略启用、同交易对运行时段重叠的并发保护、真实运行错误与最近推送时间的后台可见性。
- 不扩展到无关后台业务或全站重构。

### Additional acceptance
- [x] 已结束策略不能直接启用；草稿仍可保存历史配置供预览/恢复。
- [x] 同交易对重叠的 active 策略启用/创建原子互斥，非重叠排期允许。
- [x] 后台区分待开始、待首笔、实时正常、推送延迟、已结束与运行异常，暴露实际错误和最近 tick 而非把启用等同健康。


## Verification
- Backend: 92 distinct focused cases across market lib, architecture/documentation,
  events WebSocket, market ingestion/cache/routes, deterministic generation/worker,
  and five real-MySQL admin strategy cases. Store-backed tests used isolated
  MySQL/Mongo/Redis, not environment-variable skips.
- Admin: 502 full-suite cases; 15 production-policy and 23 coverage cases;
  typecheck/lint/build/budget passed. Final focused runtime/actions 12/12.
- Mobile: release:gate passed with 676 behavior cases, types and both builds.
- Browser: real loopback API, strategy diagnostics, expired-enable disabled state,
  editable SideSheet, action hit targets at 1728/1280px; login, Dashboard, empty
  spot orders, assets, KYC and Security Policy read-only baselines.
- Existing historical gaps remain manual recovery; no production deployment.
- Task retained for the uncommitted release handoff, not auto-archived.
