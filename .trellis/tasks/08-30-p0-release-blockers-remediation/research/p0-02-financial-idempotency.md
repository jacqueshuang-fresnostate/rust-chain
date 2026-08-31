# P0-02 资金命令强幂等研究

## 现状与风险

### 管理员人工充值

- `AdminUserRechargeRequest` 没有幂等键。
- `recharge_admin_user_wallet` 每次请求生成新的 `Uuid::now_v7()`，随后在同一事务里增加钱包、写账本、写审计。
- 网络重试、反向代理重放或管理员重复点击会形成多笔合法但重复的入账。

### 现货下单

- `CreateSpotOrderRequest.idempotency_key` 是可选值，空值路径会直接继续创建订单。
- 数据库当前对 `spot_orders.idempotency_key` 建立全局唯一约束；查询也只按 key 查找，导致不同用户不能独立复用同一客户端键。
- 业务行已有下单参数，可用于冲突比较，但缺少明确、稳定的请求指纹列。

### 杠杆资金划转

- 请求键可选；缺失时服务端生成随机 UUID，因此客户端重试无法命中第一次命令。
- 表 `margin_transfer_requests` 已按 `(user_id, idempotency_key)` 唯一，并保存请求及结果快照，
  是可复用的命令收据基础。

## 统一契约

1. 三类命令都要求客户端提供非空、长度受限的 `idempotency_key`。
2. 幂等作用域：
   - 人工充值：`(admin_id, idempotency_key)`；
   - 现货下单：`(user_id, idempotency_key)`；
   - 杠杆划转：`(user_id, idempotency_key)`。
3. 使用 SHA-256 对规范化后的全部业务参数生成 `request_fingerprint`。金额使用
   `BigDecimal::normalized()`，文本字段按接口契约 trim 后参与哈希。
4. 同一作用域、同一键、同一指纹必须返回第一次的业务结果，不重复改余额、不重复写账本。
5. 同一作用域、同一键、不同指纹必须返回 409 Conflict。
6. 收据/订单、钱包变更、账本和审计必须在同一 MySQL 事务中提交。
7. 并发请求通过数据库唯一键裁决；竞争失败方回读已提交收据并执行“回放或冲突”分支。
8. 历史现货空键不伪造客户端身份：保留列可空以兼容历史数据，但新 API 强制提供，唯一索引改为用户范围。

## 数据设计

- 新建 `admin_wallet_recharges` 业务收据表，保存管理员、用户、资产、规范化金额、理由、键、指纹及首个响应快照。
- `spot_orders` 增加 `request_fingerprint`，移除全局 key 唯一索引，新增 `(user_id, idempotency_key)` 唯一索引。
- `margin_transfer_requests` 增加 `request_fingerprint`；既有请求参数快照继续保留，作为可读审计证据。
- 使用新的不可变迁移，禁止修改历史迁移。

## 客户端行为

- 确认操作创建时生成一次 key；同一次操作的加载失败、超时和显式重试必须复用该 key。
- 用户主动更改任一业务参数或完成一次成功命令后，才创建下一枚 key。
- PC 管理端人工充值必须提交 key；现货和杠杆各端适配器不得静默省略 key。

## 验证计划

- 三类命令各验证：缺键 4xx、同键同参回放、同键异参 409、不同用户复用同字符串互不冲突。
- 三类命令各发起至少 20 个并发同键请求，断言仅一笔余额变化、账本、审计/收据。
- 路由测试覆盖真实 MySQL 事务和唯一键竞争，不用纯内存模拟替代。
- Rust 运行 `cargo fmt --all -- --check`、`cargo clippy --all-targets --all-features -- -D warnings` 与相关集成测试。

## 所有权建议

- 实现代理独占幂等迁移、管理员充值、现货下单、杠杆划转及其直接客户端/测试文件。
- 不修改行情/秒合约链路、PC 构建配置、公共进度文件与 Trellis 任务文件。
