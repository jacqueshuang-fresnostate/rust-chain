# 手机端根导航与交易选择弹窗对齐 Pencil 选稿

## Goal

以用户当前在 Pencil 中选中的 `X0ux9F`“首页 / 01-2 / 访客状态 / 交易导航选择弹窗 / 浅色主题”为视觉基线，1:1 重构生产手机端五栏根 Dock 与中央交易入口弹窗，使轮廓、尺寸、间距、遮罩、关闭按钮和过渡动效对应选稿，同时让四个入口连接当前真实可用的现货、合约、秒合约和闪兑页面。

## What I already know

- 当前 Pencil 文件为 `/private/tmp/hippo-mobile-uiux-position-tab.pen`，当前选中节点为 `X0ux9F`。
- 选稿 Dock 为 358×68px、24px 圆角、16px 水平内边距；外层导航高 84px，中央交易球为 56×56px，位于 Dock 顶部 `-12px`。
- 弹窗遮罩覆盖整屏，颜色为 `#00000059`；白色特殊轮廓为 358×300px，并在底部中央为 54px 关闭按钮保留凹形缺口。
- 弹窗内容为四行，每行 330×58px、12px 圆角、18px 水平内边距、16px 图文间距；根据最终产品反馈，四行不表达当前项，不显示 `active` 背景或尾部勾选标记。
- 当前生产 `AppBottomNav.vue` 仍把中央交易按钮直接替换到最近现货/合约路由，没有展示选稿弹窗；交易球位置仍为 `top: -18px`。
- 当前真实且独立的交易页面为现货、合约、秒合约和闪兑。Pencil 示例中的“策略交易/期权”没有对应的生产路由或后端能力，因此生产文案映射为“秒合约/闪兑”，不创建无响应假控件。

## Requirements

### 1. 根 Dock

- 保留五个有序根入口：首页、行情、交易、资产、我的。
- Dock 继续使用 `router.replace` 切换根栏目；普通入口保持 44px 最小触控目标。
- 中央交易入口不再直接跳转，改为打开选择弹窗；按钮声明 `aria-haspopup="dialog"`、`aria-expanded` 和 `aria-controls`。
- 390px 基准下交易球严格为 56×56px、Dock 顶部 `-12px`，其余几何与 `lsATG/XV7iv/t1KVLE` 一致。

### 2. 交易选择弹窗

- 使用 Teleport 到 `body`，避免路由过渡和应用舞台 transform 破坏 fixed 定位。
- 整屏遮罩、特殊 SVG 路径轮廓、内容轨道和关闭按钮严格使用 `RtubA/U99rP/n1BXc/eLvdo/QrlAB` 的尺寸与相对位置。
- 四行生产入口为现货、合约、秒合约、闪兑，分别使用 Lucide 图标，不使用表情符号。
- 现货与合约恢复最近交易对并使用 replace；合约保留 `mode=contract`；秒合约使用既有带来源状态的 replace 目标；闪兑作为二级工作流 push 命名路由 `swap`，使返回动作回到弹窗来源页。
- 四个入口保持一致的静态样式；选择后关闭弹窗并执行一次类型化导航。

### 3. 可访问性和交互

- 弹窗使用 `role="dialog"`、`aria-modal="true"` 和本地化标题，四个入口使用原生按钮语义，不伪装为单选组。
- 打开时锁定 body 滚动并聚焦关闭按钮；Tab/Shift+Tab 在弹窗内循环，Escape、遮罩和关闭按钮均可关闭。
- 关闭后焦点精确返回中央交易触发器；路由变化和组件卸载不能遗留 body 滚动锁。
- 打开状态下隐藏原中央交易球的交互面，由 54×54px 关闭按钮在原位置接管；背景 Dock 保持选稿中的遮罩层次。

### 4. 主题、响应式和动效

- 浅色严格使用选稿的白色表面、`#111111` 文字与 `#00000059` 遮罩；深色使用现有语义变量进行等价适配，不引入第二套主题状态。
- 弹窗宽度在 390px 为 358px；320–448px 按 `viewport - 32px` 流式缩放且不横向溢出，内容保持两侧 14px 内缩。
- 底部位置通过安全区计算：轮廓底部始终位于 Dock 上沿上方 4px，关闭按钮底部保留 35px 加安全区。
- 正常动效使用遮罩淡入、轮廓从中央交易球轻微上浮展开、关闭按钮旋转缩放；`prefers-reduced-motion` 下全部取消。

