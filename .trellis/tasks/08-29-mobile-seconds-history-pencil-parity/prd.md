# 手机端秒合约历史订单页对齐 Pencil 选稿

## Goal

以 Pencil 当前选中的浅色画板 `vZy6U` 与对应深色画板 `x29z7` 为唯一视觉基线，1:1 重构生产路由 `/seconds/history`，同时保留真实秒合约历史接口、鉴权、错误恢复、路由返回、国际化与可访问性行为。

## What I already know

- 选稿基准尺寸为 390×920；标题与筛选使用 16px 左右安全轨，订单卡片按用户最新修订使用 390px 视口全宽。
- 可见结构只有 52px 标题栏、38px 方向筛选、142px 历史订单卡列表与底部自然留白；不显示旧版 PageHeader 的 eyebrow、subtitle、刷新按钮或双列详情网格。
- 标题栏左侧为返回操作，右侧标题为“秒合约订单”；筛选项为“全部 / 买涨 / 买跌”。
- 订单卡头部显示“交易对 · 秒数”和带符号的真实盈亏；第二行依次显示方向、状态、时间；第三行显示投入、买入价与结算价。
- 当前浅色画布/卡片统一为 `#FFFFFF`，深色画布/卡片统一为 `#000000`；两套文字、筛选和涨跌颜色按 Pencil 精确映射。
- 当前页面已具备真实 API 请求生命周期、访客/加载/错误/列表/空态互斥、非活动订单筛选、真实结算价和净盈亏展示。

## Requirements

- 页面根节点声明 `data-pencil-source="vZy6U x29z7"`，生产结构与两张画板的可见层级一致。
- 使用 390px 画板的 16px 标题/筛选轨道、52px 标题栏、38px 筛选栏、14px 节奏与 142px 卡片作为精确基线。
- 历史卡片在 390px 基准下必须为 `x=0 / width=390 / height=142`，无圆角、边框或阴影；内容使用上下 14px、左右 16px 内边距，与页面文字轨对齐。
- 新增真实方向筛选状态：全部、买涨、买跌；筛选只作用于已加载的真实历史订单，不触发伪造数据。
- 左侧返回操作通过现有安全返回逻辑回到 `/seconds`，并保持至少 44×44px 触控目标、24px Lucide ArrowLeft 图标和可访问名称；标题贴齐标题栏右轨。
- 订单卡只消费 API 字段；盈亏继续由已固化本金、赔率和最终结果计算，缺失权威结果或价格时显示 `--`。
- 卡片时间按选稿的短日期时间格式显示；长英文、长交易对、大金额和 320–448px 视口不得横向溢出。
- 保留访客、加载、失败重试、空历史状态，并让这些状态使用同一画布和内容轨道。
- 固定文案全部进入中英文资源；不在模板中写死中文。
- 不修改秒合约后端接口、交易工作台、结算逻辑或其他页面。

## Acceptance Criteria

- [x] `/seconds/history` 的浅色与深色首屏在 390px 下匹配 `vZy6U` / `x29z7` 的结构、尺寸、间距、字体层级和颜色；卡片必须为 x=0 的 390px 全宽直角平面。
- [x] 标题栏、三个筛选项和历史卡片的 DOM 顺序与可见设计一致，不再渲染旧 PageHeader/刷新按钮/双列详情布局。
- [x] 全部、买涨、买跌筛选可切换，具有 `aria-pressed`，且空筛选结果显示本地化空态。
- [x] 赢、输、取消/未知结果分别显示正值、负值和 `--`；真实状态、方向、投入、入场价、结算价、期限与创建时间均可见。
- [x] 访客、加载、错误、列表和空态互斥；重试与退出登录生命周期保持正确。
- [x] 320px、390px、448px 明暗主题无横向溢出；卡片填满当前手机画布，其内容轨仍保持 16px 安全边距，交互目标不小于 44px，reduced-motion 可用。
- [x] 聚焦回归、Mobile 全量测试、类型检查、PWA/Tauri 构建及浏览器像素核对通过。

## Technical Approach

- 保留 `SecondsHistoryView.vue` 的数据请求和展示 helper，重组 script 中的方向筛选与短时间格式化，替换模板和 scoped 布局。
- 在 `pencil-selected-pages.css` 中为该页面定义浅深主题的 Pencil 精确令牌，避免 scoped `html[data-theme]` 选择器失效。
- 以源代码合同测试锁定 Pencil ID、可见层级、几何、筛选行为、API-only 数据边界和响应式安全规则。
- 使用 Ego Browser 在真实 Vue 页面中对照 Pencil 导出的 PNG，覆盖 390px 明暗主题及 320/448px 收缩。

## Decision (ADR-lite)

**Context**: 现有历史页采用通用 PageHeader 与双列定义列表，功能完整但与当前选中的 07d 画板没有视觉对应关系。

**Decision**: 保留现有数据域和请求生命周期，只替换页面信息架构与视觉层；Pencil 中被后续可见节点覆盖的旧“进行中/历史订单”和“近 7 天”层不进入生产实现，以最终截图与 HTML 导出的可见层为准。

**Consequences**: 页面将与选稿可见结果一致；方向筛选成为真实交互。由于后端目前一次返回最多 100 条订单，筛选仍在客户端对权威快照执行，不改变接口合同。

## Out of Scope

- 不新增日期筛选或分页后端接口。
- 不修改秒合约下单、活动订单、实时行情或结算提示。
- 不改 Pencil 画板本身。
- 不提交或推送用户未要求的 Git 变更。

## Technical Notes

- Pencil 研究：`research/pencil-seconds-history.md`。
- 生产入口：`mobile/src/views/SecondsHistoryView.vue`。
- 主题入口：`mobile/src/styles/pencil-selected-pages.css`。
- 现有合同：`mobile/src/core/secondsOrder.ts`、`mobile/src/api/seconds.ts`、`mobile/tests/seconds-history-view.test.ts`。
