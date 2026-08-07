# 重构手机端资产页并补充邀请好友入口

## Goal

以当前 Pencil 资产页画板为唯一视觉来源，重构生产端手机资产页的访客、已登录空持仓和已登录有持仓状态，并在“我的”页补充可进入现有邀请好友页面的入口。

## What I already know

- Pencil 资产页来源为访客画板 `CUK3y` / `i6YDBr` 与会员画板 `p61z2Q` / `Q4JYj`。
- 访客态只展示沉浸式登录卡，不展示遮罩金额、资产操作、持仓或资金工具。
- 会员态将总资产、今日收益占位和充币/提币/划转/账单四个操作合并到丝绸质感 Hero 中。
- 会员态在 Hero 下展示真实持仓；无真实持仓时展示设计稿空态与“去充币”操作。
- `ReferralsView` 与命名路由 `referrals` 已存在，当前缺口只是 `ProfileView` 没有入口。
- 所有资产、行情、划转和邀请数据继续来自现有 API/store；不使用 Pencil 演示数值。

## Requirements

- 资产页访客态按 `CUK3y` / `i6YDBr` 重建：标题、沉浸式登录卡、明暗主题素材和登录跳转。
- 资产页会员态按 `p61z2Q` / `Q4JYj` 重建：总资产、可见性切换、今日收益真实缺省状态、四个资产操作、持仓列表或空态、资金工具。
- 资产 Hero 使用复制到 `mobile/src/assets/` 的生产素材，不从 `mobile/pencil/` 运行时加载。
- 持仓按估值降序展示真实现货与杠杆钱包合并结果，并显示币种、余额、估值及可用/冻结摘要；未知行情不伪造估值。
- 保留现有钱包加载、行情刷新、资金划转弹层、登录保护和命名路由行为。
- 在“我的”页增加 Lucide 图标的“邀请好友”入口，进入现有 `referrals` 路由；访客点击后由邀请页现有登录态处理。
- 更新与新页面结构冲突的源代码合同测试。

## Acceptance Criteria

- [ ] 资产页声明四个 Pencil 画板来源，访客与会员 DOM 分支明确且互不泄漏。
- [ ] 会员 Hero 在 320–448px 宽度无横向溢出，四个操作保持至少 44px 触控目标。
- [ ] 明暗主题分别使用本地浅色/深色丝绸素材；主题切换不需要重新请求素材。
- [ ] 加载、错误、空持仓和有持仓状态均不展示伪造资产或收益数据。
- [ ] 充币、提币、划转、账单、提币记录、快捷充值以及划转确认层行为保留。
- [ ] “我的”页出现“邀请好友”入口并跳转到命名路由 `referrals`。
- [ ] TypeScript 类型检查、移动端测试和 PWA 构建通过。

## Definition of Done

- 生产代码、双语文案、素材和回归测试完成。
- `docs/superpowers/PROGRESS.md` 记录本切片。
- `npm --prefix mobile run type-check`、`npm --prefix mobile test`、`npm --prefix mobile run build:pwa` 通过。

## Technical Approach

- 保留 `AssetsView` 现有数据加载和资金划转逻辑，重组计算属性与模板，避免 API 行为漂移。
- 将 Pencil 使用的两张 JPG 内容素材复制到 `mobile/src/assets/assets/`，通过静态 import 渲染双主题 Hero。
- 以 `session.isAuthenticated` 作为访客/会员结构分支；账户请求状态只控制会员内部加载、错误、空态和持仓态。
- 复用 `AssetMark`、`PageHeader`、`AppBottomNav` 与现有设计 token，不引入新依赖。

## Decision (ADR-lite)

**Context**: 旧资产页只做了早期选中稿的摘要/分布布局，与当前 Pencil 沉浸式资产画板不一致。

**Decision**: 直接按当前四张资产画板重组生产模板，同时保留真实 API 与可访问性合同；今日收益在后端缺少对应字段时显示 `--`。

**Consequences**: 页面视觉结构与设计稿一致，且不会用总资产变化或演示值冒充今日收益；后端未来提供收益字段后可直接替换该缺省值。

## Out of Scope

- 不修改邀请好友二级页和后端邀请接口。
- 不新增今日收益后端接口。
- 不修改底部导航或其他根页面。

## Technical Notes

- 相关规范：`.trellis/spec/mobile/index.md`、`.trellis/spec/mobile/pwa-and-shell.md`、`.trellis/spec/mobile/navigation-and-localization.md`。
- 相关生产文件：`mobile/src/views/AssetsView.vue`、`mobile/src/views/ProfileView.vue`、双语资源与移动端源代码合同测试。
- Pencil 研究记录：`research/current-pencil-assets.md`。
