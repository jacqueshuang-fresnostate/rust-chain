# 优化手机端杠杆按钮并校验保证金上限

## Goal

优化 `/trade/:symbol?mode=contract` 杠杆下单区的按钮材质、层级、触控尺寸和交互状态，同时让手机端完整消费后端杠杆产品的 `min_margin` / `max_margin` 约束，在打开确认层和发送请求前拦截超限保证金，避免用户直接看到英文 `validation error: margin amount exceeds product maximum`。

## What I already know

* 后端 `/api/v1/margin/products` 的 `MarginProductResponse` 已包含 `min_margin` 和可选 `max_margin`，开仓用例会事务内再次强校验这两个边界。
* `mobile/src/api/trading.ts` 目前只映射 `min_margin`，丢弃了 `max_margin`；`MarginProduct` 也没有最大保证金字段。
* 杠杆百分比快捷按钮当前直接按整个杠杆钱包可用余额计算，未把产品最大保证金视为可用上限。
* 当前 0/25/50/75/100% 按钮实际高度仅 14px，不满足项目的 44px 触控合同；手动编辑保证金后也不会清除已选百分比。
* 本任务建立在已完成的专属杠杆确认面板之上，不回滚或重写上一任务的实现。

## Assumptions

* “杠杆页面按钮”指当前下单工作台中的开/平仓、保证金模式、杠杆倍数、BBO、百分比、可用资产和做多/做空主操作，不改动其他业务页。
* 优化延续当前 Pencil 的纯黑/明亮中性画布、薄荷绿与负向红语义，不引入新图标库或改变交易页两栏信息架构。
* 产品未配置 `max_margin` 时表示不设产品上限，仍以用户真实杠杆钱包可用额为快捷按钮基数。

## Requirements

* 手机端产品 DTO 必须解析 `max_margin`，无配置、空值或非法非正值统一映射为 `null`，不伪造零上限。
* 百分比快捷额以 `min(杠杆钱包可用额, 产品最大保证金)` 为可用基数；无最大值时仍按钱包可用额计算。
* 手动修改保证金时清除百分比选中态；“最大”快捷操作使用同一权威计算。
* 保证金字段展示当前产品的最小/最大范围；输入越界时字段进入可访问错误态，并显示中文/英文本地化原因。
* 打开确认层前和最终调用 `placeMarginOrder` 前均必须使用同一校验结果；超限时不调用下单接口。
* 如果产品在页面加载后被后台改配，后端仍返回最小/最大保证金错误时，手机端将已知错误翻译为本地化文案，保留确认层和原地重试。
* 按钮在不改变真实路由和下单行为的前提下，统一表面层级、可见焦点、按压反馈、禁用态和减少动态效果；快捷百分比和主要操作必须有至少 44px 触控目标。
* 仅使用已安装的 Lucide 图标，不使用 Emoji；明暗主题及 320–448px 不产生横向溢出。

## Acceptance Criteria

* [x] `MarginProduct` 保留后端 `max_margin`，并有可执行测试覆盖有上限、无上限和非法值。
* [x] 杠杆百分比按产品可用上限计算，不会因钱包余额大于 `max_margin` 而产生超额请求。
* [x] 低于 `min_margin` 或高于 `max_margin` 的输入在确认前被拦截，字段与错误文案给出明确边界。
* [x] 后端已知的 minimum/maximum 竞态错误不再将英文 validation message 直接显示给中文用户。
* [x] 百分比、设置、BBO、资产与做多/做空操作具有一致的正常、选中、焦点、按压、禁用和减少动态效果状态。
* [x] 320×720、390×844、448×900 明暗主题下无横向溢出，主下单按钮和保证金错误均可见。
* [x] Mobile type-check、聚焦测试、全量测试、PWA/Tauri 构建与 `git diff --check` 通过。

## Definition of Done

* 只修改手机端杠杆产品适配、下单约束、对应按钮/UI、i18n、测试和必要规范。
* 使用 Ego Browser 验证真实 DOM 尺寸、主题、聚焦/按压状态与字段错误。
* 更新 `docs/superpowers/PROGRESS.md`。

## Out of Scope

* 不修改后端杠杆产品边界或撤销后端事务内的强校验。
* 不改变杠杆钱包余额、借款、强平、利息或幂等请求逻辑。
* 不重设现货、秒合约、订单中心或杠杆底部设置弹层。

## Technical Notes

* 主要代码：`mobile/src/{api/trading.ts,core/types.ts,core/tradeForm.ts,core/marginOrderConfirmation.ts,views/TradeView.vue}`。
* 后端权威合同：`src/modules/margin/{presentation.rs,application/open_position.rs}`。
* 现有确认层任务：`.trellis/tasks/08-19-margin-order-confirm-dialog/`。
* 代码与浏览器审计见 `research/current-gap-audit.md`。
