# 移除 PC Header 多余品牌文本

## Goal

PC 顶部导航 Header 的 logo 区域目前会在图片旁渲染平台名称文本，用户指出 selector `#app > div > div > header > div.flex.items-center.gap-8 > div > div > span` 对应的这个 span 不需要。需要移除 Header 中的品牌文字，只保留 logo 图片和点击回首页行为。

## Requirements

- Header 左侧 logo 区域不再渲染 `BrandLogo` 的平台名称 span。
- 保留 Header logo 图片展示。
- 保留 Header logo 区域点击跳转首页。
- 不修改登录、注册、忘记密码页面的 logo 展示。
- 不修改 `BrandLogo` 组件默认能力，避免影响其他未来调用。

## Acceptance Criteria

- [ ] `pc/src/components/layout/Header.vue` 中 Header logo 不再传入 `show-name`。
- [ ] Header logo 仍使用 `BrandLogo` 图片。
- [ ] 相关静态测试覆盖 Header 不显示 `BrandLogo` 文本。
- [ ] PC 类型检查通过。

## Out of Scope

- Header 整体布局重构。
- 平台名称、浏览器 title 或后台品牌配置改动。
- 移除 `BrandLogo` 组件的 `showName` 功能。

## Technical Notes

- 代码索引确认 span 来自 `pc/src/components/common/BrandLogo.vue` 的 `<span v-if="showName">`。
- Header 当前在 `pc/src/components/layout/Header.vue` 使用 `<BrandLogo show-name ... name-class=... />`，这是本次唯一需要改的渲染入口。
