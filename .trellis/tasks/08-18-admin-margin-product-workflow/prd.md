# 杠杆产品后台业务流程优化

## Goal

把后台杠杆产品配置从“字段堆叠”优化为与真实开仓、风控和发布顺序一致的四步流程，并修复保证金模式被前端固定为逐仓的问题，使管理员可以安全配置逐仓、全仓、支持模式集合及默认模式。

## What I already know

- 用户明确要求保证金模式通过下拉选择，并继续按业务流程优化杠杆产品后台配置。
- 后端已经实现逐仓与全仓，产品可以同时支持多个模式。
- 后端把 `margin_modes` 第一项视为默认 `margin_mode`。
- 当前后台编辑会过滤掉全仓，提交又硬编码逐仓，存在静默覆盖配置的缺陷。
- 创建与编辑必须继续复用同一表单组件。

## Assumptions

- “保证金模式下拉”应覆盖后端已有的多模式能力，因此使用“支持模式多选下拉 + 默认模式单选下拉”，而不是把产品限制为单一模式。
- 本次优化后台产品配置，不扩展移动端/PC 端的杠杆下单交互，也不修改后端风险计算。
- 产品初始状态继续保持现有默认值“启用”，但发布确认页必须明确说明影响范围。

## Requirements

1. 基础配置
   - 杠杆交易对和保证金资产继续使用可搜索下拉。
   - 支持保证金模式使用多选下拉，选项中文显示“逐仓、全仓”。
   - 默认保证金模式使用单选下拉，只能选择已支持的模式。
   - 编辑时完整回填 `margin_mode` 和 `margin_modes`，不得丢失全仓配置。
2. 杠杆档位
   - 保留预设档位与自定义档位。
   - 自定义档位必须逐项为大于 1 的十进制数；非法值需要明确中文错误，不能静默忽略。
   - 最大杠杆由档位自动计算，并在页面中明确展示。
3. 风控与计费
   - 最小保证金必须为正数。
   - 最大保证金可空；有值时必须为正且不小于最小保证金。
   - 维持保证金率必须为非负十进制数。
   - 小时利率可空；有值时必须为非负十进制数。
   - 提示费率使用小数口径，例如 `0.05 = 5%`。
4. 发布确认
   - 汇总交易对、保证金资产、支持/默认模式、杠杆档位、风控参数和状态。
   - 明确说明启用会开放新开仓，变更只影响后续开仓，不改写既有仓位。
5. 流程交互
   - 使用“基础配置 -> 杠杆档位 -> 风控与计费 -> 发布确认”四步 Tab。
   - 提供上一步/下一步按钮；当前步骤不完整时下一步不可用并展示原因。
   - 只有发布确认步骤显示最终提交动作，完整表单合法时才允许提交。
6. API 映射
   - 请求同时发送 `margin_mode` 和 `margin_modes`。
   - `margin_modes` 第一项必须是管理员选择的默认模式，其余支持模式保持去重。
7. 列表可见性
   - 杠杆产品表格增加“默认保证金模式”列，中文显示逐仓/全仓。

## Acceptance Criteria

- [x] 创建表单可以通过下拉配置仅逐仓、仅全仓或同时支持两种模式。
- [x] 同时支持两种模式时，可以独立选择默认模式，请求首项与 `margin_mode` 一致。
- [x] 编辑含 `cross` 的记录后提交不会把配置降级为仅逐仓。
- [x] 移除当前默认模式时默认值自动切换；支持模式为空时流程不可发布。
- [x] 非法自定义杠杆档位、非正最小保证金、倒置的保证金上下限、非法费率都会阻止流程继续。
- [x] 发布确认页展示完整中文摘要和配置生效边界。
- [x] 创建与编辑的测试覆盖模式回填、默认模式排序、四步流程和请求体。
- [x] `npm --prefix web run typecheck`、`lint`、相关测试及生产构建通过。

## Definition of Done

- 测试已增加或更新，覆盖创建与编辑核心路径。
- TypeScript 类型检查、ESLint、Web 测试和生产构建通过。
- 管理后台规范补充杠杆产品配置契约。
- `docs/superpowers/PROGRESS.md` 记录本次交付与验证结果。

## Out of Scope

- 不修改数据库迁移、Rust 风控模型或 API 路由。
- 不修改移动端和 PC 端的杠杆下单页面。
- 不新增杠杆产品列表服务端筛选能力。
- 不改变产品启停对既有仓位的后端语义。

## Research References

- [`research/current-margin-workflow.md`](research/current-margin-workflow.md) — 现有前后端契约、缺陷、方案比较与边界。

## Technical Notes

- 主要实现：`web/src/admin/resources/actions/margin.tsx`。
- 列表配置：`web/src/admin/resources/resourceConfigs.tsx`。
- 交互测试：`web/src/admin/resources/resourceConfigs.test.tsx`。
- 复用 `AdminMultiSelect`、`AdminSelect`、`AdminTextInput` 和现有 SideSheet/FormModal。
- 后端契约来源：`src/modules/margin/application/product_config.rs`、`src/modules/margin/application/open_position.rs`、`src/modules/margin/presentation.rs`、`.trellis/spec/backend/margin-trading-actions.md`。
