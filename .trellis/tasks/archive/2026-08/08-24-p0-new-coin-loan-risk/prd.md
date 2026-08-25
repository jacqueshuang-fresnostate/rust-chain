# P0 新币与借贷风险闭环

## Goal

完成 P0-03、P0-04、P0-05：服务端权威控制新币定价和供给，解禁费真实动账，抵押借贷具备 LTV、价格时效、健康扫描、清算与坏账闭环。

## Scope and ownership

- `new_coin`、`loan` 业务模块及其 workers、必要的 admin/API DTO 与测试。
- 可新增迁移仅限 `0110`–`0113`，不得修改历史迁移。
- 不编辑全局 `config.rs/main.rs`，除非已有模块注册点确需一行接线；冲突时优先使用现有 worker 注册模式。
- 不编辑父任务、其他子任务和 `docs/superpowers/PROGRESS.md`。

## Requirements

### P0-03

- 项目配置服务端发行价、计价资产和可分配 supply；客户端 price/quote 只可作为显示或必须与权威值完全一致。
- 维护 reserved/allocated/remaining，项目行锁后原子校验并占用供给。
- 订单、扣款、分配、锁仓、供给占用同事务；失败完全回滚。
- 幂等键绑定规范化 request fingerprint；同键同参返回原结果、异参冲突。

### P0-04

- 解禁费金额和计价资产来自不可变应收快照。
- 同事务锁解禁记录和钱包，扣 available、写用户费用流水/平台收入腿、置 paid。
- 余额不足、并发重复或事务回滚不得释放锁仓；释放只接受真实 paid 记录。

### P0-05

- 借贷产品配置抵押资产白名单、initial/maintenance/liquidation LTV、oracle symbol/source/max age。
- 申请和审批均读取新鲜权威价重算估值，保存价格/时间/来源/风险快照；不满足初始 LTV 或价格失效时 fail closed。
- 实现健康度查询与周期扫描，跨清算阈值时幂等处置抵押、偿还本金/利息并记录坏账。
- 清算、还款、逾期扫描并发时只产生一个终态，钱包与 journal 可逐笔对账。

## Acceptance Criteria

- [x] 篡改新币 price、quote asset、amount/quantity 比例均在动账前失败。
- [x] 并发购买/认购总 allocated 不超过 supply；异参幂等冲突。
- [x] 解禁费余额不足零副作用；成功时钱包差额、应收快照与流水一致；并发只扣一次。
- [x] 低抵押、非白名单抵押、缺失或过期 oracle 均不能申请/批准。
- [x] 价格跨清算阈值只清算一次，坏账与抵押处置可对账。
- [x] 相关 Rust 测试、migration、fmt/clippy 通过。

## Technical Notes

- 迁移编号：0110–0113。
- 优先复用 Redis/Mongo 已存在的权威 ticker 读取器；必须校验 symbol/source/observed_at/max age。
- 若处置通道尚无真实外部卖出能力，采用明确的平台清算账户与坏账 journal，不允许静默丢弃差额。
