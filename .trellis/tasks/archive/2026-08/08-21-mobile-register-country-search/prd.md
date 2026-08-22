# 手机端注册国家支持搜索

## Goal

将手机端注册页原生国家下拉框升级为适合长列表的可搜索选择器，让用户可以按当前语言下的国家/地区名称、后端名称或 ISO 国家代码快速定位，并保持现有注册接口载荷不变。

## What I already know

- 当前注册页通过 `fetchCountries()` 获取国家列表，失败时使用本地基础列表。
- 当前控件是原生 `select`，列表可以选择但不能提供一致的站内搜索体验。
- 注册接口仍只需要已选国家的 `countryCode`，本任务不修改后端契约。
- 项目已有 `useModalDialog`，可复用滚动锁、初始焦点、Escape、Tab 闭环和焦点恢复能力。
- 用户界面必须保持中英文 i18n、Lucide 图标、44px 触控目标和 320–448px 无横向溢出。

## Assumptions

- “注册的时候”指手机端 `/register` 页面，不扩展到 KYC 国家选择。
- 点击国家字段后打开底部选择弹层，搜索框是弹层初始焦点。
- 搜索不区分大小写，并忽略重音符号；匹配本地化名称、后端原始名称和 ISO 代码。

## Requirements

- 国家字段保持现有 48px Pencil 表单几何，但改为明确的对话框触发按钮。
- 弹层展示标题、关闭按钮、搜索框、国家代码、国家名称与当前选择状态。
- 输入关键词时实时过滤；没有匹配项时显示本地化空状态。
- 选择国家后更新 `countryCode`、关闭弹层并恢复触发器焦点；遮罩、关闭按钮和 Escape 只关闭而不修改选择。
- 弹层打开时锁定页面滚动并形成 Tab 焦点闭环，卸载时清理状态。
- 国家数据加载失败时，基础列表仍可搜索。
- 保留现有系统地区默认值、注册校验、邀请码、验证码与提交跳转逻辑。

## Acceptance Criteria

- [x] 注册页国家字段可以打开搜索弹层，搜索输入自动获得焦点。
- [x] 中文/英文国家名、后端名称及 ISO 代码均可过滤国家列表。
- [x] 选择结果仍以 ISO 代码传给 `registerWithEmail`。
- [x] 当前选中项明确显示，关闭、Escape、Tab 闭环和焦点恢复可用。
- [x] 空搜索结果、标题、占位文案和辅助标签均完成中英文 i18n。
- [x] 聚焦测试、Mobile 全量测试、type-check、PWA 与 Tauri 构建通过。

## Quality Review

- 国家搜索聚焦测试与原认证合同测试 11/11 通过，Mobile 全量 470/470 通过。
- `npm run type-check`、`npm run build:pwa`、`npm run build:tauri` 通过；项目没有单独 lint script。
- Ego Browser 在 390×844 浅色验证 249 个真实国家、名称/代码过滤、选择与焦点恢复，在 320×720 深色验证无结果状态、滚动锁、Escape 与零横向溢出。
- 注册接口、系统地区默认值、验证码、邀请码、注册跳转和 KYC 页面均未改动。

## Out of Scope

- 修改后端国家列表或注册接口。
- 修改 KYC 国家选择器。
- 增加国旗图片、电话区号或地理定位权限。
- 改动注册页其他表单业务逻辑。

## Technical Notes

- 页面：`mobile/src/views/RegisterView.vue`。
- i18n：`mobile/src/i18n/messages/zh-CN.ts`、`mobile/src/i18n/messages/en.ts`。
- 既有模态生命周期：`mobile/src/core/modalDialog.ts`。
- 现有回归入口：`mobile/tests/access-identity-settings-views.test.ts`。
