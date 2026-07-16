# 修复已执行迁移 0042 checksum 冲突

## 背景

执行 `sqlx migrate run` 时报错：

```text
error: migration 42 was previously applied but has been modified
```

原因是 `0042_country_locale_config.sql` 已经在数据库执行过，但后续需求把 `remark` 字段和本地国家名称改回了 0042 文件本身，导致 sqlx checksum 不一致。

## 范围

- 恢复 `0042` 为基础国家配置迁移，不包含后续 `remark` 字段。
- 修正 `0054`，避免在 `0055` 添加 `remark` 前写入 `remark` 列。
- 保留 `0055` 负责添加 `remark` 并回填本地国家名称和中文备注。
- 更新迁移测试以反映正确的迁移顺序。

## 验收

- 已执行过 0042 的数据库不再因为本地 0042 内容改动而被 sqlx 拦截。
- 从空库顺序执行 0042 → 0054 → 0055 时，国家配置最终仍有本地名称和中文备注。
