# 后台模拟行情参数配置

## Goal

补齐基于 `market-data-emulator` 思路开发的新币模拟行情的后台可配置能力，使管理员能够查看、创建和修改生成参数，而不是只能依赖数据库默认值或代码固定值，并确保实时生成、手动补偿与既有行情发布链路使用同一份权威配置。

## What I already know

- 项目已实现 Rust 原生确定性 OHLCV 生成器、未来目标节点、实时 worker、手动缺口检测/预览/补偿和多周期聚合。
- 现有实现明确不在服务重启后自动补历史缺口，历史补偿由管理员手动发起。
- 本次问题聚焦“设置功能缺失”，需要先审计现有后端接口与后台表单，确认哪些生成参数已经持久化、哪些仍不可配置。
- 配置变更必须保留确定性、版本绑定、审计和正在运行任务的并发安全边界。
- 当前后台完整创建/编辑能力只挂在 `/admin/market/strategies/actions`，“行情策略”主页面仍是只读资源，造成管理员容易判断为没有设置入口。
- 当前 `strategy_type` 只是自由文本且生成器不读取；路径噪声系数 `0.55`、影线系数 `0.75` 和版本 seed 均不可设置。
- 当前没有场景预设、提交前 OHLCV 预览、版本历史或回滚入口。

## Confirmed Decisions

- 配置入口应合并到现有“行情策略”页面，而不是继续保留只读页和“策略动作”两个重复入口。
- 参数修改只影响新激活版本及其后生成的 K 线，不回写已经发布的历史数据。
- 后台表单需要使用中文字段名、范围提示与服务端校验错误。
- 按“单交易对 OHLCV 设置中心”范围实施，不加入多交易对相关矩阵、funding、L2 和离线导出。
- 自动 seed 在创建时生成；编辑时默认继承当前激活版本的 seed，只有管理员明确选择“重新生成”才更换。固定 seed 必须由管理员填写。
- 回滚不直接重新激活旧行，而是复制旧快照与 seed，创建一个版本号递增的新版本，保留完整审计链。

## Requirements (evolving)

- 管理员能够读取并设置模拟行情生成参数。
- 提供 `custom_path`、趋势、区间、高波动、崩盘恢复和拉升回落等后端权威场景预设。
- 提供 seed 模式、均值回归强度、噪声强度、影线强度和成交量形态等高级参数。
- 场景代码固定为 `custom_path`、`trend_up`、`trend_down`、`range`、`high_volatility`、`crash_recovery`、`pump_then_dump`；场景预设只负责向表单填充显式参数与节点，不在生成器中藏入不可见规则。
- seed 模式固定为 `auto`、`fixed`；固定 seed 长度为 1～128 个字符。
- 均值回归强度范围为 0～2，噪声强度和影线强度范围为 0～5；成交量形态固定为 `uniform`、`trend`、`bell`、`end_spike`。
- 后端执行严格范围、枚举、时间顺序和跨字段校验。
- 实时生成和手动补偿读取同一激活版本，不产生两套口径。
- 配置变更留下管理员审计记录，并具备可追踪版本。
- 提交前可生成无副作用 OHLCV 预览；可查看版本历史，并通过“复制旧版本为新版本”的方式回滚。
- 旧版本快照缺少新字段时，使用与当前固定常量一致的兼容默认值，确保同一 seed、版本、交易对与时间槽继续生成相同结果。
- 预览最多返回 240 根采样蜡烛，返回实际 preview seed；固定 seed 可精确重放，自动 seed 使用后端生成并在响应中显式返回。
- 创建策略时，交易对必须从启用且属于 `internal`/`strategy` 市场类型的后台交易对目录中下拉选择，选项同时显示交易对符号与 ID；策略类型必须从受支持类型下拉选择，不再允许自由文本产生未实现类型。

## Acceptance Criteria (evolving)

- [x] 后台能查看当前模拟行情配置及其激活版本。
- [x] 后台能创建或修改完整生成参数，并收到中文可理解的校验结果。
- [x] 场景预设和高级参数会进入不可变版本快照，旧版本缺少新字段时保持原有字节级输出。
- [x] 预览不写 MySQL/Mongo/Redis/WebSocket/检查点，且相同配置与 seed 的预览结果可重放。
- [x] 版本历史可查看；回滚生成递增的新版本且不能绕过 active 状态冲突和审计原因。
- [x] 实时 worker 和手动补偿使用同一权威配置快照。
- [x] 修改配置不会静默改写已生成历史 K 线。
- [x] 后端路由、持久化、后台交互和并发边界有自动化测试覆盖；若复用既有不可变 JSON 版本快照而无需迁移，测试和任务记录必须明确说明。
- [x] 创建策略的交易对 ID 与策略类型均为下拉选择，提交值仍保持既有 `pair_id` 和 `strategy_type` API 合同。

## Definition of Done

- 后端持久化、领域校验、管理接口及后台配置界面完整。
- 聚焦测试、后端检查、后台测试/typecheck/lint/build 通过。
- Trellis 规范与 `docs/superpowers/PROGRESS.md` 同步。

## Out of Scope

- 不引入 Python 运行时或直接嵌入第三方仓库。
- 不改变公开行情 WebSocket/REST 响应格式。
- 不把历史缺口恢复改为自动补偿。
- 不修改真实外部行情 provider 的配置逻辑。
- 不实现多交易对相关性矩阵、资金费率、L2 订单簿模拟、离线 CSV/JSON 导出或 Python 运行时。

## Technical Notes

- 参考项目：`elriseio/market-data-emulator`。
- 既有实现主要位于 `src/modules/market/`、`src/workers/synthetic_market.rs`、后台市场恢复相关模块与 `migrations/0102_synthetic_market_and_manual_kline_recovery.sql`。
- 待审计结果将写入 `research/` 并据此收敛最终参数合同。
- 审计与方案：[`research/current-gap-and-reference.md`](research/current-gap-and-reference.md)。

## Research References

- [market-data-emulator](https://github.com/elriseio/market-data-emulator) — OU、确定性 seed、场景与 1m 权威生成。
- [Scenarios Guide](https://github.com/elriseio/market-data-emulator/blob/main/docs/scenarios.md) — 场景由 regime、drift、volatility、noise、extremes 与 volume 参数组成。

## Feasible Approaches

### A. 只合并现有后台入口

改动最小，但固定生成参数、seed、场景、预览和版本仍不可设置，不能完整解决问题。

### B. 单交易对 OHLCV 设置中心（推荐）

扩展现有版本快照和生成器，增加场景、高级参数、预览、版本历史/回滚，并把完整操作合并到“行情策略”主页面；保持实时与补偿共用同一解析器。

### C. 完整移植参考项目配置面

进一步加入多交易对相关矩阵、funding、L2、离线导出和批量计划；这会与本项目既有撮合和资金费率边界冲突，超出当前设置缺口。

## Decision

采用方案 B。继续复用 `strategy_versions.config_json` 与独立 `seed` 列作为不可变权威快照，不修改已经执行过的迁移；后端增加统一快照解析器、预设目录、无副作用预览、版本查询与复制回滚接口，后台将所有能力合并到“行情策略”页面。
