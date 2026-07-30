# 全库修复二进制文本元数据

## Goal

一次性修复生产数据库中所有业务文本列被错误创建为 `VARBINARY`、`BINARY`
或二进制排序规则后，Rust `String` 解码失败的问题；当前首要复现路径是后台
KYC 审核读取 `kyc_configs.name`。修复必须覆盖现有全部业务表，并让后续新表
默认继承统一的非二进制 `utf8mb4` 文本元数据。

## What I Already Know

- KYC 配置查询将 `kyc_configs.name` 映射为 Rust `String`，生产环境当前返回
  `VARBINARY` 元数据并触发 SQLx 类型不兼容。
- 历史上 `prediction_settings`、用户/管理员/代理认证字段出现过同类问题，
  已有迁移 `0097` 和 `0098` 做过局部修复。
- 从完整迁移定义和本地规范数据库检查，业务模式中没有设计为
  `BINARY`/`VARBINARY` 的列；`BLOB` 等真正二进制类型不属于本次范围，回归测试会额外
  加入真实 `BLOB` 探针验证不被修改。
- 规范数据库当前包含 340 个 `VARCHAR`、31 个 `TEXT`、3 个
  `MEDIUMTEXT` 和 3 个 `CHAR` 文本列，合计 377 列。
- 业务文本列没有外键，也没有生成列，因此可以按表恢复完整列定义。

## Requirements

- 新增不可变的后续 SQLx 迁移，禁止修改任何已发布历史迁移。
- 迁移基于执行完 `0001` 至 `0098` 的全新 MySQL 8.4 规范数据库生成。
- 对 `_sqlx_migrations` 之外的全部业务 `CHAR`、`VARCHAR`、`TEXT`、
  `MEDIUMTEXT` 等文本列恢复准确的规范定义。
- 每列必须保留规范长度、可空性、默认值、注释和现有数据，并使用
  `utf8mb4_unicode_ci` 非二进制排序规则。
- 必须能修复两类漂移：真实 `VARBINARY`/`BINARY` 类型，以及使用
  `utf8mb4_bin` 等二进制排序规则的文本类型。
- 数据库和业务表默认字符集统一为 `utf8mb4`，默认排序规则统一为
  `utf8mb4_unicode_ci`，避免后续新表和新列继续继承错误配置。
- `BLOB` 及其他真正二进制数据类型保持不变。
- 迁移可在已经正确的数据库上执行，且 SQLx 只执行一次。
- 添加真实 MySQL 回归测试，直接执行迁移 SQL 并覆盖 KYC 生产查询路径。
- 部署说明明确该迁移会修改多张表，生产升级应预留维护窗口并先备份。

## Acceptance Criteria

- [x] 在修复前，真实 MySQL 测试可复现 `kyc_configs.name` 到 Rust
      `String` 的类型解码失败。
- [x] 执行新迁移后，KYC 配置/审核相关生产查询可正常读取 `name`。
- [x] 全部业务表不存在二进制排序规则的文本列。
- [x] 全部业务表不存在偏离规范的 `BINARY`/`VARBINARY` 列。
- [x] 数据库及全部业务表默认使用 `utf8mb4_unicode_ci`。
- [x] 修复前后的业务文本值保持一致。
- [x] 修复后列类型、长度、可空性、默认值和注释与规范数据库一致。
- [x] 认证和预测配置的既有元数据回归测试继续通过。
- [x] 全量迁移可在全新 MySQL 8.4 数据库中成功执行。
- [x] `cargo fmt -- --check`、`cargo check --all-targets`、相关真实
      MySQL 测试及 `git diff --check` 通过。

## Definition of Done

- 完成迁移、回归测试、数据库规范和部署说明更新。
- 由独立检查代理复核覆盖范围、数据保持、迁移安全性和测试结果。
- 更新 `docs/superpowers/PROGRESS.md`。
- 提交并推送到 GitHub `main` 分支。

## Out of Scope

- 不通过查询层 `CAST` 或将 Rust 字段改为 `Vec<u8>` 掩盖数据库元数据错误。
- 不转换 `BLOB`、文件、密钥密文等真实二进制数据。
- 不修改 KYC 或认证业务流程、权限规则和前端页面。
- 不自动替换无法按 UTF-8 解码的脏二进制字节；发现此类数据时迁移应失败，
  由运维备份并清理后重试。

## Technical Notes

- 迁移应显式恢复每个规范文本列，而不是仅执行
  `ALTER TABLE ... CONVERT TO CHARACTER SET`；后者不能将真实
  `VARBINARY` 恢复为 `VARCHAR`。
- `MODIFY COLUMN` 必须显式携带完整类型、`NULL`/`NOT NULL`、默认值和
  `COMMENT`，避免 MySQL 隐式丢失列元数据。
- 应排除 SQLx 自有的 `_sqlx_migrations` 表。
- 大量 `ALTER TABLE` 可能持有元数据锁或重建索引，生产环境需先备份并在
  维护窗口运行迁移。
