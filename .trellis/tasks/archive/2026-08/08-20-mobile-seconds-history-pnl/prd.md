# 手机端秒合约历史订单盈亏金额

## Goal

在手机端 `/seconds/history` 的每条历史订单中增加清晰的盈利或亏损金额，让用户能直接区分赢单净收益、输单本金损失，以及没有确定盈亏的取消/未知状态。

## Requirements

- 仅使用订单接口已返回的投注本金、下单赔率、结算结果和结算资产，不依赖实时行情或当前产品配置。
- `result = win` 时显示“盈利金额”，金额为 `stakeAmount × payoutRate`，以 `+` 前缀和正向语义色展示。
- `result = loss` 时显示“亏损金额”，金额为 `-stakeAmount`，以负向语义色展示。
- 取消、缺少结果或未知结果只显示通用“盈亏金额”和 `--`，不得推测为零、盈利或亏损。
- 盈亏金额以订单的 `stakeAssetSymbol` 作为结算资产单位；不能把含本金的总派彩金额误当成盈利。
- 盈亏信息在历史订单卡片中拥有独立、醒目的完整宽度区域，并保持现有价格、方向、期限和创建时间信息。
- 新增文案必须同时提供简体中文和英文，页面模板不得写死中文。
- 保持 320–448px 无横向溢出、明暗主题语义色和现有历史请求生命周期不变。

## Acceptance Criteria

- [ ] 100 USDT、赔率 0.8 的赢单显示“盈利金额 +80 USDT”。
- [ ] 100 USDT 的输单显示“亏损金额 -100 USDT”。
- [ ] 已取消、无结果和未知结果订单显示“盈亏金额 --”。
- [ ] 盈利与亏损分别使用现有 `--positive`、`--negative` 语义色，未知状态保持中性色。
- [ ] 历史页仍只展示非活动订单，缺失结算价仍显示 `--` 且不使用实时行情替代。
- [ ] 聚焦测试、Mobile 类型检查、全量测试及 PWA/Tauri 构建通过。

## Definition of Done

- 核心盈亏展示模型有可执行单元测试。
- 历史页面、双语资源和视觉合同测试已更新。
- 相关 Mobile 规范和进度记录已同步。
- 现有未提交改动和 `mobile/pencil/docs/` 不被覆盖或误纳入。

## Technical Approach

在 `mobile/src/core/secondsOrder.ts` 增加共享的历史盈亏展示模型，复用既有 `secondsOrderEstimatedProfit()` 作为赢单净收益口径；页面只负责格式化带符号金额和渲染语义样式。该任务不扩展后端接口，因为订单快照已提供计算所需的全部权威字段，且用户要求限定在手机端历史订单界面。

## Decision (ADR-lite)

**Context**：历史列表接口没有单独的盈亏字段，但订单已经固化投注本金、赔率和最终结果；当前后端赢单的 `payout_rate` 定义为净收益率，输单本金已在开仓时扣除。

**Decision**：在移动端共享订单模型中生成只读展示值；赢单展示净收益，输单展示负本金，未知结果保持不可用。

**Consequences**：无需修改数据库、结算事务或 API 合同；显示口径与现有预计收益一致。该值只用于界面，不参与钱包入账、下单或后续请求。

## Out of Scope

- 不修改秒合约结算、钱包入账、订单存储或后台页面。
- 不新增总派彩金额、累计盈亏统计或筛选功能。
- 不调整秒合约主交易页和历史页路由。

## Technical Notes

- `SecondsOrder` 已包含 `stakeAmount`、`payoutRate`、`result` 和 `stakeAssetSymbol`。
- 后端 `payout_rate` 是净收益率；赢单总入账为本金加净收益，历史页应只显示净收益。
- 现有 `secondsOrderStatusPresentation()` 已对结果执行去空白和大小写归一化，可保持相同边界行为。
- 相关代码与测试：`mobile/src/core/secondsOrder.ts`、`mobile/src/views/SecondsHistoryView.vue`、`mobile/tests/seconds-{api-adapter,history-view}.test.ts`。
