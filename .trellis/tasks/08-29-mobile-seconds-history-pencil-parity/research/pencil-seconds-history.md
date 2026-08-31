# Pencil 秒合约历史订单选稿测量

## Source

- 文件：`mobile/pencil/hippo-mobile-uiux.pen`（只通过 Pencil MCP 读取）
- 浅色：`vZy6U`，`07d / 秒合约 · 历史订单 · 浅色主题`
- 深色：`x29z7`，`07d / 秒合约 · 历史订单 · 深色主题`
- 基准画板：390×920；标题与筛选使用 16px 左右内轨，订单卡片改为贴齐画板左右边界的全宽布局。

## 2026-08-29 选稿修订

- 用户将当前选中卡片 `s1iRE`（`BTCUSDT · 60秒历史订单卡`）修改为 `x=0 / y=134 / width=390 / height=142`。
- 浅色三张卡片 `s1iRE / cRdKj / RTteZ` 与深色三张卡片 `o1LpPr / B2OgCj / zUyFE` 均为 390px 全宽，x=0，无圆角，y 分别为 134 / 290 / 446。
- 卡片内容保持 `[14,16]` 内边距：内容轨道 x=16、width=358，因此文字仍与标题和筛选左边缘对齐。
- 卡片之间由 14px 画布色带分隔，不使用阴影、边框或圆角制造独立浮层。

## 2026-08-30 当前选稿修订

- 当前仍同时选中浅色 `vZy6U` 与深色 `x29z7`；本轮通过 Pencil MCP 重新读取完整节点树和截图，不沿用旧导出颜色推断。
- 浅色画布与订单卡统一为纯白 `#FFFFFF`；深色画布与订单卡统一为纯黑 `#000000`。旧版浅色 `#F7FAF8`、深色 `#0D1411` 与深色卡片 `#141D18` 均不再属于当前选稿。
- 标题栏可见顺序改为左侧 Lucide `arrow-left` 返回图标、右侧标题“秒合约订单”；标题栏 bounds 为 `x=16 / y=16 / width=358 / height=52`，图标为 `x=0 / y=14 / 24×24`，标题为 `x=238 / y=8.5 / width=120 / height=35`。
- 生产页面保留 44×44px 返回触控框，但 24px 图标必须贴齐标题栏左轨；标题右对齐标题栏右轨。安全返回行为仍通过既有 `goBackOr(..., '/seconds')` 实现。

## Visible hierarchy

Pencil HTML 导出的最终可见顺序：

1. 标题栏：358×52，左侧返回、右侧标题，`space-between`，垂直居中。
2. 筛选组：358×38，横向 gap 8。
3. 历史订单卡：每张 390×142，x=0，无圆角，padding `[14,16]`，纵向 gap 8。
4. 底部留白：由内容数量与视口高度自然产生。

被后续节点覆盖且未出现在最终截图/HTML 导出中的旧“进行中/历史订单”标签和“近 7 天”文字不属于可见实现。

## Typography

- 标题：Noto Sans SC，24/700，浅色 `#17201C`，深色 `#EEF6F1`。
- 返回图标视觉：24px；生产使用 Lucide ArrowLeft，并保留 44px 触控框。
- 交易对：16/600。
- 盈亏：15/700。
- 方向：13/600。
- 状态：13/400。
- 时间与价格摘要：12/400。
- 筛选：13px；激活 600，非激活 400。

## Color mapping

| Role | Light | Dark |
| --- | --- | --- |
| Canvas | `#FFFFFF` | `#000000` |
| Card | `#FFFFFF` | `#000000` |
| Primary text | `#17201C` | `#EFF7F2` |
| Header close | `#69756E` | `#A8B5AE` |
| Positive | `#0DAA79` | `#0DAA79` |
| Negative | `#E05B68` | `#E05B68` |
| Active filter | `#DDF7EC` | `#1E3A30` |
| Inactive filter | `#EAF0ED` | `#17231E` |
| Inactive filter text | `#56625B` | `#B9C7C0` |
| Status | `#718078` | `#A8B5AE` |
| Time | `#8A948E` | `#89968F` |
| Summary | `#78847D` | `#A8B5AE` |

## Card content mapping

- Header: `{symbol} · {duration}` and signed `{profit/loss} {stake asset}`.
- Detail row: localized direction, localized status, short created time.
- Summary: localized stake label/value, entry-price label/value, settlement-price label/value, separated by centered dots.
- No live ticker or local demo value may fill missing settlement data.

## Responsive interpretation

- 390px 画板下标题和筛选保留 16px 内轨，卡片使用 390px 视口全宽；卡片内容再用 16px 内边距回到 358px 文字轨道。
- 320–448px 下卡片始终填满当前手机画布，不固定为 390px；内容、标题与筛选继续使用 16px 安全轨。
- Text rows may min-width-shrink and ellipsize/wrap only when required to prevent horizontal page overflow; the 390px reference remains single-line.
- Lists longer than three cards continue the same 14px rhythm and use document scrolling.
