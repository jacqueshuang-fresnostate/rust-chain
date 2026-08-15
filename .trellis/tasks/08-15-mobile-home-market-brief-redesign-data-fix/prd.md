# 首页行情简报视觉与数据修复

## Goal

重构手机端首页 `market-brief` 行情简报，使信息层级、视觉质感和移动端可读性与现有首页设计体系一致，同时修复行情数据选择、映射、刷新或展示口径错误，确保列表展示真实且可追溯的最新行情。

## What I already know

* 用户明确反馈当前 `market-brief` 显示效果不佳。
* 用户明确反馈当前 `market-brief` 数据不正确。
* 本任务只处理首页行情简报及其直接数据链路，不改动无关页面。

## Assumptions (temporary)

* 保留首页现有整体视觉语言、主题系统和路由行为，在组件内部做结构与样式升级。
* 行情数据应来自现有后端行情接口/实时状态，不使用硬编码占位价格。
* 需要兼容深色、浅色、窄屏和行情接口暂不可用的降级状态。

## Open Questions

* 已通过代码、接口响应和浏览器运行态完成定位，暂无阻塞问题。

## Requirements (evolving)

* 优化 `market-brief` 的标题、分组、行情行、涨跌状态和交互反馈。
* 修复交易对、最新价、涨跌幅等字段的数据来源与格式化。
* 保留实时刷新能力，避免重复订阅、陈旧快照覆盖新数据或错误回退。
* 对空数据、加载中和接口异常提供清晰且不跳动的状态。
* 补充针对数据映射和页面结构的自动化测试。

## Acceptance Criteria (evolving)

* [x] 首页行情简报在 320、360、390、448 像素宽度下无溢出或错位。
* [x] 行情简报使用后端交易对与实时 ticker，不再读取新闻或硬编码价格。
* [x] 最新价和 24h 涨跌幅使用后端同一 ticker 快照，正负色彩与格式正确。
* [x] 行情更新后简报及时刷新，旧 WebSocket/REST 快照不会覆盖较新数据。
* [x] 深浅主题、加载、空态和错误态显示正常。
* [x] 相关测试、类型检查与构建通过。

## Definition of Done (team quality bar)

* Tests added/updated (unit/integration where appropriate)
* Lint / typecheck / CI green
* Docs/notes updated if behavior changes
* Rollout/rollback considered if risky

## Out of Scope (explicit)

* 不重构首页其他业务模块。
* 不修改后台管理端或后端无关业务。
* 不替换当前整套行情供应商架构，除非定位到直接缺陷且修复属于最小必要范围。

## Technical Notes

* 原组件实际请求 `/news` 并把第一条新闻标题伪装成行情简报，已改为从共享 `MarketTicker[]` 计算市场广度、核心价格和领涨资产。
* 后端 ticker WebSocket 已发送 `high_24h`、`low_24h`、`volume_24h`、`price_change_percent_24h`，但手机端此前只消费 `last_price`，随后又以旧开盘价重算涨跌幅，造成最新价正确而百分比错误。
* 修复后完整动态字段按 `observed_at` 作为同一快照传播；只含最新价的兼容帧保留上一次权威涨跌幅，延迟 REST 只更新市场元数据。
* 本地浏览器已验证深浅主题、常见手机宽度和实时请求；手机端 367 项全量测试、类型检查、PWA/Tauri 双构建均已通过。
