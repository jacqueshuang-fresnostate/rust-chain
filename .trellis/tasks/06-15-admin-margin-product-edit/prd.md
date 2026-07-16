# 后台杠杆产品支持修改

## 背景

后台杠杆产品目前支持新增、查看详情、启用/禁用，但列表行级操作没有完整修改入口。用户希望杠杆产品可以进行修改，方便调整 Logo、保证金模式、杠杆档位、风控参数和状态。

## 目标

- 后端新增管理员修改杠杆产品接口。
- 后台杠杆产品列表新增“修改”行级操作。
- 修改表单复用新增杠杆产品的 tabs、下拉、上传和多选结构。
- 修改成功后关闭 SideSheet 并刷新列表。
- 保持现有新增、详情、状态切换逻辑不变。

## 范围

- `src/modules/margin/routes.rs`
- `tests/margin_routes.rs`
- `web/src/admin/resources/ResourceCreateActions.tsx`
- `web/src/admin/resources/resourceConfigs.test.tsx`
- 任务进度记录

## 不在范围

- 不调整杠杆仓位、强平、利息汇总页面。
- 不新增数据库字段或迁移。
- 不改 PC 端杠杆交易页面。

## 验收标准

- `PATCH /admin/api/v1/margin/products/:id` 可更新杠杆产品业务配置，并写入管理员审计日志。
- 后台杠杆产品列表行级操作出现“修改”按钮。
- 修改 SideSheet 预填当前行数据，支持提交并刷新列表。
- 目标后端测试、前端资源配置测试、类型检查和 diff 检查通过。