## Acceptance Criteria

- [x] `AppBottomNav.vue` 中央交易按钮只打开弹窗，不直接导航。
- [x] 选稿根 Dock 保持 84/68/56px 几何，中央交易球 `top: -12px`。
- [x] 弹窗使用选稿原始 SVG path、358×300px 轮廓、330×58px 四行、54px 关闭按钮和 35% 遮罩。
- [x] 现货、合约、秒合约、闪兑四个真实路由均可从弹窗进入，根栏目继续使用 replace。
- [x] 四个入口不绑定 `active`、`aria-checked` 或单选组状态，也不渲染 `trade-navigation-picker__selection` 尾部标记。
- [x] 关闭、Escape、遮罩、Tab 循环、body 锁定和焦点恢复均通过回归测试。
- [x] 中文和英文文案完整对称，图标统一 Lucide，无 emoji、无无效按钮。
- [x] 320/390/448px 明暗主题无横向溢出，安全区和 reduced-motion 生效。
- [x] Mobile 定向/全量测试、类型检查、PWA/Tauri 构建、Trellis validate 与 `git diff --check` 全部通过。
- [x] Ego Browser 完成弹窗开关、四路由和多尺寸视觉验收。

## Definition of Done

- 先添加会失败的根导航弹窗合同测试，再修改生产实现。
- 只修改手机端根导航、弹窗、i18n、对应测试、Mobile 规范与任务/进度记录。
- 不覆盖既有 `mobile/pencil/docs/superpowers/PROGRESS.md` 修改。
- 完成代码复核、测试、构建与真实浏览器验收。

## Technical Approach

1. 在现有 `AppBottomNav.vue` 内复用 `useModalDialog`，不重复实现 body 锁定、Tab 循环和焦点恢复。
2. 用内联 SVG 仅绘制 Pencil 的结构轮廓，业务图标全部来自 `lucide-vue-next`；轮廓通过 `preserveAspectRatio="none"` 随 320–448px 宽度缩放。
3. 用 `createBottomNavSecondsTarget` 保留秒合约来源语义；现货和合约使用类型化 replace，闪兑使用命名路由 push 并保留合理返回历史。
4. 在 `prototype-parity.css` 的最终覆盖层实现弹窗几何、主题和过渡，避免修改原型快照基线。

## Decision (ADR-lite)

**Context**：Pencil 示例后两行写作“策略交易/期权”，但当前生产没有对应路由或服务能力，而用户同时要求修复不合理跳转。

**Decision**：严格复刻四行视觉结构与交互，将四行绑定为现货、合约、秒合约和闪兑四个真实页面；不创建假路由、禁用占位行或点击无反馈的控件。

**Consequences**：几何与视觉可 1:1 验收，所有入口均真实可用；示例后两行文案按生产业务语义映射，而不是照搬无能力支撑的静态文案。

## Research References

- [`research/pencil-selected-navigation-spec.md`](research/pencil-selected-navigation-spec.md) — `X0ux9F`、Dock、轮廓、内容与关闭按钮的精确规格。
- [`research/pencil-reference/X0ux9F.png`](research/pencil-reference/X0ux9F.png) — 完整浅色选中画板导出图。
- [`research/pencil-reference/U99rP.png`](research/pencil-reference/U99rP.png) — 特殊轮廓弹窗背景导出图。
- [`research/pencil-reference/eLvdo.png`](research/pencil-reference/eLvdo.png) — 四行交易选择内容导出图。
- [`research/pencil-reference/lsATG.png`](research/pencil-reference/lsATG.png) — 根 Dock 导出图。

## Out of Scope

- 不新增策略交易或期权后端、路由和订单能力。
- 不修改现货、合约、秒合约或闪兑页面内部业务。
- 不重设计 Header、首页内容或其他二级弹层。
- 不修改后端接口、数据库或部署配置。

## Technical Notes

- 主要实现：`mobile/src/components/AppBottomNav.vue`、`mobile/src/styles/prototype-parity.css`。
- 重点回归：`mobile/tests/{root-trade-navigation-modal,shell-navigation,root-prototype-parity,pencil-selected-home-layout,ui-prototype-alignment-foundation}.test.ts`。
- 适用规范：`.trellis/spec/mobile/{index,navigation-and-localization,pwa-and-shell}.md`。
