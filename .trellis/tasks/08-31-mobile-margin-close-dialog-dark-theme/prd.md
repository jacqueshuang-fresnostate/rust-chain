# 修复手机端杠杆平仓弹窗深色主题

## Goal

修复 body-Teleport 的 `margin-close-dialog` 在应用深色模式下仍显示浅色面板的问题，使其严格消费现有 Pencil 深色画板 `DGiNR` 的颜色角色，同时保持浅色 `ajSJF`、平仓比例、拖动确认、焦点和资金请求逻辑不变。

## What I already know

- `MarginCloseSheet.vue` 通过 `<Teleport to="body">` 渲染，不再位于 `.app-stage.theme-dark` 后代中。
- 应用主题权威由 `applyAppTheme()` 写入 `<html data-theme="light|dark">`。
- 组件已声明正确的深色色值，但当前 scoped selector `:global(html[data-theme='dark']) .margin-close-sheet` 经 Vue `compileStyle()` 后退化成裸 `html[data-theme='dark']`。
- 深色变量因此只声明在 `<html>`，随后被 `.margin-close-sheet` 本地声明的浅色变量覆盖，运行时面板保持白色。

## Requirements

- 将深色选择器写成 Vue scoped CSS 编译后仍保留 `html[data-theme='dark']` 与 `.margin-close-sheet` 后代关系的形式。
- 深色模式面板使用现有颜色：页面 `#0b0f0d`、字段 `#181e1a`、文字 `#f5f7f6`、分隔线 `#303a35`；浅色模式继续使用现有浅色色板。
- 不引入第二份主题状态，不在组件中复制或监听 theme store，不改变 Teleport、弹窗结构、平仓比例和拖动确认行为。
- 新增编译级回归测试，直接使用 `vue/compiler-sfc` 验证 scoped CSS 输出包含目标弹窗后代选择器，并拒绝仅把深色变量写到 `html` 的退化输出。
- 在真实页面中验证明暗主题切换后已打开的弹窗能即时更新，且 320–448px 无横向溢出。

## Acceptance Criteria

- [x] 当前旧选择器的编译回归先失败并能准确复现根因。
- [x] 编译结果包含 `html[data-theme='dark'] .margin-close-sheet`，深色变量不落在裸 `html[data-theme='dark']` 规则中。
- [x] 深色弹窗与字段的 computed background 分别为 `rgb(11, 15, 13)`、`rgb(24, 30, 26)`，主要文字为 `rgb(245, 247, 246)`。
- [x] 切回浅色后弹窗恢复白色/浅灰字段，主题切换不关闭弹窗、不重置比例。
- [x] 现有平仓弹窗测试、Mobile 全量测试、类型检查、PWA/Tauri 构建和 `git diff --check` 全部通过。
- [x] `docs/superpowers/PROGRESS.md` 记录本次修复与验证。

## Definition of Done

- 实现只修改平仓弹窗主题选择器、对应回归测试、必要规范/任务记录。
- 不修改后端、杠杆请求载荷、持仓和平仓业务规则。
- 完成源码、编译产物和真实浏览器三层验证。

## Technical Approach

1. 使用整个目标选择器的 `:global(...)` 写法，让 Vue scoped 编译器保留 HTML 主题根与 Teleported 弹窗之间的后代关系。
2. 在 `mobile/tests/margin-close-sheet.test.ts` 中通过 `compileStyle()` 检查最终选择器和深色变量归属。
3. 运行现有 Mobile 门禁，并用 Ego Browser 在深色、浅色和切换过程检查 computed style。

## Decision (ADR-lite)

**Context**：弹窗 Teleport 到 body 后不能依赖 `.app-stage.theme-dark`，而当前部分 `:global()` 写法被 scoped 编译器收缩为裸 HTML 规则。  
**Decision**：整条 `html[data-theme='dark'] .margin-close-sheet` 选择器进入 `:global(...)`，仍以 `<html data-theme>` 为单一主题权威。  
**Consequences**：改动最小、无需额外响应式状态；测试必须覆盖编译后的 CSS，而不只匹配源码。

## Out of Scope

- 不重设计平仓弹窗几何、按钮、比例滑杆或颜色。
- 不修改杠杆下单确认弹窗及其他 Teleported 弹层。
- 不修改后端接口、持仓计算或平仓幂等逻辑。

## Technical Notes

- 实现文件：`mobile/src/components/MarginCloseSheet.vue`。
- 回归文件：`mobile/tests/margin-close-sheet.test.ts`。
- 适用规范：`.trellis/spec/mobile/pwa-and-shell.md` 的 Margin Position Close Sheet Contract。
- 根因证据：[`research/scoped-teleport-dark-theme.md`](research/scoped-teleport-dark-theme.md)。
