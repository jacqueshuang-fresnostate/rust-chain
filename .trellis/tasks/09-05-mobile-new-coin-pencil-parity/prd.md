# 手机新币页面 Pencil 1:1 复刻

## Goal

以 Pencil 当前选中的八张 390x920 明暗主题画板为唯一视觉真值，重新实现手机端完整新币流程：打新专区、新币详情、打新记录与交易先机。生产页面必须保留现有真实项目、钱包、行情、认购、上市后购买、解禁费和释放业务，并在 320–448px 手机宽度、PWA 与 Tauri 容器内保持可用。

## What I already know

- 当前 Pencil 选择明确覆盖四个业务状态及其明暗主题：
  - `oOJ0q` / `ZTtvY`：打新专区。
  - `nFwYy` / `B6Qh9J`：打新详情。
  - `A9It6g` / `h4gfd`：打新记录。
  - `XG67j` / `E2qzxN`：交易先机。
- 现有生产路由为 `/products/new-coins`、`/products/new-coins/:symbol`、`/products/new-coins/records`，已具备安全返回 fallback 和隐藏底部导航的路由元数据。
- 现有三张 Vue 页面已连接真实接口，但仍是上一版平铺设计：专区缺少选中横幅、一级/五项状态栏和交易先机；详情缺少选中主视觉、四阶段、规则卡与底部申购面板；记录仍按四种 API 类型分 Tab，而非选中设计的状态胶囊与 168px 卡片。
- 用户端新币接口已经返回发行价、供给计数、生命周期、上市/解锁配置与交易对 ID，但 Mobile 尚未映射 `quote_asset_id`、`reserved_supply`、`allocated_supply`、`remaining_supply`。
- `assets` 表已拥有后台配置的 `name` 与 `logo_url`，公开新币项目查询尚未联表输出；为让未登录页面遵守“后台 Logo 权威”合同，需要在不改 schema 的前提下扩充公开项目 DTO。
- 交易先机可以复用共享 `useMarketStore()` 的 REST 冷启动和 WebSocket 租约，不能另建轮询或每次 Tab 切换重复请求。
- Pencil 的 28px 状态栏属于原生系统 Chrome，不在 Web 文档重复渲染；业务内容从页面 Header 开始按选中画板相对几何复刻。

## Assumptions

- “手机端的新币页面”包含当前选中的四组页面/状态，而不是只修改列表页。
- 视觉 1:1 指 390px 下业务 Header 之后的尺寸、间距、排版、颜色、圆角、边框和层级一致；动态字段使用真实数据，因此文本长度与样本值可以不同。
- 不新增后台配置表或伪造 APR、项目简介、申购结束时间等当前模型不存在的数据。缺少的业务值显示 `--` 或使用已有准确字段重命名展示，不把 Pencil 样本写入生产。
- 新币项目公开 DTO 仅补充关联资产/计价资产已有元数据，不改变认购、购买、结算、解禁与供给会计逻辑。

## Requirements

### 1. Pencil visual parity

- 在页面根节点声明全部对应明暗画板 ID。
- 把选中横幅位图复制到 `mobile/src/assets/` 的跟踪资源中，生产代码不得依赖 `mobile/pencil/`。
- 390px 下复刻研究文档中的 Header、横幅、一级 Tab、筛选、卡片、详情阶段/规则/申购面板和记录卡片几何。
- 明暗主题必须只改变选中画板定义的调色板，不改变结构几何。
- 图标只使用 Lucide；不得使用表情符号、内联 SVG 或外部图片服务。

### 2. New Coin Zone and Trading Opportunities

