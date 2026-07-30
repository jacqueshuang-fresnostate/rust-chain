# 修复 VARBINARY 字符串解码错误

## Goal

修复已部署 MySQL 中 `prediction_settings.default_settlement_mode` 为
`VARBINARY` 或使用二进制字符串元数据时，SQLx 无法把字段解码为 Rust
`String`，导致预测市场同步与配置接口持续报错的问题。

## What I Already Know

- 运行日志曾明确指出 `default_settlement_mode` 的 `String/VARCHAR` 与
  `VARBINARY` 不兼容。
- Rust 持久化模型正确使用 `String`，公开 API 也应继续返回文本。
- 原始 `0075_prediction_markets.sql` 从首次提交起就声明该字段为
  `VARCHAR(32)`，不能修改已应用迁移。
- 修复需要兼容历史结构漂移，同时保持新数据库迁移结果不变。

## Requirements

- 新增后续 SQLx 迁移，显式把预测设置中的文本状态字段规范化为
  `utf8mb4` 非二进制排序规则。
- 必须保留已有配置值、NULL 语义、长度、默认值和 NOT NULL 约束。
- 不改变预测市场 API、Rust DTO 或业务规则。
- 增加真实 MySQL 回归测试，覆盖从 `VARBINARY` 漂移结构修复到
  `String` 正常解码。
- 更新数据库规范和项目进度记录。

## Acceptance Criteria

- [x] 迁移后 `default_settlement_mode` 可被 SQLx 解码为 `String`。
- [x] `default_invalid_refund_policy`、`last_sync_status` 和
  `last_sync_error` 同步使用明确的非二进制文本元数据。
- [x] 漂移结构中的原值在迁移后完整保留。
- [x] 迁移对已经正确的 `VARCHAR` 新库也能成功执行。
- [x] 聚焦 MySQL 测试、格式化、编译和迁移校验通过。

## Definition of Done

- 测试与质量检查通过。
- 不修改历史迁移校验和。
- 规范、进度和 Trellis 任务记录更新。
- 变更提交到 Git。

## Out of Scope

- 不重构预测市场业务逻辑。
- 不修改其他业务表的字符集。
- 不改变预测市场外部接口。

## Technical Notes

- 受影响读取位于 `src/modules/prediction/infrastructure.rs` 的
  `load_settings` 与 `load_settings_in_tx`。
- 数据模型位于 `src/modules/prediction/repository.rs` 的
  `PredictionSettingsRow`。
- 使用追加迁移修复结构，遵守 SQLx 已应用迁移不可变约束。
