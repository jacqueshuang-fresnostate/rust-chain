# 手机新闻页增加返回按钮

## Goal

让手机端 `/news` 恢复明确、可访问的二级页返回入口；从产品中心进入时返回原页面，直接打开或刷新新闻页时安全回退到产品中心。

## Requirements

- 将 `NewsView` 的 Pencil `PageHeader` 从根页样式改为显式二级页返回样式。
- 复用共享 `PageHeader`、Lucide `ArrowLeft` 和 `goBackOr`，不新增重复返回逻辑或图标。
- 将 `/news` 的 `meta.backFallback` 设为 `/products`，保证没有可用内部历史时不会退出应用。
- 保留搜索按钮、新闻分类、查询参数、新闻接口和详情页跳转现有行为。
- 同步更新导航规范与源代码回归测试。

## Acceptance Criteria

- [x] `/news` Header 渲染 44px 可点击返回入口并具有本地化 `aria-label`。
- [x] 从 `/products` 进入 `/news` 后点击返回可回到 `/products`。
- [x] 直接打开 `/news` 后点击返回使用 `router.replace('/products')` 兜底。
- [x] 搜索、分类、新闻数据加载和详情跳转代码不受影响。
- [x] 聚焦测试、Mobile 全量测试、type-check、PWA/Tauri 构建与 `git diff --check` 通过。

## Out of Scope

- 不重做新闻内容布局、搜索交互或新闻详情页。
- 不修改新闻后端 API、数据结构或 i18n 文案。
- 不修改其他页面的返回策略。

## Technical Notes

- 主要文件：`mobile/src/views/NewsView.vue`、`mobile/src/router/index.ts`。
- 共享返回行为：`mobile/src/components/PageHeader.vue` 调用 `mobile/src/core/navigation.ts` 的 `goBackOr`。
- 相关回归：`mobile/tests/ui-prototype-alignment-secondary.test.ts`，并新增 `/news` 历史/直开回退断言。