- 一级 Tab 为“新币活动 / 交易先机”，切换只改变本地展示状态。
- 新币活动筛选为全部、预热中、申购中、待上市、已上市；按后端生命周期筛选。
- 项目卡显示后台 Logo/名称、真实生命周期、发行/供给/进度、计价资产、解锁或上市时间，并跳转详情。
- 项目卡 Logo 必须直接使用 `asset_id` 关联资产的 `logo_url`；发行价金额与计价资产符号分槽排版，金额过长时只允许金额槽收缩，计价资产符号必须保持可见。
- 交易先机复用共享行情 Store，展示有真实后上市交易对的项目及最新价、24h 涨跌、24h 成交量；筛选为全部、即将上线、今日上线、热门涨幅。
- “去交易”跳转真实现货交易对。没有真实交易对/行情时不生成样本卡，缺失值显示 `--`。
- 页面卸载释放自己持有的共享行情租约；切换一级 Tab 不重复初始化数据。

### 3. New Coin Detail

- 映射并使用项目 `quoteAssetId`、项目/计价资产符号与 Logo、供给计数；申购资产不能从用户钱包任意猜测。
- 复刻 210px 项目主视觉、112px 四阶段、104px 规则区和 328px 申购面板。
- 百分比为 25/50/75/全部，并按真实可用余额与现有精确 Decimal 工具计算；选中态与 Pencil 一致。
- 保留认购和上市后购买两条原有 mutation，保留精确十进制、余额校验、登录重定向、复核弹窗、焦点闭环、Escape/遮罩关闭和提交态。
- 分享继续使用 Web Share/剪贴板回退。

### 4. New Coin Records

- 页面顶部显示全部、进行中、待结算、已完成四个状态筛选。
- 把订阅、派发、上市后购买、解禁四个权威接口合成为一个按时间倒序的统一展示模型；状态筛选不触发重复请求。
- 每条记录使用 358x168 选中卡片结构，显示项目/资产 Logo、名称、时间、状态、主要数量、支付金额、稳定记录号和上下文操作。
- 解禁记录仍可支付手续费或释放，且保留现有鉴权、余额校验、焦点闭环和错误反馈。
- Header 筛选按钮提供真实记录类型过滤入口，不得成为无响应装饰。

### 5. Backend/API contract

- 公开项目列表与详情联表读取项目资产 `name/logo_url` 和计价资产 `symbol/logo_url`，保持列表/详情字段完全一致。
- Mobile 严格映射新增可选文本/Logo和已有供给/计价字段；空/空白 Logo 归一化为 `undefined`，非字符串 Logo 作为合同错误，不推导符号图片路径。
- 不新增数据库迁移，不修改订单写入、供给预留/分配、账本或解禁结算算法。

### 6. Responsive, accessibility, localization and states

- 320px、390px、448px 均无水平滚动；可见小控件通过透明命中区达到至少 44x44。
- Header/卡片/输入/按钮具备键盘焦点；状态筛选使用 `aria-pressed`；对话框保留 `role=dialog` 与 `aria-modal=true`。
- 所有新增可见文案在 `zh-CN` 和 `en` 对称定义；Vue 模板不得硬编码中英文可见文案。
- 初次 loading、已有数据后台刷新失败、完全 error、empty、guest、disabled、submitting 状态必须区分并保持主要 Pencil 轨道。

## Acceptance Criteria

