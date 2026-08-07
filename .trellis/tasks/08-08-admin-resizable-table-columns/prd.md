# 后台所有表格支持拖动列宽

## Goal

让后台和代理后台的所有具名数据列都能从表头右边缘拖动调整宽度，统一解决列内容显示不全的问题，同时保持现有固定操作列、横向滚动、分页、选择、排序、筛选、空态和接口行为不变。

## What I already know

- 后台基于 React 19、Semi Design 2.99.2。
- 标准资源页及代理页统一使用 `DataTable`，另有详情抽屉、KYC、行情配置、预测配置和 SMTP 共九处直接使用 Semi Table。
- 当前紧凑表格会为无宽度列补 160px，并按列宽总和计算 `scroll.x`。
- Semi 官方不推荐同时使用内置 `resizable` 与 `scroll.x`，固定列组合还要求保留无宽度列；这与“所有业务列可拖动”和现有固定操作列冲突。

## Requirements

### Shared behavior

- 新增唯一共享的可拖动表格封装，所有生产表格必须经该封装渲染。
- 每个应用声明的叶子列都必须有表头拖动手柄，包括固定操作列、动态详情列和配置工作台列。
- 未显式提供数值宽度的列使用统一默认宽度；现有数值宽度保持为初始值。
- 拖动时实时更新该列宽度并同步更新表格数字型横向滚动宽度。
- 列宽必须限制在安全最小值和最大值内，避免列消失或无限撑大页面。
- 不启用 Semi 原生 `resizable`，避免与 `scroll.x`、固定列组合产生重复列或错位。

### Interaction and accessibility

- 手柄位于表头列的右边缘，hover、focus 和拖动中都有清晰视觉反馈。
- 手柄使用 `role="separator"`、中文可访问名称和当前/最小/最大宽度语义。
- 支持鼠标/触控 Pointer 拖动；键盘左右方向键按步长调整，Home/End 跳到最小/最大宽度。
- 操作手柄不得触发表头排序、筛选或列内按钮。
- 拖动期间禁止文本误选，结束、卸载或指针取消后必须清理全局监听和拖动态。

### Coverage and compatibility

- `DataTable` 的本地/服务端分页、紧凑/自适应密度、行选择和稳定 `rowKey` 行为保持不变。
- 详情抽屉、KYC、行情配置、预测配置、SMTP 的直接表格全部接入共享封装，现有 `components` 自定义 body、loading、pagination、aria-label 和固定列配置保持不变。
- Semi 生成的选择框/展开工具列保持框架宽度；所有由应用 columns 数组声明的具名列均可调整。
- 调整宽度只保存在当前挂载的表格实例中，不新增后端字段或跨会话持久化。

## Acceptance Criteria

- [x] 生产源码中只有共享封装内部可以直接渲染 Semi `<Table>`。
- [x] 标准资源页、代理页以及九处独立表格的每个具名叶子列都显示项目级拖动手柄。
- [x] Pointer 拖动和键盘操作都能改变列宽，并遵守最小/最大值。
- [x] 拖动后 `scroll.x` 跟随列宽总和更新，固定右侧操作列无重复、无错位。
- [x] 原生 `.react-resizable-handle` 数量保持为 0。
- [x] 现有分页、选择、排序、筛选、loading、empty、detail 和业务动作测试不回归。
- [x] 后台 typecheck、lint、全量 test、build、`git diff --check` 通过。
- [x] Ego Browser 在 1728px 和 1280px 验证实际拖动、横向滚动、固定操作列以及 document 无横向溢出。

## Out of Scope

- 不持久化个人列宽偏好到 localStorage 或后端。
- 不新增列显隐、列排序、列冻结配置。
- 不修改任何 API、权限、分页语义或业务动作。

## Technical Notes

- 研究记录：`research/table-inventory-and-semi-constraints.md`。
- 共享封装必须保留 Semi Table 的泛型 `TableProps` 透传能力。
- 宽度键优先使用 column key/dataIndex，动态列缺失时再使用稳定路径索引。

## Definition of Done

- 实现、测试、浏览器验收、Admin UI 规范和进度记录全部更新。
- 最贴近改动的完整后台质量门通过。
