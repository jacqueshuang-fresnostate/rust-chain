# 现有 `/seconds` 生产实现审计

## 受影响主路径

- `mobile/src/views/SecondsView.vue`：2,223 行单文件，包含完整页面状态、实时行情/K 线、订单对账、确认弹层、结算提示、模板和 scoped CSS。
- `mobile/src/api/seconds.ts`：产品/周期、订单列表、创建订单 DTO 适配与幂等键。
- `mobile/src/core/secondsOrder.ts`：活动订单边界、净收益、进度、状态展示、创建响应合并、历史请求与结算 FIFO 追踪。
- `mobile/src/stores/market.ts`、`mobile/src/api/marketTickerStream.ts`、`mobile/src/api/marketDetailStream.ts`：市场快照、实时 ticker 与实时 1m K 线。
- `mobile/src/core/types.ts`：`MarketTicker` 已包含 `iconUrl/baseIconUrl/quoteIconUrl`，`WalletAccount` 包含 `logoUrl`。
- `mobile/src/components/AssetMark.vue`：真实后台图片优先、失败后字母圆形回退。
- `mobile/src/i18n/messages/{zh-CN,en}.ts`：现有 `seconds.*` 文案对称。
- `mobile/src/router/index.ts`：`/seconds` 与 `/seconds/history` 已是独立命名路由。

## 必须保留的现有业务行为

1. **公开与私有加载隔离**
   - `fetchSecondsProducts()` 独立加载公开产品/周期。
   - 登录后才并行读取 `fetchSecondsOrders(100)` 与 `fetchWalletAccounts()`；单个私有接口失败不应隐藏公开行情。
   - `loadRequestVersion`、`privateReconciliationGeneration` 和 `privateSessionGeneration` 防止旧请求跨刷新/账号写回。

2. **实时行情**
   - `subscribeTickers()` 同时订阅当前产品列表和全部活动订单交易对，支持非当前交易对活动卡的最新价。
   - ticker 订阅使用 generation 与精确 symbols 集合，重建时清理旧价格。
   - `createMarketDetailStreamSession()` 订阅当前交易对 1m K 线；REST 通过 request/context 版本并入实时流，保留最近 48 点。
   - 当前画布绘制器自己监听 ResizeObserver、主题和 K 线变化；卸载时清理。

3. **真实订单生命周期**
   - 活动状态由共享 `activeSecondsOrders()` 识别 `opened/pending/active`。
   - 预计净收益复用 `secondsOrderEstimatedProfit()`，严格为本金 × 利润率。
   - 创建成功返回的订单立即 `upsert`、写入 committed map 并加入结算追踪；后续列表刷新失败只显示 refresh warning。
   - 对账用 `mergeSecondsOrderReconciliation()` 保留尚未出现在列表中的创建响应，直到服务端同 ID 行接管。
   - 每秒更新当前时间；到期订单触发 5 秒重试的私有对账，而不是用行情在前端判定输赢。
   - 多笔订单可并行存在；结算只接受服务端 `settled + win/loss`，通过 FIFO 去重提示。

4. **会话、弹层与路由**
   - 退出登录/卸载会清空私有订单、钱包、重试、提交映射和结算队列。
   - 确认弹层 Teleport 到 body，锁滚动、Escape/Tab 闭环、关闭后恢复焦点；短视口使用三行网格。
   - 结算提示是非模态 pointer-transparent island，不锁页面。
   - Header 历史与结算提示历史操作都进入命名路由 `seconds-history`；安全返回保留 legacy bottom-navigation source 兼容。

## 与当前 Pencil `VL8er/g9agt` 的主要视觉偏差

