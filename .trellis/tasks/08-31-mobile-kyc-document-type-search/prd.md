# 手机端 KYC 证件类型支持搜索

## Goal

将手机端 KYC 表单中的原生证件类型下拉框替换为可搜索的底部选择弹层，让后台配置了较多证件类型时仍能快速定位，同时保持提交给后端的原始 `document_type` 值不变。

## Requirements

- 证件类型选项继续只来自当前认证国家规则的 `document_types`；规则未配置时沿用既有四种兜底类型。
- 点击证件类型字段打开 Teleport 到 `body` 的底部弹层，不使用原生 `select`。
- 搜索同时匹配当前语言下的证件类型显示名和后端原始类型值；匹配应忽略大小写、标点、连续空白、全角形式和可分解重音符号，并保留后台顺序。
- 只有明确点击某个搜索结果时才更新 `form.documentType`；打开、搜索、无结果、关闭和 Escape 均不得改变当前选择。
- 弹层打开后聚焦搜索框，锁定页面滚动，支持 Tab 焦点循环、Escape、遮罩和关闭按钮，并在关闭后把焦点还给证件类型触发器。
- 当前证件类型需要有明确选中态和 `aria-pressed`；无结果状态及所有辅助文案必须同时提供中文与英文。
- 国家变化后继续由既有规则保证当前证件类型有效；提交载荷仍发送后台原始 `document_type`。

## Acceptance Criteria

- [x] KYC 表单中不再使用证件类型原生 `select`，而是使用至少 44px 的对话框触发器。
- [x] 搜索中文显示名、英文/后台原始类型值均能得到正确结果，并保持配置顺序。
- [x] 选择结果后触发器更新，提交仍使用选项原始值。
- [x] 关闭、Escape、遮罩、无结果不会修改原选择，且焦点与 body 滚动正确恢复。
- [x] 深浅主题、受限设备及低动态模式复用现有 KYC 搜索弹层的可读样式与降级规则。
- [x] 相关单元/源码契约测试、Mobile 全量测试和类型检查通过。

## Definition of Done

- 实现、双语资源和回归测试完成。
- `npm --prefix mobile run type-check` 与 `npm --prefix mobile test` 通过。
- `git diff --check` 通过，并更新 `docs/superpowers/PROGRESS.md`。

## Technical Approach

- 复用 `useModalDialog` 和现有 KYC 国家搜索弹层的交互、焦点与主题结构。
- 在 `mobile/src/core/` 增加证件类型纯过滤函数，复用已有 Unicode 搜索归一化逻辑，避免把过滤规则留在视图中。
- 将国家与证件类型弹层的重复视觉类收敛为 KYC 通用搜索选择器样式，但保持两套独立状态，避免搜索词和选中值串扰。

## Decision (ADR-lite)

**Context**：现有国家字段已经形成完整的搜索弹层和无障碍合同，而证件类型仍是原生下拉框。

**Decision**：采用同页第二个独立搜索弹层，共用视觉类与 `useModalDialog`，证件类型过滤使用独立纯函数并复用国家搜索的 Unicode 归一化。

**Consequences**：交互保持一致且不增加第三方依赖；两个选择器状态相互隔离，后续若出现第三类同构选择器再评估抽取组件。

## Out of Scope

- 不修改后端 KYC 配置、审核、提交或数据库结构。
- 不新增或猜测后台未配置的证件类型。
- 不修改国家搜索的数据来源和选择规则。

## Technical Notes

- 相关实现：`mobile/src/views/KycView.vue`、`mobile/src/core/countrySearch.ts`、`mobile/src/core/modalDialog.ts`。
- 相关规范：`.trellis/spec/mobile/pwa-and-shell.md`、`.trellis/spec/mobile/navigation-and-localization.md`、`.trellis/spec/mobile/backend-integration.md`。
- 现有国家弹层已覆盖 Teleport、焦点、滚动锁、主题和受限设备模糊降级，本任务沿用同一合同。
