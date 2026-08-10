# 手机端资产账单分类与国际化优化

## Goal

重构手机端 `/assets/ledger`，让资金流水可以按真实业务类别完整筛选、按日期清晰分组，并确保所有固定文案和已知后端流水类型都通过 `vue-i18n` 显示，不再把后端枚举直接作为主要界面文案。

## What I Already Know

- 当前页面只有“全部 / 充币 / 交易 / 合约”四个入口，并分别把后三项错误地映射为单个 `change_type`，因此会漏掉同业务下的其他流水。
- `GET /wallet/ledger` 目前只支持精确 `change_type`，无法对一组同类流水做可靠的服务端分页。
- 页面只按时间倒序平铺，没有日期分组，也没有明确展示每条流水所属业务分类。
- 已知类型有少量 i18n 映射；未覆盖类型直接显示英文枚举。
- 现有接口已经返回分页总量、手续费、引用类型等信息，但手机端适配器没有完整消费分页信息。

## Requirements

- 后端 `GET /wallet/ledger` 新增可选 `category` 查询参数，并在服务端分类后分页，保留既有精确 `change_type`、资产、引用和时间筛选兼容性。
- 支持 `funding`、`spot`、`margin`、`seconds`、`convert`、`earn`、`new_coin`、`loan`、`prediction`、`other` 十个业务分类；省略参数表示全部。
- 每条账单响应返回服务端权威 `category`，避免手机端重复猜测分类规则。
- 手机端分类栏展示“全部”与十个业务类别，保持横向滚动、44px 触控目标、选中态和加载禁用态。
- 流水按用户本地日期分组，日期标题显示“今天 / 昨天 / 本地化日期”和该组条数。
- 每条流水展示本地化类型、业务分类、资产、时间、有符号金额、变动后余额；真实手续费大于零时显示手续费。
- 所有固定文案、分类名、日期文案、已知流水类型、错误/空态和辅助文本均使用 `vue-i18n`，中英文 key 对称。
- 未知后端类型使用本地化“其他资金变动”作为主标签，并保留原始枚举作为次要技术信息，避免错误翻译和信息丢失。
- 切换分类、刷新、加载更多时保持服务端分类分页正确；分类切换不混入旧分类响应。

## Acceptance Criteria

- [x] `category=funding|spot|margin|seconds|convert|earn|new_coin|loan|prediction|other` 返回对应类别且分页总数与列表使用相同谓词。
- [x] 不支持的 `category` 返回 400 validation error；既有 `change_type` 精确筛选仍可使用。
- [x] 响应中的每条 entry 都包含稳定分类值，未知类型归入 `other`。
- [x] `/assets/ledger` 可按全部类别筛选，且列表按日期分组、组内按时间倒序。
- [x] 中英文 locale 中不存在页面固定文案缺失，已知流水类型不会回退为原始英文枚举。
- [x] 空态、首次错误、带缓存错误、加载更多和刷新行为继续区分。
- [x] 320px、390px、448px 宽度不产生页面级横向溢出，交互目标不少于 44px。
- [x] Rust 定向测试、Mobile 定向/全量测试、type-check、PWA build、Trellis validate 与 `git diff --check` 通过。

## Definition of Done

- 后端分类契约、查询过滤与响应 DTO 完成并有单元/路由测试。
- 手机端 API 适配、分类/日期核心逻辑、页面与双语资源完成并有回归测试。
- 最贴近改动的 Rust 与 Mobile 质量门通过。
- `.trellis/spec/` 与 `docs/superpowers/PROGRESS.md` 更新。

## Technical Approach

后端在钱包模块集中定义分类白名单与 `change_type -> category` 规则，查询层对行查询和 COUNT 查询复用同一分类谓词；响应直接携带分类。手机端 API 返回分页对象，页面用服务端分类筛选，以独立核心模块维护分类顺序、i18n key、已知类型标签及本地日期分组，视图只负责交互状态和渲染。

## Decision (ADR-lite)

**Context**: 仅在当前 30 条数据上做前端分类会破坏分页正确性；用单个 `change_type` 代表一类业务会持续漏数据。

**Decision**: 分类属于后端账单查询契约，手机端消费权威 category；展示层另按日期分组。业务分类保持细分，现货、合约和秒合约互不合并。

**Consequences**: API 增加向后兼容字段和查询参数；后续新增 `change_type` 时必须同时评估分类与 i18n 标签，未知类型仍安全归入 `other` 并保留原值。

## Out of Scope

- 不修改资金入账、结算、余额计算或数据库账务写入逻辑。
- 不新增账单详情路由、导出功能、资产搜索或时间范围选择器。
- 不改变后台管理员账单页面。
- 不把 `margin_wallet_ledger` 合并进现货钱包流水接口。

## Technical Notes

- 主要文件：`src/modules/wallet/{presentation,application,infrastructure,routes}.rs`、`mobile/src/{api/wallet.ts,views/WalletLedgerView.vue,i18n/messages/*}`。
- 既有页面 Pencil 来源标识和登录/错误/空态分支需要保留。
- 未知枚举必须可见，符合移动端 localization contract。

## Research References

- [`research/ledger-taxonomy.md`](research/ledger-taxonomy.md) — 当前代码中真实账务类型、分类边界与分页约束审计。
