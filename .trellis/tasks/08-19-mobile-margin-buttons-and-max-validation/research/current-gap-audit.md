# 手机端杠杆按钮与保证金上限审计

## 根因链路

1. Rust `MarginProductResponse` 已序列化 `min_margin` 和 `max_margin`;
2. `fetchMarginProducts()` 的 `BackendMarginProduct` 与映射结果只保留 `min_margin`;
3. `setQuantity()` 把整个杠杆钱包 `availableBalance` 传入 `quantityForBalancePercentage()`;
4. 当钱包可用额大于产品 `max_margin` 时，25–100% 快捷额可直接超限;
5. `reviewOrder()` 和 `submitOrder()` 只检查正数及行情，未校验产品保证金边界;
6. 后端事务内 `validate_product_margin()` 最终拦截，所以用户看到原始英文 validation message。

## 按钮审计

Ego Browser 与 DOM 尺寸检查确认：

* Header、开/平仓、模式/倍数、BBO 和主下单操作已有 44–46px 触控框，但表面、焦点、按压和禁用态不统一;
* 0/25/50/75/100% 按钮的真实 DOM 高度只有 14px;
* 百分比按钮仅用文字变色表示选中，浅色主题下层级弱;
* 保证金手动编辑后不会重置百分比，选中态与真实金额可能不一致;
* 杠杆按钮需要延续 Pencil 密度，不应把交易台改成营销页式大卡片。

## 收敛方案

* 保留现有两栏交易台和功能顺序;
* 为按钮统一冷中性双层表面、内高光、轻量按压位移和完整焦点环;
* 百分比使用 44px 触控框与较紧凑的内层视觉芯片，不用 14px 文字热区;
* 金融边界由同一纯函数同时驱动字段、确认层和请求守卫;
* 后端仍保留最终强校验，前端仅做提前反馈和已知错误本地化。

## 实现闭环

* Mobile DTO 已保留可选 `max_margin`，快捷额统一取钱包可用额与产品上限的较小值，手动编辑会清除快捷选中态。
* 字段错误、打开确认层与最终请求共用同一边界结果；已知后端竞态错误保持确认层并转为本地化反馈。
* Ego Browser 已在 320×720、390×844、448×900 明暗主题验证 44px 触控、错误可见、无横向溢出、快捷上限封顶、手动清除选中及减少动态按压覆盖。
