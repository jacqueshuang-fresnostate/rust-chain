# Pencil 杠杆持仓操作对照

## 当前选中稿

- Pencil 文件：`mobile/pencil/hippo-mobile-uiux.pen`。
- 当前选中：`sBAXi`、`wFT1U`，节点名均为“参考版持仓详情”。
- 所属主画板：`cjzfi`（浅色）与 `p6GfgT`（深色）。
- 选中稿持仓操作组：
  1. `止盈止损按钮`
  2. `平仓按钮`
  3. `市价全平按钮`
- 持仓区上方另有“当前交易品种”“一键平仓”和筛选入口。

## 生产端差异

- `mobile/src/views/TradeView.vue` 的 `positions` 页签使用 `trade.positionAssetsTab`，中文值为“资产”，没有明确表达其内容为杠杆持仓。
- 每张 `contract-position-card` 只有一个 `contract-position-action` 按钮。
- 页面已持有 `closeMarginPosition` 与 `closeAllMarginPositions(productId)`，无需新增重复 API。卡片内“市价全平”是单仓动作；顶部“一键平仓”才是批量动作。

## 后端能力边界

- `.trellis/spec/backend/margin-trading-actions.md` 规定：
  - `bulk_close=true`
  - `position_risk=true`
  - `take_profit_stop_loss=false`
  - `strategy_orders=false`
- 因此止盈止损只能按能力字段呈现受限状态；单仓关闭与当前交易对批量关闭可以直接接入真实接口。

## 对齐结论

- 页签改为“持仓 (N)”。
- 卡片保留三枚独立按钮及 10px 间距；把已实现能力映射到对应真实操作，未实现能力禁止发起请求。
- 卡内“平仓”和“市价全平”都只关闭当前 `position.id`，但保留独立的二次确认意图。
- 顶部批量操作按 `currentPairOnly` 决定传入 `selectedProduct.id` 或不传产品 ID，保留原作用域。
