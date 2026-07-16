# PC 注册接口重新接入邮箱验证码与邀请码策略

## Goal

PC 注册页需要接入真实后端注册链路：邮箱验证码可以由后端发送并在注册时校验；后台安全策略可以配置邀请码是选填还是必填；PC 表单根据公开注册配置展示和校验邀请码字段。

## What I Already Know

- PC `Register.vue` 已有邮箱验证码输入框和邀请码输入框，但 `sendVerifyCode` 当前是本地 mock，并提示“后端不需要验证码”。
- PC `register` 请求当前只提交 `email/password/country_code`，没有提交验证码和邀请码。
- 后端 `/api/v1/auth/register` 当前只创建用户并发 token，没有注册邮箱验证码发送/校验，也没有邀请码绑定逻辑。
- 已有邮箱验证码表 `user_email_verifications` 依赖已存在用户的 `user_id`，不适合注册前邮箱验证。
- 已有后台安全策略 `UserSecurityPolicy` 保存登录 2FA 和资金动作校验，可扩展注册策略。

## Requirements

- 新增公开注册配置接口，返回注册邮箱验证码是否必填、邀请码是否必填。
- 后台安全策略支持配置“注册邀请码必填/选填”，默认保持选填，避免影响现有开放注册。
- 新增注册邮箱验证码发送接口，复用已启用 SMTP 配置和验证码 HTML 模板能力。
- 用户注册必须提交邮箱验证码；验证码错误、过期或尝试次数过多时拒绝注册。
- 注册时如果后台配置邀请码必填，则邀请码为空必须拒绝；如果填写邀请码，则必须校验邀请码有效，并在注册成功时绑定邀请关系。
- PC 注册页加载公开注册配置，验证码按钮调用真实接口，注册时提交 `code` 和 `invite_code`。
- PC 注册页和 i18n 文案移除“当前后端不需要验证码”语义，邀请码 label 按策略显示必填/选填。

## Acceptance Criteria

- [ ] PC 点击发送验证码会调用后端注册验证码接口。
- [ ] PC 注册请求会携带邮箱验证码、国家代码和邀请码。
- [ ] 后台安全策略页可以保存注册邀请码必填/选填。
- [ ] 公开注册配置可以反映后台邀请码策略。
- [ ] 没有有效邮箱验证码时，后端注册返回参数错误，不创建用户。
- [ ] 邀请码必填时，空邀请码注册返回参数错误。
- [ ] 填写有效邀请码注册后，用户邀请关系被写入。

## Out of Scope

- 登录密码找回接口。
- PC 注册页整体视觉重构。
- 代理邀请码管理规则调整。
- 邮箱验证码模板编辑能力，本任务只复用现有模板。

## Technical Notes

- 关键后端文件：`src/modules/auth/routes.rs`、`src/modules/security.rs`、`src/modules/admin/routes.rs`、`src/openapi.rs`。
- 关键 PC 文件：`pc/src/api/auth.ts`、`pc/src/views/auth/Register.vue`、`pc/src/i18n/index.ts`。
- 关键后台文件：`web/src/admin/actions/SecurityPolicyPage.tsx`。
- 新注册验证码表应独立于 `user_email_verifications`，因为注册前还没有 `user_id`。
