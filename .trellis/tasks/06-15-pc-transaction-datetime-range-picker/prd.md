# PC端交易记录日期时间弹窗筛选

## Goal

PC 端用户中心交易记录的“日期范围”筛选需要支持选择具体时间，并通过弹窗完成开始/结束时间选择，同时补齐中英文 i18n 文案。

## What I Already Know

- 当前页面是 `pc/src/views/User/Transaction.vue`。
- 现有筛选使用两个原生 `<input type="date">`，只能选日期。
- 当前 `fetchTransactionHistory` 只通过 `record.createTime.slice(0, 10)` 比较日期，无法精确到时间。
- PC 端已有轻量弹层/下拉写法，例如注册页国家选择和闪兑资产选择，使用 Vue + Tailwind，不需要引入新库。

## Requirements

- 交易记录页日期范围改成点击触发的时间选择弹窗。
- 弹窗内包含开始时间和结束时间，输入类型使用 `datetime-local`，精确到分钟。
- 弹窗提供清空、取消、确认操作。
- 确认后才应用到列表筛选；取消不改变当前筛选。
- 如果结束时间早于开始时间，显示 i18n 错误并不应用筛选。
- 筛选逻辑支持 `datetime-local` 的 `YYYY-MM-DDTHH:mm` 格式，也兼容旧的 `YYYY-MM-DD` 日期格式。
- 日期时间相关按钮、标签、错误提示需要中英文 i18n。

## Acceptance Criteria

- [ ] 交易记录页不再渲染原生 `type="date"` 日期范围筛选。
- [ ] 点击日期范围控件会显示包含两个 `datetime-local` 输入框的弹窗。
- [ ] 选择开始/结束时间并确认后，PC 交易记录按完整时间范围过滤。
- [ ] 结束时间早于开始时间时显示本地化错误，不触发应用。
- [ ] 中英文环境下日期范围弹窗文案均来自 i18n。
- [ ] PC 相关测试和类型检查通过。

## Out of Scope

- 不改后端 `/wallet/ledger` 接口。
- 不新增第三方日期时间选择器依赖。
- 不调整交易记录表格结构或闪兑记录 tab。

## Technical Notes

- 相关文件：`pc/src/views/User/Transaction.vue`、`pc/src/api/transaction.ts`、`pc/src/i18n/index.ts`。
- 可复用现有注册页下拉的外部点击关闭模式。
