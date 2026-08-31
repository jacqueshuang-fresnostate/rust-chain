# Pencil 双方向杠杆弹窗与现有业务差距

## 当前选中画板

- 深色：`NTiiS`，名称 `06b / 合约交易 · 调整杠杆弹窗 · 深色主题`
- 浅色：`CulR4`，名称 `06b / 合约交易 · 调整杠杆弹窗 · 浅色主题`
- 两个根画板均为 390×920；遮罩 390×920；底部面板 `(0,80,390,840)`。
- Pencil MCP 截图确认无布局折叠或裁切。

## 几何与视觉

- 面板：顶部圆角 24；padding `18 20 16 20`；gap 14；底部按钮 `(20,772,350,52)`。
- Header：`(20,18,350,34)`；标题 22/700；关闭 34×34。
- 双方向 section：label 16/500；调节行 350×64、水平 padding 28；加减 42×42/r10；主数值 52/600，单位 22/500。
- 快捷轨：350×46/r23，1px 边，padding 4、gap 2；六个等宽 38px pill + 32×38 chevron。当前项浅色为黑底白字，深色为白底黑字。
- 做多信息卡 350×103（三行），做空信息卡 350×75（两行），r14、padding14、gap9。
- 深色：page `#0B0F0D`、field `#181E1A`、step `#202723`、line `#364039`、text `#F5F7F6`、muted `#87918C`、long `#14C982`、short `#FF3E73`、submit `#16A765`。
- 浅色：page `#FFFFFF`、field/step `#F0F2F1`、line `#D8DEDA`、text `#111512`、muted `#8B928E`、long `#14C982`、short `#FF3E73`、submit `#087A16`。

## 快捷窗口推断

Pencil 做多为 `[5,10,20,30,50,75]` 且 30 选中，做空为 `[1,2,3,5,10,20]` 且 3 选中。二者可以由同一升序产品档位表按当前索引截取六项：选中高档位时窗口右移，选中低档位时窗口从首项开始。无需创建虚假方向专属产品配置。

## 现有实现差距

- `ContractTradeSheets.vue` 仅有 `draftLeverage`，500px 面板、range、单组六快捷项、装饰性的“应用到多空”开关。
- `TradeView.vue` 仅有 `leverage`，做多和做空按钮、审核快照与请求都使用同一值。
- `margin_user_settings` 仅有 `leverage`；PATCH 只接受 `{leverage}`；GET 只返回一个值。
- 当前设置后端明确只影响后续开仓，不能把 Pencil 中的资金转出样本文案解释为真实持仓变更。
- 当前 `html[data-theme='dark'] .contract-sheet` 位于 scoped style，存在与已修复平仓弹窗相同的 Teleport 编译风险；新实现必须检查编译产物。

## 兼容方案

- 增加 `long_leverage` / `short_leverage`，旧值回填两边。
- PATCH 旧格式将三列统一；新格式同时保存两边；`leverage` 保持做多兼容值。
- Mobile 老后端兼容：若响应缺方向字段，用 legacy 值回退两边。
- 用户关闭弹窗不写库；确认一次原子更新；订单按方向选择设置。
