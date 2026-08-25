# P0 事件时点与行情权威性闭环

## Goal

完成 P0-06、P0-07、P0-08、P0-10：秒合约按事件时点可重放结算，预测市场本地关盘，闪兑使用事务内权威报价，行情重载不遗留旧 provider 写入者。

## Scope and ownership

- seconds_contract、prediction、convert、market_feed 及必要的 worker 配置/注册与测试。
- 可新增迁移仅限 `0114`–`0117`，不得修改历史迁移。
- 本工作流唯一拥有全局行情 worker 的 `config.rs/main.rs` 必要改动。
- 不编辑父任务、其他子任务和 `docs/superpowers/PROGRESS.md`。

## Requirements

### P0-06

- 结算价按 `expires_at` 的明确选择规则从不可变历史 tick/candle 获取，并保存 source、observed_at、generation/version。
- 准时、延迟和重放必须复用同一 settlement snapshot；窗口外或缺失价格保持 pending，不得读取处理时最新价兜底。

### P0-07

- 报价和下单消费事务都使用数据库时间检查 `< end_at`、`last_synced_at` 最大时效、market version/status。
- 独立本地关盘任务按数据库时间关闭到期市场；与下单竞态通过行锁/版本保证单一终态。

### P0-08

- MySQL quote 保存 owner、fingerprint、行情 source/time/version、expires_at、consumed_at，并作为唯一权威事实。
- 确认事务 `FOR UPDATE` 检查 owner、指纹、过期和消费状态；Redis 仅缓存而非权威。
- 双钱包按稳定 `(user_id, asset_id)` 顺序锁定，双向并发无环。

### P0-10

- provider 父子任务共享 CancellationToken，使用 JoinSet/TaskTracker 管理生命周期。
- reload/disable 先取消再等待全部旧代际子任务退出；旧 generation 写 Redis/Mongo/event 前必须被 fence。
- panic/异常可观测并按 supervisor 策略重建或使 readiness 失败。

## Acceptance Criteria

- [x] 秒合约延迟 5 分钟与准时执行结果一致，窗口外 ticker 不结算，重放复用快照。
- [x] prediction 在 `now == end_at`、同步过期、关盘竞态下拒绝新订单且零半提交。
- [x] convert 缺失/错误/非正/未来/陈旧 ticker、过期 quote、异参重放均零动账；一次消费。
- [x] 双向闪兑并发不死锁、不重复动账。
- [x] 连续 N 次 market reload 后每 provider 只有一个循环；disable 后旧任务归零；旧 generation 写入被拒。
- [x] 相关 Rust 测试、migration、fmt/clippy 通过。

## Technical Notes

- 迁移编号：0114–0117。
- 时间判断使用数据库 UTC 时间与持久化 observed_at，避免进程时钟与处理延迟改变资金结果。
- 结构化并发测试必须使用可控 provider/fake writer，不依赖公网行情。
