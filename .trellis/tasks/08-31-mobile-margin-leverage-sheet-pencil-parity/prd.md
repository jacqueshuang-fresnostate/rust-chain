# 按 Pencil 1:1 重构手机端杠杆倍数调整弹窗

## Goal

将手机端杠杆交易的“调整杠杆”弹窗严格映射当前 Pencil 选中的深色 `NTiiS` 与浅色 `CulR4` 画板，并让做多、做空倍数成为真实、可独立保存且实际用于对应方向下一笔开仓的后端设置，而不是只复制静态视觉或让两个方向共享一个伪状态。

## What I already know

- 现有 `ContractTradeSheets.vue` 是 500px 单倍数面板，使用一个 range、六个快捷按钮、应用到多空的装饰开关，与当前 840px 双方向 Pencil 画板不一致。
- 当前后端 `margin_user_settings.leverage` 只保存一个默认倍数；移动端也只有一个 `leverage` ref，所以无法诚实实现画板中的做多 30x、做空 3x 独立状态。
- 产品 `leverage_levels` 是唯一允许档位。Pencil 两组快捷值可以由同一有序档位表围绕各自当前值截取六项：高倍数窗口与低倍数窗口不需要伪造第二套产品配置。
- 设置只影响后续开仓，不改已有仓位；现有后端注释与业务合同已经明确这一边界。
- 弹窗 Teleport 到 `body`，明暗主题必须从 `<html data-theme>` 获取；scoped CSS 的整条主题后代选择器必须全局化并检查编译产物。

## Requirements

### Pencil parity

- 根遮罩覆盖 390×920 参考画板；面板位于 `y=80`、宽 390、高 840，仅顶部 24px 圆角，内边距 `18px 20px 16px`、纵向间距 14px。
- 标题栏高 34px，标题 22px/700，关闭按钮 34×34 圆形并使用 Lucide `X`。
- 做多和做空各自包含：16px 方向标题、64px 加减调节行、42×42 加减按钮、52px 数值与 22px `x` 单位、46px 胶囊快捷轨、最多六个 38px 快捷项及 32px 更多入口。
- 做多使用 `#14C982`，做空使用 `#FF3E73`；浅色面板 `#FFFFFF`、字段 `#F0F2F1`、正文 `#111512`，深色面板 `#0B0F0D`、字段 `#181E1A/#202723`、正文 `#F5F7F6`。
- 信息卡按真实数据呈现“调整后最大可开”“所需保证金”，可验证的逐仓场景才显示本地同源预估强平价；缺失或全仓场景显示 `--`，不得使用 Pencil 样本数值。
- 底部确认按钮 350×52、26px 圆角；320–448px 与短屏下保持可滚动、底部安全区、无横向溢出和无内容遮挡。

### Interaction and data

- 做多、做空草稿在每次打开弹窗时从当前产品用户设置初始化；关闭不保存，失败保留两边草稿，成功后一次原子请求保存并关闭。
- 加减按钮只沿后端 `leverage_levels` 前后移动；边界禁用。快捷胶囊只展示真实档位，围绕各自当前档位截取最多六项；更多入口移动可见窗口但不改变倍数。
- 每个方向的快捷项使用 radio/pressed 语义；加减、快捷、更多、关闭和确认均满足至少 44px 触控区域与可见键盘焦点。
- 做多订单实际使用 `long_leverage`，做空订单实际使用 `short_leverage`；订单确认快照与请求继续冻结对应方向的真实倍数。
- 保证金模式仍是单一产品级设置，不随本任务改变；已有仓位、持仓杠杆、资金和平仓逻辑均不被改写。

### Backward-compatible backend contract

- 新增不可变迁移 `0120_margin_directional_leverage_settings.sql`，为 `margin_user_settings` 增加可空 `long_leverage`、`short_leverage` 并从旧 `leverage` 回填。
- `PATCH /api/v1/margin/settings/{product_id}/leverage` 接受二选一载荷：旧 `{ leverage }` 或新 `{ long_leverage, short_leverage }`。新格式必须同时提供两值；混合、部分或全空请求在写库前拒绝。
- 两个方向均必须精确命中同一产品 `leverage_levels`，并在一个数据库事务中保存；任一非法时两边都不改变。
- 响应与 GET 增加 `long_leverage`、`short_leverage`；兼容字段 `leverage` 继续返回做多值。旧请求会把三列统一为同一值，保证 PC/旧 Mobile 客户端行为不变。
- 迁移后旧行的三列值一致；只改保证金模式不得覆盖任一倍数列。

## Acceptance Criteria

- [x] 旧 UI 契约测试先失败，并证明当前面板没有双方向结构和 840px 几何。
- [x] Pencil 两个选中画板的结构、尺寸、色板、字号、间距、快捷选中态和底部确认按钮在真实页面中对齐。
- [x] 做多/做空独立加减、快捷选择、更多窗口、取消、失败重试和确认均可用，且只消费后台真实档位。
- [x] 新旧 PATCH 格式、GET 回读、旧行回填、非法部分请求和事务原子性有后端回归覆盖。
- [x] 做多和做空下单分别冻结对应方向倍数，旧客户端仍可读取/写入单倍数。
- [x] 明暗主题即时切换，scoped CSS 编译后保留 `html[data-theme='dark'] .contract-sheet--leverage`，320/390/448px 无横向溢出。
- [x] Rust 定向测试、Mobile 定向/全量测试、类型检查、PWA/Tauri 构建及 `git diff --check` 通过。
- [x] 规范与 `docs/superpowers/PROGRESS.md` 更新。

## Technical Approach

1. 先扩展后端设置表与 PATCH/GET DTO，以单事务保存双方向倍数并保留旧字段兼容。
2. Mobile API 将双方向字段严格解析为正数；旧服务缺字段时以 legacy `leverage` 同时回退两边。
3. `TradeView` 维护 `longLeverage` / `shortLeverage`，保留一个按 `side` 派生的 active leverage 给现有订单审核链路。
4. 将 leverage 分支重构为独立 Pencil 组件结构，复用产品档位窗口算法和真实余额/行情预览，其他 pair/mode/orderType 弹层不改结构。
5. 编译级、纯函数、源合同、真实浏览器四层验收。

## Decision (ADR-lite)

**Context**：Pencil 明确展示独立多空倍数，而现有单字段后端无法表达；仅复制两个控件会制造保存覆盖或展示与下单不一致。  
**Decision**：把方向性默认倍数扩展为后端权威设置，旧 `leverage` 作为兼容做多值；新移动端一次原子保存两边，并在下单时按方向选择。  
**Consequences**：需要一个小型兼容迁移和跨层测试，但不会修改已有仓位，也不会破坏旧客户端单倍数流程。

## Out of Scope

- 不调整已有仓位的实际杠杆、保证金或强平条件。
- 不新增持仓杠杆变更/资金转出接口。
- 不改变保证金模式、订单类型、交易对和其他三个底部弹层。
- 不展示未经后端/同源公式支持的伪造风险数字。

## Technical Notes

- Pencil：`mobile/pencil/hippo-mobile-uiux.pen`，选中 `NTiiS` / `CulR4`；仅通过 Pencil MCP 读取。
- Mobile：`ContractTradeSheets.vue`、`TradeView.vue`、`api/trading.ts`、双语资源与相关测试。
- Backend：新迁移、margin presentation/application/infrastructure/routes 与 `tests/margin_routes.rs`。
- 研究证据：[`research/pencil-directional-leverage-contract.md`](research/pencil-directional-leverage-contract.md)。
