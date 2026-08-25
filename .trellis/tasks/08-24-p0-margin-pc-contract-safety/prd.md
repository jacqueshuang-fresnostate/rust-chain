# P0 杠杆风险与 PC 操作合同闭环

## Goal

完成 P0-09、P0-11：全仓杠杆资金转出不能绕过维持保证金，PC 的平仓方向、保证金模式、能力展示和批量结果必须与后端真实执行合同一致。

## Scope and ownership

- 后端 margin transfer/risk/locking/read model，以及 PC contract API/store/components/tests。
- 可新增迁移仅限 `0118`–`0119`，不得修改历史迁移。
- 不编辑 mobile、父任务、其他子任务和 `docs/superpowers/PROGRESS.md`。

## Requirements

### P0-09

- margin→spot 的 cross 转出在事务提交前按同一批新鲜标记价计算转后 equity、maintenance requirement 与安全缓冲。
- 锁 cross account、关联仓位、利息负债与钱包，校验 account version；与开/平仓、计息、强平并发时保持一致锁序。
- price unavailable/stale、账户 liquidating、风险计算失败均 fail closed。
- API 可返回权威最大可转额和拒绝原因，不能只依据 available。

### P0-11

- “平多”只关闭 long position，“平空”只关闭 short position，使用真实 position id/action。
- 在后端未支持部分/限价平仓前，PC 只提供市价全平，不发送被忽略参数。
- 下单保证金模式来自服务端 capability 与用户当前 setting 的交集，不固定 isolated。
- 批量操作强类型消费 succeeded/failures；任一失败必须展示部分失败和未完成仓位，不能显示纯成功。

## Acceptance Criteria

- [x] 最大可转额等于风险缓冲；多仓、对冲、利息、陈旧价格、liquidating 场景测试通过。
- [x] 转账与开/平仓/计息/强平并发不提交低于维持阈值状态。
- [x] long/short 双持仓 fixture 中按钮精确关闭目标仓位。
- [x] DOM/请求不再出现后端不支持的 partial/limit close 参数。
- [x] cross/isolated 请求与用户设置和 capability 一致。
- [x] 批量部分失败不会出现纯成功提示，并列出失败项。
- [x] 后端 Rust 与 PC lint/type-check/行为测试通过。

## Technical Notes

- 迁移编号：0118–0119；若无需 schema 变更则不强制新增。
- 风险公式必须与 liquidation worker/queries 共享实现，禁止复制出第三套公式。
- UI 优先收敛到后端当前可执行能力，而不是伪造未实现能力。
