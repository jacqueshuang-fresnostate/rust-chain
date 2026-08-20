# 手机端杠杆持仓入口对齐 Pencil

## Goal

依据 Pencil 当前选中的浅色与深色“参考版持仓详情”画板，补齐手机端杠杆交易页的持仓入口和持仓操作按钮，使用户能明确区分委托与持仓，并通过已有真实接口完成单仓平仓与批量平仓。

## What I already know

- Pencil 当前选中节点为 `sBAXi` 与 `wFT1U`，均命名为“参考版持仓详情”，分别属于 `cjzfi` 与 `p6GfgT` 杠杆交易主画板。
- 选中稿的持仓卡片底部包含“止盈止损 / 平仓 / 市价全平”三段操作，并在卡片上方提供“一键平仓”。
- 生产端 `TradeView.vue` 已有真实持仓列表、单仓平仓和当前交易对批量平仓接口，但持仓页签仍显示为“资产”，卡片底部仅显示一个“平仓”按钮。
- 后端能力集明确返回 `take_profit_stop_loss=false`、`bulk_close=true`、`position_risk=true`；本任务不能伪造未实现的止盈止损业务。
- 用户允许使用 Ego 浏览器进行本地页面调试。

## Requirements

- 将杠杆工作区的 `positions` 页签明确展示为“持仓”，并显示当前可见真实持仓数量。
- 持仓卡片底部按 Pencil 视觉顺序提供“止盈止损 / 平仓 / 市价全平”三段按钮。
- “平仓”继续复用已有单仓平仓接口与二次确认状态。
- 持仓卡内“市价全平”复用已有单仓关闭接口，并拥有与普通“平仓”互不串联的二次确认状态；它不能关闭同交易对的其他持仓。
- 持仓区顶部“一键平仓”保留既有作用域：仅看当前交易对时批量关闭当前产品，关闭筛选后批量关闭全部持仓。
- “止盈止损”在后端未声明能力时保留设计位置但呈现清晰的不可用状态，不发起伪造请求；能力开放后入口可被现有能力字段解锁。
- 浅色、深色、焦点、按压、禁用和保存中状态保持一致，按钮触控高度不低于 42px，整组不产生横向溢出。
- 同步修正中英文 i18n，不在模板中写死用户可见文案。
- 以 Pencil 当前选中稿作为视觉参考，不改写画板中的其他业务状态或弹层。

## Acceptance Criteria

- [x] `/trade/BTC_USDT?mode=contract` 的工作区显示“持仓 (N)”而不是“资产”。
- [x] 有真实持仓时，每张持仓卡片显示三段操作，顺序与 Pencil 一致。
- [x] 点击“平仓”保留现有二次确认并只关闭所选持仓。
- [x] 点击持仓卡内“市价全平”保留独立二次确认并只关闭该张卡对应的持仓。
- [x] 顶部“一键平仓”继续按“当前交易品种”筛选状态决定关闭当前产品或全部持仓。
- [x] 后端未支持止盈止损时，该按钮可见、可理解但不会调用任何交易接口。
- [x] 无持仓、未登录、保存中和批量失败场景仍能正确反馈。
- [x] 手机端明暗主题均复刻 Pencil 三枚独立按钮的 10px 间距、圆角和主题描边，不发生裁切或错位。
- [x] 相关源码合同测试、类型检查和移动端构建通过；Ego 浏览器完成 390px 视口运行时检查。

## Definition of Done

- Tests added/updated for tab semantics, capability gating, single-position close and current-pair bulk close.
- `npm run type-check`、相关 Node 测试和 `npm run build:pwa` 通过。
- Ego 浏览器验证明暗主题或至少当前主题下的布局、交互和横向溢出。
- `docs/superpowers/PROGRESS.md` 记录本次切片和验证结果。

## Decision (ADR-lite)

**Context**: Pencil 视觉稿包含三个持仓操作，但后端只声明单仓市价关闭与批量关闭能力，止盈止损尚未实现。

**Decision**: 以真实能力为边界完成视觉对齐；持仓卡内“平仓”和“市价全平”都使用单仓关闭接口，但保留独立确认意图；顶部“一键平仓”继续使用批量接口，止盈止损只呈现能力受限状态，不新增或伪造交易语义。

**Consequences**: 页面能完整表达设计层级并保持真实交易安全，不会因为卡内“市价全平”误关其他持仓；待后端正式开放止盈止损能力后，可直接复用该入口补充弹层与接口。

## Out of Scope

- 新增杠杆止盈止损订单、部分平仓或限价平仓后端能力。
- 修改杠杆保证金、强平、利息和撮合计算。
- 重构现货、秒合约或独立订单历史页面。

## Technical Notes

- Pencil 文件：`mobile/pencil/hippo-mobile-uiux.pen`；选中节点：`sBAXi`、`wFT1U`；主画板：`cjzfi`、`p6GfgT`。
- 生产实现：`mobile/src/views/TradeView.vue`。
- 业务适配器：`mobile/src/api/trading.ts`；产品能力类型：`mobile/src/core/types.ts`。
- 参考研究：[`research/pencil-position-actions.md`](research/pencil-position-actions.md)。
