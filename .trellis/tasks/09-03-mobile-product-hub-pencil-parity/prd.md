# 手机端产品中心 Pencil 1:1 对齐

## Goal

以 Pencil 当前选中的 `Z0B0N6`（浅色）与 `zMsKE`（深色）为唯一视觉真值，精确重构手机端 `/products` 产品中心页面；只保留画板中的 Header、两条产品入口和一条产品说明入口，不增加卡片、装饰、分割线或其他设计。

## What I already know

- Pencil 当前同时选中分类标题节点 `d2FQe` 和两个手机画板 `Z0B0N6`、`zMsKE`；生产页面只映射两个 390×920 手机画板，分类标题节点不是运行时页面内容。
- 去除 Pencil 的 28px 操作系统状态栏后，生产页 Header 为 `y=0..60`，Body 从 `y=60` 开始，8px 顶部内边距使第一条产品行位于 `y=68`。
- 页面固定为两条 64px 产品行：预测、新闻中心；随后间隔 18px 放置一条 48px 产品说明入口。
- 现有路由和业务动作已正确：预测进入 `prediction`，新闻进入 `news`，产品说明进入带 `category=product` 的 `news`。
- 当前结构与纵向坐标已基本正确，但正常态仍存在 Pencil 未定义的底部分割线、语义强调色图标、错误字体尺寸/字重和不精确的明暗色板。

## Requirements

1. 生产页面只声明并实现 `Z0B0N6 zMsKE`，不把 `d2FQe` 分类标题画进手机页面。
2. 保持 60px Header、20px 水平页面边距、8px Body 顶部内边距、18px 分组间距。
3. 两条产品行均为 350×64、无背景、无边框、无圆角、14px 元素间距。
4. 产品图标圆盘为 44×44、22px 圆角、1px 精确主题边框；图标为 Lucide 19px 且使用主题正文色。
5. 产品标题为 Geist 15px/700/22px；说明为 Geist 11px/450/16px、固定 `#7A8B80`；文本组高度 41px、间距 3px。
6. 行尾 ChevronRight 为 18px、`#7A8B80`。
7. 产品说明入口为 350×48、无背景、无边框；BookOpen 为 16px 正文色，标签为 13px/650/19px 正文色，ChevronRight 为 16px、`#7A8B80`。
8. Header 返回图标保持 22px；右侧 Ellipsis 改为 Pencil 的 18px；透明控件中心坐标与画板一致。
9. 浅色根画布/正文/圆盘/边框为 `#FFFFFF`/`#111714`/`#FFFFFF`/`#CCD5D0`；深色为 `#000000`/`#F2F7F4`/`#0C100E`/`#29342E`。
10. 保留键盘焦点、44px 触控命中区、路由历史和 i18n；不添加画板外动画、阴影、渐变、说明或产品入口。

## Acceptance Criteria

- [x] 390×920 下 Header、Body、两行与 Help 的外框位置分别匹配去除状态栏后的 Pencil 坐标。
- [x] 正常态无任何产品行或 Help 分割线、卡片底色、圆角、阴影和额外装饰。
- [x] 标题、说明、Help 文案的字号、字重、行高、颜色和文本组高度与 Pencil 一致。
- [x] 两个圆盘在明暗主题下的背景、边框和 19px 正文色图标与 Pencil 一致。
- [x] Header 右侧 Ellipsis 为 18px，返回、标题和右侧动作中心保持画板坐标。
- [x] 页面仍只有预测、新闻中心和产品说明三个真实入口，且导航目的不变。
- [x] 320px、390px、448px 均无横向溢出，长英文文案在既有单行省略边界内。
- [x] 浅色与深色 Ego 浏览器截图完成逐屏核对。
- [x] 聚焦测试、Mobile 生产/测试类型检查、源码治理和 `git diff --check` 通过。

## Definition of Done

- 相关 Vue/CSS/i18n 合同测试更新并通过。
- Mobile 类型检查及最贴近改动的构建/治理验证通过。
- Mobile 规范与项目进度记录同步。
- 全部无关脏改动保持原样。

## Out of Scope

- 不改 Pencil `.pen` 文件。
- 不增加、删除或重命名产品业务入口。
- 不修改预测、新闻或后端接口。
- 不重构全局 PageHeader 或其他二级页面。
- 不处理 Pencil 的操作系统状态栏。

## Technical Notes

- Primary page: `mobile/src/views/ProductHubView.vue`。
- Cross-theme root boundary: `mobile/src/styles/pencil-selected-pages.css`。
- Shared header: `mobile/src/components/PageHeader.vue`，本任务只调整调用处图标尺寸。
- Focused contract: `mobile/tests/pencil-trading-product-selected-parity.test.ts`。
- Pencil measurements and runtime delta: `research/product-hub-pencil-truth.md`。
