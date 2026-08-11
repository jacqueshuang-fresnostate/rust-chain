# 统一后台表格操作按钮尺寸与操作列宽

## Goal

统一后台业务表格“操作”列的视觉密度和布局约束：操作按钮使用更紧凑的尺寸，按钮组始终保持单行，操作列获得足够的初始宽度，避免中文操作文案被压缩后换行，同时保留现有列拖拽能力。

## What I already know

- 后台前端位于 `web`，使用 React 19、Semi Design 和项目级 `ResizableTable` / `DataTable`。
- 标准资源页的操作列由 `AdminResourcePage` 统一生成，当前固定在右侧、宽度为 216px，并使用允许换行的 `Space`。
- 自定义后台页面还存在若干 `key: 'actions'` 的操作列，初始宽度从 100px 到 520px 不等。
- 表格行内按钮当前由全局样式设为 30px 高、较大的水平内边距；通用 `span` 规则还会让按钮内容继承单元格的换行行为。
- 审计日志中也有名为“操作”的业务数据列，不能仅凭标题把它误判为按钮操作列。

## Assumptions

- 本次只调整后台表格行内操作按钮，不改变页面头部按钮、表单按钮和弹窗确认按钮。
- 使用列 `key: 'actions'` 作为操作列的明确标记；缺失该 key 的真实操作列会在本次补齐。
- 标准资源页操作列加宽到 288px；已有显式宽度的自定义操作列保持业务需要，但不得被拖拽到小于 120px。
- 代理管理等确实包含大量操作的列继续保留更大的显式宽度。

## Requirements

- 表格操作按钮高度和水平内边距统一缩小，文字保持单行。
- 操作按钮组不换行、不被通用单元格 `span` 样式拉伸。
- 操作列标题和单元格获得可识别的专用 class，便于稳定地限定样式。
- 操作列的拖拽最小宽度高于普通数据列，防止再次被缩窄到按钮换行。
- 标准资源页操作列固定右侧并使用更合理的初始宽度。
- 非操作业务列（例如审计日志中的“操作”字段）不受操作列专用规则影响。

## Acceptance Criteria

- [x] 后台业务表格中的操作按钮比当前 30px 按钮更紧凑，且中文按钮文案不换行。
- [x] 标准资源页操作列中的多个按钮保持在同一行，初始宽度为 288px。
- [x] `key: 'actions'` 操作列最小可拖拽宽度为 120px，普通列仍可缩到 80px。
- [x] SMTP、KYC、行情订阅、竞猜配置、代理管理等自定义操作列被统一识别并应用操作列样式。
- [x] 审计日志的业务“操作”字段不被标记为按钮操作列。
- [x] 相关单元测试覆盖操作列 class、宽度边界和标准资源页单行按钮组。
- [x] `typecheck`、`lint`、相关测试、全量测试、构建和 `git diff --check` 通过。

## Definition of Done

- Tests added/updated (unit/integration where appropriate)
- Lint / typecheck / CI green
- `docs/superpowers/PROGRESS.md` 已更新
- 管理后台表格操作列视觉回归已通过本地浏览器检查，或明确记录未能检查的原因

## Out of Scope

- 不修改移动端、PC 用户端或后端接口。
- 不重新设计后台表格其他业务列、筛选器、分页器和弹窗。
- 不改变操作按钮对应的权限、确认流程和接口行为。

## Technical Notes

- 重点文件：
  - `web/src/shared/ResizableTable.tsx`
  - `web/src/admin/resources/AdminResourcePage.tsx`
  - `web/src/admin/actions/*Page.tsx`
  - `web/src/styles.css`
- 相关规范：`.trellis/spec/admin/ui-system.md`、`.trellis/spec/guides/index.md`。
- 通过 `key: 'actions'` 识别操作列，避免误伤 `resourceConfigs.tsx` 中审计日志的 `{ key: 'action', title: '操作' }` 数据列。
