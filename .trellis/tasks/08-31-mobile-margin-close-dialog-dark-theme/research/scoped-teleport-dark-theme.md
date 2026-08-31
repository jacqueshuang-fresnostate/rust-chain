# Vue scoped CSS + Teleport 深色主题根因

## 运行时主题边界

- `mobile/src/stores/theme.ts` 的 `applyAppTheme()` 把权威主题写入 `document.documentElement.dataset.theme`。
- `MarginCloseSheet.vue` Teleport 到 `body`，因此不能依赖 `.app-stage.theme-dark` 的祖先关系；`html[data-theme='dark']` 是正确边界。

## 当前编译证据

源码：

```css
:global(html[data-theme='dark']) .margin-close-sheet { /* dark variables */ }
```

使用项目同版本 `vue/compiler-sfc` 的 `compileStyle({ scoped: true })` 编译后变为：

```css
html[data-theme='dark'] { /* dark variables */ }
```

`.margin-close-sheet` 后代部分消失。深色变量虽然存在于 `<html>`，但弹窗自身的浅色变量是本地声明，层叠时本地值覆盖继承值，因此面板仍为白色。

Ego Browser 在真实 Vite 页面中读取到的编译规则和 computed style 与上述一致：

- 基础规则：`.margin-close-sheet[data-v-5fd61511]`，其本地变量为浅色。
- 深色规则：仅 `html[data-theme="dark"]`，没有弹窗后代选择器。
- `<html data-theme="dark">` 下弹窗计算背景仍为 `rgb(255, 255, 255)`，文字为 `rgb(16, 21, 18)`，`--close-sheet-field` 仍为 `#f2f4f3`。

## 修复合同

整条选择器放入 `:global(...)`：

```css
:global(html[data-theme='dark'] .margin-close-sheet) { /* dark variables */ }
```

编译输出必须仍包含：

```css
html[data-theme='dark'] .margin-close-sheet { /* dark variables */ }
```

组件类名在项目内唯一，整条 global 不会扩大到其他弹层。编译级测试应验证目标后代选择器，并拒绝携带深色变量的裸 HTML 规则。
