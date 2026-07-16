# 修复已执行迁移 0052 checksum 冲突

## 背景

执行 `sqlx migrate run` 时报错：

```text
error: migration 52 was previously applied but has been modified
```

`0052_schema_column_comments_zh.sql` 已经在数据库执行过，后续国家配置本地名称与中文备注需求又把 `country_configs.remark` 和本地语言字段描述补回了 0052，导致 sqlx 校验已执行迁移的 checksum 时失败。

## 范围

- 恢复 `0052` 为当时的字段中文注释迁移，不包含后续 `remark` 字段规则。
- 保留 `0055` 负责新增 `country_configs.remark` 并设置字段注释。
- 增加迁移测试，防止后续再次把 `remark` 写回已执行的 0052。

## 验收

- 已执行过 0052 的数据库不再因为本地 0052 内容改动而被 sqlx 拦截。
- 从 0052 继续向后迁移时，0055 仍能新增 `remark` 并保留中文字段注释。
