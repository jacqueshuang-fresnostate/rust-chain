# 手机端杠杆下拉框与输入框对齐 Pencil 选稿

## Goal

以用户当前在 Pencil 中选中的浅色 `IpirH`“参考版交易模块”及其深色对应节点 `mcfEf` 为唯一视觉基线，调整生产手机端杠杆交易页的保证金模式、杠杆、委托类型、价格和保证金输入排版，使真实 `/trade?mode=contract` 工作台在 390px 基准下与选稿结构一致，同时完整保留现有产品能力、设置弹层、限价校验、BBO、连续比例滑杆和真实下单合同。

## What I already know

- 当前 Pencil 文件为 `/private/tmp/hippo-mobile-uiux-position-tab.pen`，当前选中节点是 `IpirH`；同一主交易页的深色对应节点是 `mcfEf`。
- 选稿把保证金模式与杠杆组成一行两列 `98 / 6 / 98`，高度 38px；委托类型从原三列设置行中拆出为下一行 202×40px 的完整下拉框。
- 价格区域是 202×54px 的两列结构：138px 输入壳、6px 间距、58px 最优价按钮；保证金输入是下一行 202×48px 的双层信息壳。
- 当前生产实现仍使用 32px 三列设置行、56px 价格行和 46px 保证金输入，纵向位置与当前选稿不一致。
- 后端订单载荷仍以 `marginAmount` 为准，不把保证金输入改成基础资产数量，也不新增不存在的结算资产选择接口。
- 工作区已有保证金模式、杠杆和委托类型三个真实 Teleport 弹层；本任务只调整触发器和输入壳的排版，不改其数据来源或提交行为。

## Requirements

### 1. 选稿和主几何

- 杠杆生产根继续声明主画板 `cjzfi/p6GfgT`，新增可追溯的选中模块来源 `IpirH/mcfEf`。
- 390px 基准下交易模块由 460px 调整为选稿 500px，左右轨道保持 `14 + 202 + 10 + 150 + 14`；下单区与盘口区可用高度为 490px。
- 下单区保持 6px 垂直节奏：开平仓 `y0/h30`、设置 `y36/h38`、委托类型 `y80/h40`、价格 `y126/h54`、保证金 `y186/h48`、比例 `y240/h32`。
- 后续真实余额、止盈止损、开多/开空摘要及按钮顺延至选稿轨道，不允许覆盖、裁切或挤压。

### 2. 下拉框

- 保证金模式和杠杆触发器为两列等宽 98px、高 38px、间距 6px，内容左右分布，左侧值 11px/600，右侧 Lucide `ChevronDown` 11px。
- 委托类型触发器独占下一行 202×40px，内容同样左右分布，不再和保证金模式、杠杆挤在三列中。
- 三个触发器继续使用现有真实可用性、忙碌态、`aria-expanded`、`aria-controls`、键盘焦点、精确触发器焦点恢复和弹层逻辑。
- 只有后端真实能力可成为委托类型选项；不得把点击触发器改回循环切换。

### 3. 输入框

- 价格行是 138×54px 价格输入壳与 58×54px BBO 按钮；价格标签 9px，数值 17px/22px，壳体圆角 8px、内边距 `7px 10px`。
- 保证金输入壳为 202×48px，左侧双层标签/数值、右侧真实结算资产；标签 9px，数值 15px/20px，圆角 8px。
- 输入仍使用外壳唯一焦点环，嵌套原生输入的 border、outline 和 box-shadow 为零；错误态必须优先于焦点态且不改变尺寸。
- 市价保持只读且请求不带价格；限价保持可编辑、精度安全、BBO 按方向填充。保证金继续使用真实钱包余额与产品最小/最大值校验。
- Pencil 中数量行的下拉箭头不映射成无功能控件；生产尾部只呈现当前产品真实结算资产，除非后端未来提供资产切换能力。

### 4. 响应式与主题

- 浅色使用选稿表面 `#FFFFFF`、描边 `#CCD5D0`、主文字 `#111714`、次文字 `#68736D`；深色使用 `#0C100E`、`#29342E`、`#F2F7F4`、`#95A19A`。
- 320px 紧凑轨道保持两列设置和独立委托类型，不恢复三列压缩；价格输入与 BBO 按钮可按现有紧凑规则流式缩小但不得横向溢出。
- 390px、320px、448px 明暗主题均需检查真实计算尺寸、输入可写、弹层开关、焦点环、Header/持仓区域不被模块高度变化覆盖。

## Acceptance Criteria

- [ ] 生产模板把委托类型触发器从 `.contract-mode-row` 拆为独立行，并记录 `IpirH/mcfEf`。
- [ ] 390px 下模块 500px、控制台/盘口 490px，关键轨道与 Pencil 误差不超过 1px。
- [ ] 保证金模式与杠杆为 98×38px 两列，委托类型 202×40px，价格 138×54px+BBO 58×54px，保证金 202×48px。
- [ ] 价格与保证金输入保留真实校验、只读/可写状态、BBO、ARIA 错误和外壳唯一焦点环。
- [ ] 三个下拉框继续打开对应真实弹层，关闭不改变值，明确选择才提交。
- [ ] 后续滑杆、余额、TP/SL、摘要、按钮和盘口不重叠或裁切。
- [ ] 320/390/448px 明暗主题无横向溢出，持仓标签页紧跟 500px 模块。
- [ ] 聚焦回归、Mobile 全量测试、类型检查、PWA/Tauri 构建、Trellis validate 和 `git diff --check` 全部通过。
- [ ] Ego Browser 完成输入、下拉弹层和多尺寸几何验收。

## Definition of Done

- 先更新会失败的 Pencil 排版合同测试，再修改生产实现。
- 只修改手机端杠杆控件、对应测试、Mobile 规范和任务/进度记录。
- 不覆盖既有 `mobile/pencil/docs/superpowers/PROGRESS.md` 修改。
- 完成代码复核、测试、构建与真实浏览器验收。

## Technical Approach

1. 保留 `TradeView.vue` 的响应式状态和业务函数，仅重组三个设置触发器的模板层级并更新 scoped 几何。
2. 使用当前已有主题变量，不创建第二套局部主题状态；选稿颜色由现有 `--contract-*` 角色解析。
3. 更新旧的 460px/三列/56px/46px源码合同为 `IpirH/mcfEf` 新规格，并用 Ego Browser 的 computed style 验证全局样式没有重新撑高按钮。
4. 模块增高后同步盘口容器和下游绝对轨道，保持持仓工作区自然下移，不动后端接口。

## Research References

- [`research/pencil-selected-field-spec.md`](research/pencil-selected-field-spec.md) — 当前选中明暗模块的精确节点、几何、颜色和生产映射。
- [`research/pencil-reference/IpirH.png`](research/pencil-reference/IpirH.png) — 浅色选中模块导出图。
- [`research/pencil-reference/mcfEf.png`](research/pencil-reference/mcfEf.png) — 深色对应模块导出图。

## Out of Scope

- 不修改后端杠杆产品、用户设置、钱包、下单或数据库结构。
- 不新增保证金资产切换、数量单位切换或任何伪下拉能力。
- 不重设计杠杆弹层内容、确认下单弹层、持仓卡片或其他交易页面。
- 不修改现货和秒合约工作台。

## Technical Notes

- 主要实现：`mobile/src/views/TradeView.vue`。
- 重点回归：`mobile/tests/{contract-pencil-selected-parity,margin-product-boundaries,margin-order-type-sheet,pencil-trading-product-selected-parity}.test.ts`。
- 适用规范：`.trellis/spec/mobile/{index,pwa-and-shell,backend-integration}.md`。

