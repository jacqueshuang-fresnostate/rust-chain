# 移动端资产页 Member 态 Pencil 补设计

- 日期：2026-08-05
- 范围：`mobile/pencil/hippo-mobile-uiux.pen` 设计源文件
- 交付物：仅 Pencil 设计画板（不改线上代码）

## 背景与问题

- 现有 `09 / Assets`（画板 ID `CUK3y`，由 `scripts/05-secondary.js` 创建）只覆盖 **Guest 浅色** 状态。
- 线上 `mobile/src/views/AssetsView.vue` 已实现 Member 态数据合并（`assetRows`，现货 + 杠杆钱包按币种合并），但 UI 只渲染 Top4 占比条，**不展示每个币种的具体持仓数量**。
- 用户核心诉求：登录后必须能看到"我有多少个 USDT / BTC"，即每币种数量 + 估值。

## 已确认决策

| 决策点 | 结论 |
|---|---|
| 交付物 | 仅补 Pencil 设计画板，线上代码后续另行 parity |
| 变体覆盖 | 全量补齐 明暗 × Guest/Member（新增 3 块，CUK3y 已覆盖 Guest 浅色） |
| 现有画板 | **只新增，不修改** `09 / Assets`（CUK3y） |
| Member 版式 | 重新设计：以"我的持仓"列表为一级区块 |

## 新增画板

| 画板名 | 状态 | 内容 |
|---|---|---|
| `09 / Assets · Dark · Guest` | 访客 · 暗色 | CUK3y 结构的暗色镜像（登录引导 hero + 四操作 + 分布占位 + 资金工具） |
| `09 / Assets · Light · Member` | 登录 · 浅色 | 新版式（见下） |
| `09 / Assets · Dark · Member` | 登录 · 暗色 | 新版式暗色主题 |

## Member 新版式结构（自上而下）

1. **Header**：`PORTFOLIO / 资产`，右侧 eye 图标（余额可见性切换，对齐线上 `PageHeader` 的 eye/eye-off 行为；区别于 Guest 的 history 图标）。
2. **总资产 hero（收窄）**：eyebrow `TOTAL VALUE / LIVE`，主数值 `$ —`（等宽数据字体），说明"余额、估值和收益均来自账户接口"。不再是页面唯一信息。
3. **四操作网格**：充币 / 提币 / 划转 / 账单（与 Guest 一致）。
4. **我的持仓（核心一级区块）**：eyebrow `MY HOLDINGS / PRIVATE` + 标题 `我的持仓`；每行结构：
   - 左：AssetMark 风格图标 + 币种符号（数据字体加粗）+ 资产名称副行
   - 右：**总数量**（大字号等宽，`available + frozen + locked`）+ `≈ $—` 折算估值副行
   - 冻结 > 0 时增加副行：`可用 — · 冻结 —`
   - 按估值降序；行点击意图 = 进入资金账单（对齐线上 allocation row 行为）
   - 空态：`暂无持仓` + `去充币` 引导按钮（对应线上 `assets.empty`）
5. **资金工具列表**：资金账单 / 提币记录 / 快捷充值（不变）。
6. **底部导航**：`资产` 高亮。

**移除**：独立的"资产分布 / ALLOCATION"占比条区块（占比信息并入持仓行，可用迷你条表达），避免与持仓列表重复。

## 数据依据（全部为真实接口字段，不虚构）

- `WalletAccount { assetId, symbol, logoUrl, available, frozen, locked }`（`mobile/src/core/types.ts`），现货 `fetchWalletAccounts()`、杠杆 `fetchMarginWallets()`。
- 估值：`marketStore.tickerFor(`${symbol}/USDT`)?.lastPrice` 折算；USDT/USDC/USD 按 1:1。
- 设计稿数值沿用项目惯例（"由接口返回"/`—` 占位），画板结构承载真实数字形态。

## 实施方式

- 新建 `mobile/pencil/scripts/15-assets-variants.js`，沿用现有脚本 helper 模式（`S()/status()/header()/nav()/T()/I()/eyebrow()/empty()/row()`，参照 `05-secondary.js`、`07-wallet.js`）。
- 执行：`mobile/pencil/run-execute.sh mobile/pencil/hippo-mobile-uiux.pen mobile/pencil/scripts/15-assets-variants.js`。
- 结构检查：无 placeholder、零尺寸、横向溢出节点后导出。
- 导出 3 张 PNG 到 `mobile/pencil/exports/`（按画板 ID 命名）。
- 更新 `mobile/pencil/artboards.json`（追加 3 条）与 `mobile/pencil/screen-inventory.md`（Assets 行拆分为 4 状态）。

## 测试影响

- `mobile/tests/pencil-selected-unmapped-pages.test.ts` 断言 `AssetsView.vue` 的 `data-pencil-source="CUK3y i6YDBr"`。本次**不改线上代码**，测试不受影响；后续做代码 parity 时需同步更新该断言与 `data-pencil-source`。

## 非目标（YAGNI）

- 不修改线上 `AssetsView.vue`（后续单独 parity 任务）。
- 不增加搜索、隐藏小额资产、排序切换等持仓列表增强。
- 不改动 CUK3y 及其他任何现有画板。
- 不涉及后端接口变更。
