# 修复安全策略历史布尔 JSON 兼容

## Goal

修复 `/api/v1/user/2fa` 和 `/api/v1/user/third-party-bindings` 在读取 `security_policy_configs.policy_value` 时，因为历史 JSON 中存在字符串或数字布尔值（例如 `"0"`、`"1"`、`0`、`1`）导致 `UserSecurityPolicy` 反序列化失败的问题。

## Requirements

* `load_security_policy` 需要兼容历史布尔表示，不能因为 `"0"` 或 `0` 报 `expected a boolean`。
* 兼容范围限于安全策略 JSON 内的布尔字段，例如 `enabled`、`*_enabled`、`registration_invite_required`。
* 保持保存策略时仍写入标准 JSON boolean，不扩大到数据库迁移重写。
* `/api/v1/user/2fa` 和 `/api/v1/user/third-party-bindings` 读取策略时应返回正常响应。

## Acceptance Criteria

* [x] 历史字符串/数字布尔值可以被规范化为 boolean。
* [x] 安全策略默认值和现有强类型响应保持不变。
* [x] 覆盖单元测试，避免再次出现安全策略 JSON 布尔兼容回归。
* [x] 后端格式化和相关测试通过。

## Definition of Done

* 相关 Rust 测试通过。
* `docs/superpowers/PROGRESS.md` 记录本次交付。
