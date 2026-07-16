# SMTP Config Creation SideSheet

## Goal

后台 SMTP 邮件配置页中，“新增发信配置”必须通过右侧 SideSheet 完成，不再占用主页面右侧编辑面板。

## Scope

- 发信配置列表的“新增配置”按钮打开 SideSheet。
- SideSheet 内提供新增发信配置所需字段，并提交到现有 `/admin/api/v1/smtp/configs` 新增接口。
- 新增成功后关闭 SideSheet、刷新配置列表，并选中新建配置。
- 现有编辑配置、验证码模板、发信策略、测试发送功能保持原行为。

## Acceptance Criteria

- 点击“新增配置”出现标题为“新增发信配置”的 SideSheet。
- SideSheet 中填写 SMTP 信息并确认后，请求体仍符合现有新增接口契约。
- 新增保存成功后 SideSheet 自动关闭，页面刷新。
- 回归测试覆盖新增 SideSheet 流程。
