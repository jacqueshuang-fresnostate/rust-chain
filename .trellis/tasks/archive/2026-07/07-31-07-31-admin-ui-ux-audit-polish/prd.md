# 审查并优化后台管理端页面设计

## Goal

以线上真实后台为基准，对 `web/` 管理端进行系统化 UI / UX 重构。修复公共外壳、资源页、表格、筛选器、表单、弹窗和高曝光专用页面的层级、密度、品牌一致性与响应式问题，同时保持现有接口、权限和业务行为不变。

## What I already know

- 后台前端位于 `web/`，技术栈为 React 19、React Router 7、Semi Design 2.99。
- 线上后台由 `https://hipoex.cllbmz.kdns.fr` 提供，已有可用管理员会话。
- 用户要求使用 Ego 浏览器主动发现问题并完成修复。
- 项目已有 HIPPO 品牌图片，位于 `mobile/src/assets/brand` 与 `mobile/public/pwa`。
- 多数后台列表共用 `AdminResourcePage`、`FilterBar`、`DataTable`、`DetailDrawer`，适合用公共重构覆盖大量二级页面。
- 现有未提交的 `pc/src/config/app.ts` 属于任务外改动，本任务不得覆盖或夹带。

## Assumptions

- 后台以运营效率、信息密度和稳定可读性优先，不照搬手机端的实验性动效。
- 视觉方向采用 HIPPO 品牌的石墨色、金属质感与橙色高光，但主内容保持明亮、专业。
- 现有 Semi Design 组件继续保留，通过结构调整和项目级 token / CSS 统一，而不是更换组件库。

## Requirements

### R1：品牌化后台外壳

- 使用现有 HIPPO 品牌资产替换 `RC / Rust Chain` 占位品牌。
- 侧边栏、顶栏和内容画布必须形成清晰层级。
- 顶栏展示当前业务域、管理员身份和生产环境标识。
- 顶栏在内容滚动时保持稳定，不被页面内容遮挡。
- 长侧边栏滚动条需要弱化；1280px 中间视口需要自动收窄或折叠。

### R2：统一页面标题与内容容器

- `PageHeader` 的标题、描述、操作区必须在 1728px 与 1280px 下稳定对齐。
- 页面最大宽度、边距和背景装饰统一，不再出现纯白无限延伸的空画布。
- Card、Tabs、Banner、输入框、按钮、开关具有统一圆角、描边、阴影、焦点态与状态色。

### R3：资源页与筛选器

- 工具栏重构为稳定的主操作区和筛选区。
- 筛选字段标签常显，placeholder 只作为补充。
- 1728px 与 1280px 下查询/重置按钮不出现孤立换行。
- 空状态需要有明确容器和说明，避免页面看起来未完成。
- 列表密度切换文案必须表达“将切换到的模式”，不能把当前模式误当动作。

### R4：表格

- 移除 Semi Table 中 `resizable` 与 `scroll.x` 的冲突。
- 收窄固定操作列并增强边界，操作列不得看起来插入业务列中间。
- 优化表头、行高、Hover、状态标签、横向滚动条和分页区。
- 继续支持服务端分页、行选择、CSV 导出和现有 row actions。

### R5：弹窗、SideSheet 与表单

- Detail SideSheet 使用受控最大宽度，不占据 80% 超宽画布。
- 全局 SideSheet / Modal 使用清晰的头部、内容滚动区和操作层级。
- 表单在常见桌面宽度下使用合理的两列或自适应布局，避免三列过密。
- 普通提交动作不得使用危险色误导。

### R6：高曝光页面专项优化

- 登录页改为 HIPPO Operations 品牌入口，并加强视觉焦点与登录卡片层级。
- Dashboard 的计数指标不显示无意义小数；KPI 与异常状态具有清晰视觉分组。
- KYC 页面优化审核状态工具栏、空状态和表格容器。
- 安全策略页的 Tabs 行为与内容展示一致，开关、说明和保存区对齐。

### R7：行为兼容

- 不修改 API 路径、请求参数、权限和写操作逻辑。
- 登录、导航、刷新、筛选、分页、密度切换、打开详情/编辑面板必须保持可用。
- 不触碰 `pc/src/config/app.ts`。

## Acceptance Criteria

- [x] 登录、后台外壳、Dashboard、资源页、KYC、安全策略和 SideSheet 呈现统一 HIPPO 运营后台视觉。
- [x] 1728px 与 1280px 下页面文档级横向溢出为 0。
- [x] 1280px 下侧栏不再挤占约 257px，筛选器排列规整。
- [x] 资产表固定操作列宽度合理、边界清晰，不再表现为插入业务列中间。
- [x] 用户空列表、KYC 空列表呈现完成度足够的空状态。
- [x] 安全策略的 Tabs 与当前可见内容一致。
- [x] 资产编辑 SideSheet 表单密度合理，提交动作视觉语义正确。
- [x] `npm --prefix web run typecheck` 通过。
- [x] `npm --prefix web run lint` 通过。
- [x] `npm --prefix web run test` 通过。
- [x] `npm --prefix web run build` 通过。
- [x] Ego Browser 对本地修复版本完成登录、Dashboard、用户、资产、KYC、安全策略、资产 SideSheet 的回归截图与交互检查。

## Definition of Done

- 代码、样式、相关测试和任务进度记录均已更新。
- 浏览器回归与 Web 包质量命令全部完成。
- 任务外改动保持隔离。
- 变更已提交，任务已按 Trellis 流程归档。

> 本轮按用户要求不创建 Git 提交、不归档任务，提交与归档由主会话后续处理。

## Out of Scope

- Rust 后端业务、数据库、Docker 或部署配置。
- PC 用户端和手机端 UI。
- 新增后台业务功能或修改权限模型。
- 暗色主题、多语言和全量可视化图表系统。

## Technical Notes

- 审查报告：`research/admin-ui-audit.md`
- Semi Table 文档提示：固定列需要稳定列宽；不推荐 `resizable` 与 `scroll.x` 同时使用。
- 优先修改公共组件和 token，再对 Dashboard、KYC、安全策略做小范围专用结构优化。
