# 后台管理端 UI / UX 审查

## 审查范围

- 线上地址：`https://hipoex.cllbmz.kdns.fr`
- 浏览器：Ego Browser 独立任务空间 `34`
- 桌面视口：`1728 × 1006`
- 响应式抽查：`1280 × 800`
- 页面类型：
  - 登录页、总览仪表盘
  - 通用资源列表页（用户、资产）
  - 专用审核页（KYC）
  - 配置工作台（安全策略）
  - SideSheet 编辑表单（资产修改）
  - 13 个自定义后台路由的批量结构指标

## 关键证据

- `research/screenshots/dashboard-before.png`
- `research/screenshots/users-before.png`
- `research/screenshots/users-1280-before.png`
- `research/screenshots/assets-before.png`
- `research/screenshots/kyc-before.png`
- `research/screenshots/security-policy-before.png`
- `research/screenshots/asset-edit-before.png`

## 发现的问题

### P0：通用表格的固定操作列存在视觉错位风险

- 资产表在横向滚动区域内同时开启 `resizable`、统一固定列宽和右侧固定操作列。
- 1728px 页面中，宽 300px 的固定操作列覆盖在普通列之上，看起来像被插入“支持充值 / 支持提现”中间。
- 横向滚动条直接暴露在数据区下方，固定列边界不清晰。
- Semi Table 官方文档明确提示：不推荐同时使用 `resizable` 与 `scroll.x`；固定列场景需要稳定列宽，并保留可伸缩列或关闭列伸缩。

### P1：资源页工具栏缺少稳定布局

- 1728px 下用户页工具栏已经达到 96px 高；“重置”按钮被挤到第二行。
- 1280px 下三个筛选控件以不规则的居中方式换行，操作按钮、筛选器和空状态缺少统一对齐线。
- 筛选标签被视觉隐藏，只靠 placeholder 表达字段含义，选中后语义弱。
- 空数据时资源卡片只剩一行“暂无数据”，主体形成大量无意义空白。

### P1：后台外壳缺少品牌与层级

- 侧边栏仍显示 `RC / Rust Chain`，与现有 HIPPO 品牌资产不一致。
- 侧边栏占宽约 240–272px，并永久露出原生滚动条，折叠入口位于底部，长菜单浏览成本高。
- 顶栏只有账号和退出按钮，没有当前业务域、环境状态或清晰的层级提示。
- 主内容使用纯白底和相同强度的卡片，页面之间缺少视觉节奏。

### P1：仪表盘信息优先级不足

- 6 张 KPI 卡片在一行平铺，数字、说明、风险状态的权重基本一致。
- 计数型指标显示为 `0.00 / 2.00`，不符合运营后台的阅读习惯。
- 详情卡片全部同一视觉模板，异常状态只依赖 Banner，缺少清晰的状态色和分组语义。

### P1：专用页面仍停留在组件默认样式

- KYC 页面在 1424px 宽卡片内使用一个约 760px 的下拉框和 14 列空表格，审核上下文、待办数量和空状态层级不足。
- 安全策略页顶部使用 Tabs，但四个内容卡片同时显示；导航语义和内容行为不一致。
- 安全策略卡片被强制拉成相近高度，开关与说明文本分离，产生大块空白和错位。
- 资产编辑 SideSheet 内使用三列密集表单；操作按钮混入正文，提交按钮呈现红色文字但不是危险动作。

### P2：响应式策略不足

- 1280px 时侧边栏仍占约 257px，主内容只剩约 1023px。
- 当前仅在 840px 以下调整工具栏，常见的 1024–1440px 管理后台宽度没有中间态。
- 页面没有文档级横向溢出，但筛选器、表格和固定列通过局部拥挤规避溢出，阅读体验仍差。

## 批量路由审查结论

已只读遍历总览、KYC、代理、快速充值、竞猜配置、新币动作、行情策略动作、行情订阅、安全策略、两步验证、品牌配置、SMTP、上传配置。13 个页面均未出现文档级横向溢出，说明问题主要集中在：

1. 公共外壳和设计 token；
2. 通用资源页 / 表格 / 筛选器；
3. Card、Tabs、Form、SideSheet 的全局密度与层级；
4. Dashboard、KYC、安全策略三个高曝光页面的专用结构。

## 修复方向

1. 复用 `mobile/src/assets/brand` 与 PWA 图标，建立 HIPPO Operations 品牌外壳。
2. 统一浅色运营后台 token：暖白主画布、石墨侧栏、HIPPO 橙主色、蓝色信息辅助色。
3. 重构侧栏、顶栏、PageHeader，使当前业务域、账号和环境状态清晰可见。
4. 资源页使用稳定的“主操作 + 筛选面板 + 数据区”三段结构；筛选标签常显。
5. 按 Semi 官方约束修复 Table：移除横向滚动与列伸缩冲突，收窄固定操作列，增强固定列边界、空状态和分页区。
6. 统一 Card、Tabs、输入框、按钮、Modal / SideSheet 的圆角、描边、焦点态和空间密度。
7. 专项优化 Dashboard、KYC、安全策略和资产编辑 SideSheet。
8. 增加 1280px 中间断点，并保持 840px 以下可用。

## 非目标

- 不修改 Rust 后端接口、权限、业务规则或数据结构。
- 不改动 PC 用户端和手机端页面。
- 不触发审核、保存、删除、充值、提现等写操作。
- 不引入新的大型 UI 框架。

## 修复后验证

主会话使用本地 Vite（API 指向线上服务）和 Ego Browser 完成复验：

- 1728px Dashboard：HIPPO 外壳、整数 KPI、四块运行状态卡正常，横向溢出为 0。
- 1280px 用户管理：侧栏 208px；三个常显标签筛选字段等宽同排；查询与重置独立成组；横向溢出为 0。
- 1728px 资产管理：固定操作列 216px；表格无 `.react-resizable-handle`；表格局部横向滚动不扩张文档。
- KYC：当前人工审核 Tab 拥有匹配的 `tabpanel`，空状态与审核筛选形成独立工作台。
- 安全策略：只显示当前 Tab 对应的策略卡片，保存栏稳定在工作台底部。
- 资产编辑 SideSheet：宽 720px，表单为两列 307px，提交动作使用普通主色并右对齐。
- 登录页：HIPPO 品牌、生产环境说明和登录卡片形成清晰双栏入口。

修复后截图：

- `research/screenshots/login-after.png`
- `research/screenshots/users-1280-after.png`
- `research/screenshots/assets-after.png`
- `research/screenshots/kyc-after.png`
- `research/screenshots/security-policy-after.png`
- `research/screenshots/asset-edit-after.png`