1. **整体顺序不同**：当前为市场板 → 活动订单（有数据才出现）→ 下单控制台；选稿要求固定 420px 交易操作区内完成市场/图表/表单，再进入独立订单工作区。
2. **市场信息缺失/尺寸不符**：当前只显示 34px 价格、期限和赔率，图表高 170px；选稿要求轮次状态、收益率胶囊、31px 价格、涨跌/标记价、实时标签和 112px 图表。
3. **表单不符**：当前方向 52px 实心按钮在周期前；周期为 36px/18px 圆角；金额是 52px 下划线式，另有余额摘要、反馈、风险说明和 52px 圆角主按钮。选稿要求周期 30px、限额条 26px、金额框 38px、方向 40px、主按钮 44px，并移除额外可见摘要/风险卡对固定几何的占用。
4. **活动订单卡不符**：当前卡片约 4 行、14px 圆角、绿色整卡描边、6px 进度条、没有交易对 Logo/筛选/标题数量；选稿要求 82px 三层密度、主题中性描边、22px Logo、3px 进度和全部/买涨/买跌筛选。
5. **Header 不符**：当前 PageHeader 中心是 44px 原生 `<select>` 壳，根节点还记录旧的 `Lpt6q/WxeB8`；选稿要求 22px 居中交易对轨道与 40px 左右操作，根只声明当前 `VL8er/g9agt`。
6. **主题令牌不精确**：当前使用通用 `var(--page/surface/text/...)`，而选稿规定明确的浅/深画布、订单区、卡片、描边、正文和弱底色。

## 数据映射注意事项

- `SecondsProduct` 当前不含 Logo 字段；不能臆造。交易对和活动订单 Logo 可从 `marketStore.tickerFor(symbol)?.baseIconUrl || iconUrl` 取得后台市场图片，缺失时交给 `AssetMark` 字母回退。
- ticker 实时帧已包含可选 `changePercent`，但当前 `SecondsView` 只保存 `lastPrice`。为了选稿的实时涨跌展示，应保存当前符号的完整实时展示快照或复用市场 freshness 合并逻辑；不要用旧 REST 百分比与新价格混搭。
- 后端没有全局“轮次编号”字段。生产页面不能生成假的 `01842`；应使用同尺寸的真实状态文案与最近活动订单倒计时/待下单状态。订单到期仍以各订单 `expiresAt` 为权威。
- 画板四个期限是密度示例。真实 `cycles` 可能少于或多于四个；390px 前四项需对齐选稿，更多项应可访问且保持无横向页面溢出。

## 推荐测试接缝

- 扩展 `mobile/tests/pencil-trading-product-selected-parity.test.ts` 或增加专用 `seconds-pencil-selected-parity.test.ts`，锁定 frame IDs、模板区域顺序、Logo 来源、筛选和无演示值。
- 扩展 `mobile/tests/award-ui-trading-workspaces.test.ts`，锁定 390px 关键几何、主题令牌、44px 主操作和 112px 图表。
- 保留并运行 `seconds-live-multi-orders.test.ts`：创建响应即时 upsert、并发订单、到期对账、ticker/K 线更新、跨会话隔离。
- 保留并运行 `seconds-api-adapter.test.ts`：DTO、净收益、活动状态、FIFO 结算追踪。
- 保留并运行 `trading-lending-views.test.ts`、`root-prototype-parity.test.ts`：路由、历史入口和主壳层回归。
- Ego Browser 使用真实/只读夹具检查 320、390、448px 明暗主题的计算边界框、滚动范围、中心 Header、筛选和焦点，不触发资金下单。

## 重构前真实浏览器基线

- 2026-08-22 在 390 × 920、浅色、远程真实公开产品/已登录钱包下测得：页面宽 390、无横向溢出；市场板 `y=60, h=222`，图表 `350 × 170`，控制台 `y=282, h=466`，主按钮 `350 × 52`、圆角 26px。
- 这与选稿的 420px 固定交易区、112px 图表、202px 表单和 44px 主按钮明显不符，证明本任务需要结构重排而不是只换颜色。
- 基线截图：`/var/folders/f9/9q7ggh6s5ms7fljhc7d3nmvh0000gn/T/ego-browser-shot-3347-1.png`（本地临时验收证据，不纳入产品资源）。
