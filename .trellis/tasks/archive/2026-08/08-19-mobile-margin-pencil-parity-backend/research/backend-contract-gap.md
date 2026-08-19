# 杠杆页面后端契约差距

## 已存在能力

- 产品列表已返回交易对、精度、结算资产、Logo、保证金模式、杠杆档位、保证金上下限、维持保证金率和小时利率。
- 钱包聚合已返回杠杆余额、opened 仓位及全仓账户风险快照。
- 单仓风险接口已复用强平 worker 的权威风险公式。
- 已有单笔/批量平仓、单笔/批量撤单、保证金模式和杠杆设置接口。
- 市价/限价下单及行情触发成交已在上一任务闭环。

## 本次需要补齐

1. **公开产品目录**：产品配置不依赖用户身份，`GET /api/v1/margin/products` 应允许访客读取，使未登录交易页也能正确展示产品、Logo、精度和能力；资金与设置接口继续鉴权。
2. **显式页面能力**：在 `capabilities` 中增加止盈止损、策略、一键平仓、风险快照等布尔能力，客户端只显示后端真实支持状态。
3. **风险展示字段**：单仓风险响应补充：
   - `position_quantity`
   - `unrealized_pnl`（保留旧 `realized_pnl` 兼容字段）
   - `return_rate`
   - `margin_ratio`
   - `estimated_liquidation_price`
   - `liquidation_distance_rate`
4. **DTO 完整映射**：手机端不得丢弃产品 Logo、结算资产、维持保证金率、小时利率、仓位结算资产和全仓账户风险数据。

## 计算约定

- `position_quantity = notional_amount / entry_price`
- `return_rate = unrealized_pnl / margin_amount`
- `margin_ratio = equity / maintenance_margin`；维持保证金为 0 时返回空。
- 逐仓预估强平价：
  - 多：`entry * (1 + (maintenance - margin + interest) / notional)`
  - 空：`entry * (1 - (maintenance - margin + interest) / notional)`
- `liquidation_distance_rate = abs(mark - liquidation) / mark`
- 非法、非正值或全仓模式返回 `null`，不编造展示值。
- 所有比率沿用小数比例传输，客户端负责格式化百分比。

## 安全与兼容

- 风险接口仍要求用户 JWT，并继续验证仓位归属。
- 产品目录公开不改变任何写入口或用户资产读取权限。
- 新字段只做向后兼容扩展；保留已有 JSON 字段。
- 后端返回 DECIMAL 字符串，手机端以字符串接收，避免金融值经 JSON number 丢精度。
