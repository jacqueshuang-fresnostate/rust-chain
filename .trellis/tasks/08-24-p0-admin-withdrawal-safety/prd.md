# P0 管理员与提现安全闭环

## Goal

完成 P0-01、P0-02、P0-12：阻断生产默认管理员接管，修正提现广播歧义状态导致的双付风险，并让 mobile 的费用授权与后端真实冻结额完全一致。

## Scope and ownership

- 后端 bootstrap/migrator、管理员首次改密闸门、钱包提现 application/infrastructure/worker/routes/DTO。
- mobile 提现 API 映射、报价展示与提交绑定；只在必要范围内修改现有未提交文件。
- 可新增迁移仅限 `0107`–`0109`，不得修改历史迁移。
- 不编辑父任务、其他子任务和 `docs/superpowers/PROGRESS.md`。

## Requirements

### P0-01

- production 默认不创建管理员；只有显式 bootstrap mode 才创建。
- 缺失/空值/`Qaz123456@` 等已知默认口令必须启动失败；Compose/1Panel 示例不得提供生产固定口令回退。
- bootstrap 管理员需记录强制改密状态；首次登录后除查询自身与改密外的受保护管理操作均拒绝。
- 改密原子清除强制状态并撤销既有会话；重复 migrator 不重复创建账号。

### P0-02

- 广播错误分类为 deterministic rejected、unknown、retryable-before-acceptance。
- timeout、连接中断、5xx、无效响应等 unknown 在重试耗尽后保持全部预留资金 frozen，转 `unknown_broadcast`/`manual_review`。
- 使用稳定 `gateway_request_id` 查询远端状态；查询到 tx 后只核销一次；权威未受理才能只解冻一次。
- 重启、重复回执、人工复核均幂等并保留审计信息。

### P0-12

- 服务端生成并持久化提现 quote：标准化 amount、tiered fee、net、total_reserved、network、expiry、fee config version、owner、fingerprint、consumed。
- 创建提现事务锁定并消费 quote；过期、配置变化、异参重放均零动账。
- mobile 完整映射阶梯费和 quote，确认页展示必须与创建响应和冻结额一致。

## Acceptance Criteria

- [x] production 三类无效 bootstrap 配置均测试为失败，源码/Compose 无固定生产口令。
- [x] 临时管理员未改密前无法执行其他后台写操作，改密后恢复且旧 session 失效。
- [x] 远端已受理但客户端持续超时超过预算后资金仍冻结；查询确认只核销一次。
- [x] 确定拒绝只释放一次，unknown 绝不自动释放。
- [x] 固定费、阶梯边界、开放尾档、配置变化和 quote 过期均有测试。
- [x] mobile 展示 fee/net/total_reserved 与服务端 quote、提现创建响应一致。
- [x] 相关 Rust/mobile 测试、fmt/clippy/type-check 通过。

## Technical Notes

- 迁移编号：0107–0109。
- 所有金额使用 Decimal，报价与提现状态写入同一 MySQL 事务。
- 保留现有 API 兼容字段，但资金提交必须使用服务端 quote。
