# 根因记录

- 后端 `load_pair_rule` 同时匹配正向和反向资产，并在反向请求时使用 `target_min_amount/target_max_amount`、固定汇率倒数及真实请求方向。
- 列表接口只返回 `convert_pairs` 配置行的原始方向，同时已经返回目标侧限额。
- 手机端适配器此前丢弃 `target_min_amount/target_max_amount`，页面 `swapDirection()` 只在数组里寻找显式反向行。
- 当生产配置只有一条方向时，`resolveReverseSwapPair()` 返回 `undefined`，按钮没有反馈也没有状态变化。
- 即使简单生成一条反向对象，现有 `pairId` 也无法区分共享同一后端配置 ID 的两个方向，因此选择键必须包含配置 ID 和 from/to 资产 ID。
