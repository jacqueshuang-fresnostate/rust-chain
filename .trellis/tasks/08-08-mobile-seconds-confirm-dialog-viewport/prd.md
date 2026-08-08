# 修复秒合约确认下单弹层底部裁切

## Goal

修复手机端秒合约“确认下单”弹层在较矮视口中底部内容和操作按钮只能露出一部分的问题，使弹层始终位于真实视觉视口内，头部与操作区完整可见，仅中间订单明细按需滚动。

## What I already know

- 当前确认层嵌套在 `SecondsView` 路由根节点内，而路由节点长期保留动画 `transform`，会让后代 `position: fixed` 相对变换容器而不是真实视口定位。
- 当前 `.seconds-dialog` 整体设置 `overflow-y: auto`，订单明细较高时底部按钮位于首屏折叠区域。
- 截图中弹层顶部正常、底部操作按钮只显示上缘，符合“整张弹层滚动而非中间内容滚动”的表现。
- 页面已经具备 Escape、Tab 焦点闭环、背景滚动锁、焦点恢复和提交中关闭保护。

## Requirements

- 使用 Vue `Teleport to="body"` 将秒合约确认层挂载到路由变换容器之外。
- 确认层使用 `position: fixed; inset: 0` 和动态视口/安全区内边距，覆盖真实视觉视口。
- 弹层采用三行结构：固定头部、`minmax(0, 1fr)` 可滚动明细区、固定操作区。
- 只有明细区设置 `overflow-y: auto` 与 `overscroll-behavior: contain`；弹层本体不得整体滚动。
- 取消与确认按钮在普通手机和矮屏中始终完整可见，窄屏单列按钮也不得被裁切。
- 保留遮罩点击关闭、Escape、Tab 闭环、初始取消焦点、背景滚动锁、焦点恢复、提交中禁用和现有下单接口载荷。
- 不影响秒合约行情、金额校验、产品周期、订单提交和历史记录。

## Acceptance Criteria

- [x] 确认层通过 `Teleport` 挂载到 `body`，运行时父节点不再位于 `.view-stack` 内。
- [x] `.seconds-dialog` 为 `auto minmax(0, 1fr) auto`，本体 `overflow: hidden`。
- [x] `.seconds-dialog__body` 独立滚动并保留明细、错误反馈；操作按钮位于滚动区之外。
- [x] 320×568、320×720、390×667、390×844、448×900 下弹层不超出视口，两个按钮均完整可见且可操作。
- [x] 安全区、明暗主题、Escape、Tab、遮罩关闭、滚动锁与焦点恢复保持正常。
- [x] 聚焦测试、mobile type-check、全量测试、PWA/Tauri 构建和 `git diff --check` 通过。

## Out of Scope

- 不调整秒合约下单字段、赔率、金额、周期和业务接口。
- 不重做其他业务弹层。
- 不修改秒合约页面主体布局。

## Technical Notes

- 实现文件：`mobile/src/views/SecondsView.vue`。
- 回归测试优先扩展 `mobile/tests/pencil-trading-product-selected-parity.test.ts`。
- 需遵循 `.trellis/spec/mobile/index.md` 与 `.trellis/spec/mobile/pwa-and-shell.md` 的 Teleport、视口、安全区、触控和质量门合同。

## Definition of Done

- Teleport、内部滚动布局、交互回归、矮屏浏览器验收、全量质量门和进度记录全部完成。
