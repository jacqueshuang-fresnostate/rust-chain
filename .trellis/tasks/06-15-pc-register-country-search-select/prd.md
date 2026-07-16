# PC端创建账户国家地区搜索下拉框

## Goal

PC 端创建账户页面的“国家 / 地区”选择体验需要优化，支持在下拉框内搜索国家名称或国家代码，方便国家列表较多时快速选择。

## What I Already Know

- 当前创建账户页面是 `pc/src/views/auth/Register.vue`。
- 当前国家 / 地区使用原生 `<select>` 渲染，数据来自 `fetchPublicCountries()` 和后端 `/countries`。
- 注册提交仍需要传 `countryCode: form.value.countryCode`，不能改变后端请求字段。
- 国家选项格式为 `PcCountryOption { code, name, defaultLocale, supportedLocales }`。
- KYC 页面也有国家选择，但用户本次明确说“创建账户”，本任务只改注册页。

## Requirements

- 注册页国家 / 地区选择器支持搜索。
- 搜索应匹配国家名称和国家代码。
- 下拉展示国家名称和代码，例如 `中国` / `CN`。
- 保留原有国家列表加载、默认选中第一个国家、无国家可注册提示、注册提交字段逻辑。
- 加载中或无国家时选择器不可交互。
- 不影响 KYC 页面国家选择器。

## Acceptance Criteria

- [x] 注册页不再使用原生 `<select>` 作为国家 / 地区选择器。
- [x] 注册页国家 / 地区下拉包含搜索输入。
- [x] 搜索逻辑同时匹配 `country.name` 和 `country.code`。
- [x] 选择国家后仍写入 `form.countryCode` 并用于注册请求。
- [x] PC 端相关测试和类型检查通过。

## Definition of Done

- 只修改 PC 注册页国家 / 地区选择器及相关 i18n/测试。
- 执行最贴近改动的 PC 测试和类型检查。
- 更新 `docs/superpowers/PROGRESS.md`。

## Technical Approach

在 `Register.vue` 中使用现有 Vue/Tailwind 模式实现轻量 searchable dropdown：按钮显示当前选中国家；展开后显示搜索输入和过滤后的国家列表；点击国家设置 `form.countryCode` 并关闭下拉；点击外部关闭下拉。新增 i18n 文案和源码级回归测试，避免回退到原生 select。

## Out of Scope

- 不调整后端 `/countries` 接口。
- 不改变注册请求字段。
- 不改 KYC 国家选择器。
- 不引入新的 UI 组件库。

## Technical Notes

- 相关文件：`pc/src/views/auth/Register.vue`、`pc/src/i18n/index.ts`、`pc/tests/*`。
- 现有 PC 测试使用 Node 内置 test runner 和源码契约断言，本任务沿用该轻量方式。
