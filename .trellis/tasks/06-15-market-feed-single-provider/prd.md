# 行情订阅 providers 仅允许启用一个

## Goal

后台“行情订阅配置”的 providers 当前可以同时启用多个行情源，但实际产品要求同一时间只支持启用一个 provider。需要让后台页面交互和后端保存校验都遵守这个约束，避免配置保存后 worker 获得多个行情源。

## Requirements

- 后端保存行情订阅配置时，`providers` 必须非空且最多只能包含一个有效 provider。
- 重复提交同一个 provider 可以正常去重保存为单个 provider。
- 提交多个不同 provider 时返回参数错误，不保存配置。
- 后台“行情订阅配置”页面中，provider 行选择应表现为单选：
  - 启用某个未选中的 provider 时，替换当前 provider 列表为该 provider。
  - 禁用当前 provider 时，可以清空 provider 列表，提交保存时由后端继续拦截“不能为空”。
  - 其他订阅项（总开关、symbols、intervals）保持原逻辑。
- 现有运行态展示仍按数组展示，兼容历史数据读取。

## Acceptance Criteria

- [ ] 后端 `validate_providers` 拒绝多个不同 provider。
- [ ] 后台页面点击第二个行情源后，只保留被点击的 provider。
- [ ] 相关后端和后台页面测试覆盖单 provider 约束。
- [ ] 格式化、类型检查和目标测试通过。

## Definition of Done

- Tests added/updated for backend validation and admin UI behavior.
- Rust `cargo fmt --check` and `cargo check` pass.
- Web target test and typecheck pass.
- `docs/superpowers/PROGRESS.md` records the completed slice.

## Technical Approach

- 在 `src/modules/admin/market_feed_config.rs` 的 `validate_providers` 中先做 provider code 归一化和去重，再检查归一化后的 provider 数量是否超过 1。
- 在 `web/src/admin/actions/MarketFeedConfigPage.tsx` 中调整 provider 的 `toggleSubscription` 分支，使 provider 只做单选替换。
- 更新 `tests/admin_routes.rs` 和 `web/src/admin/actions/MarketFeedConfigPage.test.tsx` 中相关断言。

## Decision (ADR-lite)

**Context**: 现有数据结构和运行态仍以数组表示 providers，代码中多处消费 `Vec<String>` / `string[]`。  
**Decision**: 保持 API 字段为数组以减少迁移范围，但业务层约束数组最多一个有效值。  
**Consequences**: 未来如需恢复多 provider，可放宽校验和 UI 单选逻辑；当前不需要数据库迁移。

## Out of Scope

- 不修改 worker 的运行态数据结构。
- 不迁移或清理已有历史配置里的多 provider 数据。
- 不新增 provider 优先级、fallback 或多源轮询策略。

## Technical Notes

- 已检查 `src/modules/admin/market_feed_config.rs`：`save_config` 调用 `validate_providers` 后保存 `providers_json`。
- 已检查 `web/src/admin/actions/MarketFeedConfigPage.tsx`：provider 切换当前通过 `toggleListItem` 多选。
- 已检查 `web/src/admin/actions/MarketFeedConfigPage.test.tsx` 和 `tests/admin_routes.rs`：现有测试包含多 provider 归一化断言，需要更新。