- [x] `/products/new-coins` 在 390px 明暗主题分别与 `oOJ0q`/`ZTtvY` 的打新活动状态同构，并可切换到 `XG67j`/`E2qzxN` 交易先机状态。
- [x] `/products/new-coins/:symbol` 在 390px 明暗主题与 `nFwYy`/`B6Qh9J` 同构，所有真实认购/购买 mutation 回归通过。
- [x] `/products/new-coins/records` 在 390px 明暗主题与 `A9It6g`/`h4gfd` 同构，统一状态筛选与四类记录/解禁操作均可用。
- [x] 后端公开新币响应包含可空 `name/logo_url/quote_asset_symbol/quote_asset_logo_url`，现有字段和业务语义不回归。
- [x] 打新专区项目卡把公开响应的资产 `logo_url` 传给 `AssetMark`，发行价后方显示权威 `quote_asset_symbol` 且不会被金额省略规则一起裁掉。
- [x] Mobile 不依赖 `mobile/pencil/`，不包含 Pencil 样本金融值或外部币种图片 URL。
- [x] 320px 与 448px 的三个路由及专区两个一级 Tab 均 `scrollWidth === clientWidth`。
- [x] 明暗主题 Palette、卡片尺寸、主要坐标与设计审计记录一致；运行时无控制台错误。
- [x] Mobile 聚焦测试、全量测试、`type-check`、`type-check:tests`、PWA/Tauri production build、source-size/test-quality 通过。
- [x] Rust formatter、check、Clippy 和新币路由/单元测试通过；有 MySQL 时验证真实联表字段，无 MySQL 时明确记录跳过。
- [x] `docs/superpowers/PROGRESS.md` 记录完整改动与验证。


## Review / Acceptance Evidence

- 最终独立复核见 [`review.md`](review.md)。390px 运行时按 892px Web body（仅扣除参考图 28px 原生状态栏）核对八张导出图；专区第一张项目卡为 `x=16, y=344, 358x300`，对应内容顶距 8px、标题 36px、标题后 12px。
- 部署 HIPPO 数据验证了 `immediate_on_listing -> 上市即释放`、缺失名称的诚实本地化回退，以及 `100,000,000 HIPPO` 在固定卡片轨道内完整可读；320/390/448px 均无水平溢出。
- 交易先机只要求权威 `post_listing_pair_id` 命中共享实时 Store 的真实 ticker，不再受仅用于新币直接购买 mutation 的开关影响；Tab 往返只持有/释放一个消费者租约。
- 访客专区与详情始终停留公开路由，公开请求剥离陈旧 Bearer 且不刷新/清除会话；记录、钱包和四个写动作保持认证边界。记录三种样本状态统一使用导出图的 forest/sage green 系统。
- Mobile `release:gate` 最终通过（672/672）；Rust formatter/check/Clippy、新币单元测试及一次性 MySQL 的公开列表/详情权威资产元数据聚焦测试通过。

## Definition of Done

- Production implementation, API adapters and tests are complete.
- Focused source/behavior contracts and runtime visual checks support every acceptance criterion.
- Relevant Mobile/backend specs are updated with the new selected-frame and API contracts.
- No unrelated dirty files are modified or reverted.

## Out of Scope

- Editing the Pencil canvas or replacing the selected design.
- Adding new admin fields, tables, migrations, APR calculation, project marketing CMS or subscription-window scheduler.
- Changing new-coin issuance accounting, distribution, unlock or wallet settlement semantics.
- Redesigning unrelated Mobile/Admin/PC routes.
- Committing or pushing before a separate explicit user request.

## Research References

- [`research/pencil-selected-new-coin-frames.md`](research/pencil-selected-new-coin-frames.md) — exact selected geometry, palette, content hierarchy and interaction mapping.
- [`research/current-new-coin-audit.md`](research/current-new-coin-audit.md) — current implementation/API/test audit (generated by Trellis research agent).
- [`research/reference/`](research/reference/) — 1x PNG exports of all eight selected Pencil frames for runtime comparison.

## Technical Notes

- Likely Mobile files: `mobile/src/views/NewCoinsView.vue`, `NewCoinDetailView.vue`, `NewCoinRecordsView.vue`, `mobile/src/api/newCoin.ts`, shared presentation helpers/components, locale files, selected-page CSS, router tests and new focused parity tests.
- Likely backend files: `src/modules/new_coin/{repository,infrastructure,presentation}.rs` plus `tests/new_coin_routes.rs` and unit/source contracts.
- Reuse `useMarketStore()` consumer lease rather than directly calling `fetchMarketTickers()` in the new-zone surface.
- Reuse `AssetMark` and its API-image/fallback contract. Page-local CSS may size or position it but must not decorate a successfully loaded image.
