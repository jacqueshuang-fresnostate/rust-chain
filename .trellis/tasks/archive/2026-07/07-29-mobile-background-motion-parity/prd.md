# 手机端背景与动效精确对齐

## Goal

以已确认的 Sites v16 原型和仓库内 `mobile/sites-prototype/app/` 源码为唯一视觉基准，把生产 Vue 手机端缺失的背景层、Canvas 动态、路由切换和微交互补齐，同时保留真实后端接口、PWA、Tauri 与 Android 行为。

## What I already know

- 生产端已拥有原型的静态 CSS 快照、字体和 HIPPO 品牌素材。
- 生产 `App.vue` 仅渲染静态桌面舞台图，没有渲染原型中的 `.ambient-layer`、`SignalField` 和 `.route-veil`。
- 原型的信号背景由 Canvas 逐帧绘制四组波形、粒子、扫描带、网格和指针吸附圆环，不是静态图片。
- 原型只在非二级、非现货、非合约的表现型一级页面展示信号背景。
- 原型根栏目切换使用方向性幕帘和 280ms 位移动画；二级页面使用更克制的 170-180ms 淡入。
- 原型已有完整的 `prefers-reduced-motion` 降级规则。
- 当前本地 390x844 运行画面中 Canvas 数量为 0，`.ambient-layer` 和 `.route-veil` 均不存在。
- 当前公开原型 390x844 运行画面中存在一个 `.signal-field` Canvas，CSS 尺寸覆盖可视区，内部像素按 DPR 扩展。

## Assumptions

- Sites v16 和仓库内 `mobile/sites-prototype/app/page.tsx`、`globals.css` 是本轮 1:1 对齐的事实来源。
- 不修改现有 API DTO、鉴权、交易提交、资产、借贷、消息和安全中心业务逻辑。
- 本轮只增加原型已经定义的动效，不引入新的第三方动画库或 WebGL 依赖。

## Requirements

- 将原型 `SignalField` 算法等价移植为 Vue 组件。
- Canvas 必须支持 DPR 上限、像素总量上限、窗口尺寸变化、指针/触控响应、页面隐藏暂停和卸载清理。
- 浅色与深色画布颜色必须对应原型参数。
- 低动态偏好下必须绘制确定性静态帧，停止逐帧循环，并保留 CSS 静态背景回退。
- 只在首页、行情、资产、个人中心等表现型一级页挂载信号背景；现货、合约、秒合约和二级页面不挂载。
- 生产应用壳必须渲染与原型同结构的 `.ambient-layer` 和 `.route-veil`。
- 根栏目切换需要按底栏次序区分前进、后退和原地切换，并触发原型同类幕帘。
- 二级页面切换保持快速、克制，不播放根栏目幕帘。
- 头部、底栏、按钮按压和聚焦反馈要与原型末级 CSS 规则一致，不降低 44x44 触控面积。
- 所有动画层不得遮挡 sticky header、底栏、弹窗或实际业务控件。
- 保留 PWA service worker、Tauri 构建隔离和真实后端数据契约。

## Acceptance Criteria

- [ ] 390x844 首页存在 `.ambient-layer > .signal-field-shell > canvas.signal-field`。
- [ ] Canvas CSS 尺寸覆盖可视区，内部像素尺寸按 DPR 和像素上限计算。
- [ ] 正常动态偏好下，两个不同时刻的画布帧摘要不同；低动态偏好下保持稳定。
- [ ] 从首页切换到行情与返回首页时，幕帘方向分别为 `forward` 和 `back`。
- [ ] 从一级页进入二级页时不播放根栏目幕帘，且 sticky header 始终位于内容之上。
- [ ] 现货、合约、秒合约和二级页不挂载动态信号背景。
- [ ] 深色和浅色模式均无空白 Canvas、内容遮挡、横向溢出或错误色块。
- [ ] 320x720、360x745、390x844、448x900 四个视口布局稳定。
- [ ] Canvas 在 `visibilitychange`、窗口变化和组件卸载时正确暂停、重绘或清理。
- [ ] `npm run type-check`、`npm test`、`npm run build:pwa`、`npm run build:tauri` 全部通过。
- [ ] Android aarch64 debug APK 构建通过，并能在已连接设备上启动到首页。

## Definition of Done

- 测试覆盖 Canvas 运行合同、壳层 DOM 和路由方向状态。
- 完成浏览器同视口视觉回归与控制台检查。
- 更新 mobile 规范和 `docs/superpowers/PROGRESS.md`。
- 完成 Trellis check、提交和任务归档。

## Out of Scope

- 不调整后端接口、数据字段或鉴权流程。
- 不重新设计已经完成的业务卡片、表单和二级页信息架构。
- 不修改公开 Sites 原型。
- 不引入虚构行情、余额、借贷或账户数据。

## Technical Notes

- 事实来源：`mobile/sites-prototype/app/page.tsx` 中的 `SignalField`、路由方向状态和壳层 JSX。
- 静态规则来源：`mobile/src/styles/prototype-base.css`，尤其是 `.ambient-layer`、`.signal-field-*`、`.route-veil-*`、`.bottom-nav` 和 reduced-motion 段。
- 生产入口：`mobile/src/App.vue`、`mobile/src/core/navigation.ts`、`mobile/src/components/AppBottomNav.vue`。
- 视觉审计记录：`research/motion-parity-audit.md`。
