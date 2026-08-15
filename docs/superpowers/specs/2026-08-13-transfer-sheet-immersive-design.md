# 39 资金划转 Sheet 内部沉浸（方案 A）

日期：2026-08-13  
画板：`v6phV`（Light）/ `TuWXq`（Dark）  
方案：数量英雄 + 玻璃控件

## 目标

把 Pencil 里两张「39 / Transfer Sheet」的**内部组件**改成 v03 Immersive Instrument Language：数量是唯一视觉主角，路径和资产是次级仪器，去掉描边表单盒。业务合同与生产 `AssetsView` 划转弹窗对齐，但不改生产代码。

## 范围

改：

- `mobile/pencil/scripts/37-transfer-sheet-immersive.js`（新建）
- 仅重建上述两张画板里 `Transfer Sheet` 的子树

不改：

- 画板外壳、Status Bar、`Faux Assets` 背景、`Dim` 遮罩
- 现货 ↔ 杠杆、资产、数量、确认 四个交互合同
- `AssetsView.vue` 与任何生产测试
- 秒合约 Pair Picker 或其他弹窗

## 内部组件栈（自上而下）

1. **Grab + 标题行**  
   保留 Grab Bar、标题「资金划转」、圆形关闭。标题 18/700。关闭 32 圆、`$surface-2`、Lucide `x`。

2. **数量英雄（ONE HERO）**  
   - 丝绸底板 + mint 径向 Bloom（Light：浅丝绸；Dark：`$surface-2` + `#FFFFFF14` 叠层，不用资产页照片）。  
   - 标签：`划转数量 · USDT`，10–11/500 `$muted`。  
   - 主数字：`0.00`，Geist Mono 30/700 `$text`（Dark 用 `#FFFFFF`）。  
   - 左下：`可划转 —`，禁止编造余额。  
   - 右下：毛玻璃 chip「全部」（仅视觉，不接逻辑）。  
   - 圆角 `$radius-l`，内边距约 16–18。

3. **路径仪器条**  
   一条毛玻璃条，不是并排两个描边盒。  
   左：`从` / `现货账户`；中：mint 圆钮 Lucide `arrow-left-right`；右：`到` / `杠杆账户`（右对齐）。  
   Light：`#FFFFFF99` + `background_blur` 18；Dark：`#FFFFFF14` / `#FFFFFF26`。高度 ≥ 44。

4. **资产行**  
   持仓行语言：Lucide 币种标（`coins`）+ `USDT` + 副文案「选择资产」+ 右侧 `—` / `可划转`。  
   无描边卡片、无原生 select 外观。高度 ≥ 52。

5. **提示 + 确认**  
   提示沿用：`可用余额由钱包接口返回 · 划转即时生效`。  
   主按钮：mint 实心、高 50、圆角 4、文案「确认划转」、图标 `arrow-left-right`、字色 `#07110D`。

Sheet 底板保持 `$surface`，顶圆角 20，不上整板透明。高度按新内容上调（约 500–540），底边贴齐 816 舞台，不遮住状态栏。

## 数据与状态

Pencil 只画默认态：

- 方向：现货 → 杠杆  
- 资产：USDT  
- 数量：`0.00`  
- 可划转：`—`

不画提交中、成功、失败。不伪造金额或可用余额。

## 交付

只拆 `Transfer Sheet` 子树再按上面栈重建。`Status Bar`、`Sheet Stage`、`Faux Assets`、`Dim` 原节点保留，不整板重画。

执行：

```bash
mobile/pencil/run-execute.sh \
  mobile/pencil/hippo-mobile-uiux.pen \
  mobile/pencil/scripts/37-transfer-sheet-immersive.js
```

## 验收

- 两张画板都有：`Amount Hero`、`Route Bar`、`Asset Row`、`Primary 确认划转`
- Sheet 内不再出现描边 `From`/`To`/`Asset`/`Amount` 表单盒
- 主数字是 `0.00`，可划转是 `—`
- `Faux Assets` 与 `Dim` 仍在
- 脚本 `Print` 两张板 ID；无 placeholder 根节点

## 39b 选择资产二级弹窗

画板：`tPkL1` / `tPkD1`，叠在简化划转页之上。

- Grab + 标题「选择资产」+ 关闭
- 毛玻璃搜索「搜索资产」
- 持仓行列表：USDT 选中（mint-soft + check）、BTC、ETH
- 可划转一律 `—`，不编造余额
- 底部说明：可划转余额由钱包接口返回

生产 `AssetsView` 仍用原生 `<select>`，本次不改 Vue。

## 非目标

生产 Vue 同步、错误态、真实余额、背景改成完整划转 Sheet 复制。
