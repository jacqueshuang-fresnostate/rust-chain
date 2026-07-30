# 优化手机端 Header 与全局配色

## Goal

优化生产 `mobile/` Vue/Tauri 客户端的 Header、明暗主题色板、按钮与输入表面层级，使根页面和二级页面在保持现有原型几何、真实接口和交互契约的前提下，获得更清晰、明亮、稳定且一致的视觉体验。

## What I Already Know

- 当前生产端已经有 64px RootHeader、76px PageHeader、44x44 拟物化 Header 图标控件和 sticky `z-index: 70` 契约。
- 生产视觉快照位于 `mobile/src/styles/prototype-base.css`，运行时修正应追加在 `mobile/src/styles/prototype-parity.css`。
- RootHeader 使用官方 HIPPO 紧凑 Logo、主题按钮和消息按钮；PageHeader 负责返回、标题、场景标签、说明和可选 action。
- 浅色 `page/surface/surface-2` 过于接近，贷款、安全、交易等页面的字段、卡片和分隔层级偏弱。
- 深色主题偏绿黑，Logo 对比不足；首页禁用公告卡存在白色表面配浅色文字的问题。
- 所有图标必须继续使用 Lucide，不使用表情符号。

## Requirements

- 将明暗主题从偏绿的单色体系调整为冷中性基础色，并保留绿色、蓝色、珊瑚色的业务状态角色。
- 浅色主题必须明显区分页面、普通表面、次级表面和边框；禁止使用已废弃的 `#0b1811` / `rgba(11, 24, 17, ...)` 色族。
- 深色主题必须保持足够的文本和控件对比，修复公告卡、Logo、禁用态和状态文案的颜色冲突。
- RootHeader 和 PageHeader 使用统一但不过度厚重的拟物材质，保持 Logo 清晰、图标居中、消息点附着和完整键盘焦点环。
- PageHeader 标题、eyebrow、subtitle 和底部信号轨形成清晰层级，但不改变返回逻辑、action slot 或 Header 几何。
- 统一主按钮、次按钮、选中态、输入框、状态卡的表面、边框、按压、聚焦和禁用表现。
- 重点覆盖首页、行情/交易、资产、我的、消息中心、贷款和安全中心。
- 保留当前路由、接口、真实数据诚实状态、PWA/Tauri 构建隔离和动态背景行为。

## Acceptance Criteria

- [x] 明暗主题的页面、表面、边框、正文、弱化文字和状态色令牌具有明确层级。
- [x] 深色首页公告卡不存在白底白字或低对比文案，深色 Logo 清晰可辨。
- [x] RootHeader 保持 64px，PageHeader 保持 76px，两者 sticky 且 `z-index: 70`。
- [x] Header 图标控件保持 44x44，Lucide SVG 双轴中心偏差不超过 0.5px。
- [x] Header 控件的默认、hover、active、focus-visible、disabled 和 reduced-motion 状态完整。
- [x] 浅色贷款、安全和交易页面的卡片、字段、按钮和分隔结构不再发白或难以辨认。
- [x] 320x720、390x844、448x900 的首页、交易页和重点二级页无横向溢出或 Header 遮挡。
- [x] 更新聚焦视觉契约测试；`npm run type-check`、`npm test`、`npm run build:pwa`、`npm run build:tauri` 和 `git diff --check` 通过。

## Out Of Scope

- 不修改后端接口、DTO、鉴权、交易、贷款、安全或资产业务逻辑。
- 不修改路由结构、底部导航栏目、PWA 缓存策略或 Tauri 配置。
- 不替换官方 Logo 文件，不引入新的 UI 或动画依赖。
- 不修改独立的 `mobile/sites-prototype/`。

## Technical Notes

- 优先在 `prototype-parity.css` 通过生产令牌和目标选择器覆盖，避免改写 8078 行原型快照。
- 复用现有 `RootHeader.vue`、`PageHeader.vue` 与 Header 控件选择器，必要时只增加稳定的语义 class/data 属性。
- 视觉审计记录见 [`research/visual-audit.md`](research/visual-audit.md)。

## Definition Of Done

- 视觉实现和契约测试完成。
- 明暗主题、多尺寸和重点路由浏览器验收通过。
- 移动端质量门与 PWA/Tauri 构建通过。
- 更新移动端规范与 `docs/superpowers/PROGRESS.md`。
