# 修复手机端秒合约 Header、实时行情与并行订单

## Goal

修复手机端秒合约页面头部重叠、价格与图表只停留在首屏快照、已有活动订单时无法继续下单的问题，使页面在 320–448px 手机宽度内保持稳定布局，价格和 1 分钟 K 线持续接收内部行情流，并支持多笔秒合约订单并行展示和独立倒计时。

后续需求追加：将已结束的秒合约历史订单从交易工作台中拆出为独立二级页面，`/seconds` 右上角历史按钮直接进入该页面。

## What I already know

- `SecondsView` 同时渲染了 `PageHeader` 标题和绝对定位的 `seconds-pair-field`，两者占用同一中间区域，形成截图中的重复文字、蓝色边框和溢出。
- 秒合约页面只调用 `marketStore.refresh()` 和一次性 `fetchKlines()`；已有共享 ticker WebSocket、详情 K 线 session、REST/WS 竞态合并与本地 KLineCharts 渲染能力。
- 手机端使用单一 `activeOrder`，并在存在活动订单时禁用方向、周期、金额和提交按钮；后端订单表和开仓事务没有“同一用户只能有一笔活动单”的限制。
- 后端通过独立幂等键、钱包行锁和余额校验保护每次开仓；并行下单仍必须保留这些合同，余额不足应由后端作为最终裁决。

## Requirements

- 在共享 `PageHeader` 中提供可复用的中间内容插槽，秒合约交易对选择器必须进入 Header 的中间网格，不再使用覆盖 Header 的绝对定位元素。
- Header 保留返回、交易对、秒合约标识和历史入口；在 320px、390px、448px 宽度下不得出现重复标题、焦点边框溢出、按钮挤压或被正文遮挡。
- 页面加载时先取得内部 REST 行情和 1 分钟 K 线快照，再启动项目现有的内部 WebSocket 行情；切换交易对时立即切换实时订阅。
- 最新价与走势图必须持续响应 WebSocket 推送；实时 candle 对同时间戳的迟到 REST candle 具有最终权威，旧交易对、旧请求和重连前 generation 不得污染当前图表。
- 图表继续使用项目内置 KLineCharts/本地渲染链路，不添加外部 TradingView iframe、远程脚本或第三方数据源。
- 将单一活动订单状态改为活动订单集合；存在活动订单时仍可选择方向、周期、金额并再次提交。
- 每一笔活动订单都按自身交易对、到期时间和金额展示独立最新价、倒计时、进度与预计收益，跨交易对活动订单也不得被当前选择器隐藏。
- 多笔订单到期时按订单 ID 去重触发 reconciliation，避免同一订单每秒重复刷新，同时确保到期订单和钱包余额最终回到后端权威状态。
- 下单 mutation 期间只禁用当前提交动作，不因已有活动订单锁死整个表单；继续使用每次唯一幂等键、现有确认弹窗和现有 API。
- 保持访客可查看公共产品与行情；订单和钱包仍遵守现有登录态及后端权限。
- 新增命名路由 `seconds-history`，路径为 `/seconds/history`，隐藏底部导航、深度高于 `/seconds`，直接打开时返回兜底为 `/seconds`。
- `/seconds` 右上角 History 按钮必须通过命名路由 push 到 `seconds-history`，不再滚动到本页订单记录；交易页底部不再重复渲染历史订单列表。
- `/seconds` 继续保留全部活动订单卡片，因为它们属于当前交易状态而不是历史记录。
- 独立历史页只显示非活动状态订单，并展示真实交易对、方向、投入金额、期限、开仓价、结算价、结果/状态和创建时间；不得用实时价伪造缺失的结算价。
- 历史页复用 `fetchSecondsOrders()`、订单适配器和现有状态翻译；访客进入时呈现紧凑登录引导，加载、错误、空数据和真实列表必须互斥且可重试。
- 历史页使用共享 Pencil `PageHeader`、Lucide 图标、双主题语义色、44px 触控目标和底部安全区，在 320–448px 下无横向溢出。

## Acceptance Criteria

- [x] 秒合约 Header 只渲染一个交易对标题区域，返回、交易对选择和历史按钮在 320–448px 宽度内均不重叠。
- [x] 交易对选择器的 focus/active 样式限制在中间控件内部，不再出现覆盖整段 Header 的异常矩形边框。
- [x] 首屏 REST 数据加载后，ticker WebSocket 推送能更新最新价，K 线 WebSocket 推送能追加或覆盖当前 1 分钟 candle。
- [x] 切换交易对和组件卸载会清理旧订阅；迟到 REST、旧交易对和旧 generation 数据不会覆盖当前图表。
- [x] 已存在一笔或多笔活动订单时，方向、周期、金额和确认下单仍可操作；提交中的重复点击仍会被阻止。
- [x] 页面同时渲染全部活动订单，并为每笔订单显示独立交易对、方向、金额、开仓价、实时价、倒计时和进度。
- [x] 多笔订单同时到期不会形成重复刷新风暴，结算后订单与钱包可完成后端权威对账。
- [x] 现有秒合约确认弹窗、安全区和滚动行为不回退。
- [x] 针对 Header、实时数据流、并行订单与到期刷新补充回归测试。
- [x] Mobile type-check、tests、PWA build、Tauri build、`git diff --check` 和 Ego 浏览器手机视口验收通过。
- [x] `/seconds` 右上角历史按钮 push 到 `#/seconds/history`，浏览器返回回到交易页，直接打开历史页时返回兜底为 `/seconds`。
- [x] `/seconds` 不再渲染底部历史订单列表，活动订单卡片仍保留在交易工作台。
- [x] 历史页只渲染非活动订单，完整显示真实订单字段，并正确覆盖访客、加载、失败、空态与重试。
- [x] 历史页在 320px、390px、448px 明暗主题下无横向溢出，Header 和记录行触控/聚焦符合移动端规范。
- [x] 路由、数据过滤、状态显示和页面结构回归测试以及 Mobile 全量质量门通过。

## Out of Scope

- 不修改秒合约数据库结构、结算公式、赔率计算、钱包扣款语义或后端幂等合同。
- 不新增订单数量上限、风控策略或私有 WebSocket 协议。
- 不重做秒合约页面其他未提及的信息架构，也不修改 PC 或后台页面。
- 不新增历史订单后端接口、服务端分页或状态筛选参数；独立页消费现有最近 100 条订单响应并在适配后过滤活动状态。

## Technical Notes

- Header 根修复优先扩展 `PageHeader` 的 copy slot，保留原文字渲染为默认 fallback。
- ticker 复用 `marketStore`/`subscribeTickers`；K 线复用 `createMarketDetailStreamSession` 的 generation 和 REST/WS 竞态合同，必要时只扩展 channel 选项而不复制协议解析器。
- 活动订单使用 `activeOrders` 计算集合和逐单 helper；到期去重使用 `Set<orderId>` 或等价结构。
- 历史订单判断复用 `isActiveSecondsOrder`，禁止在页面中维护第二份活动状态枚举；路由与返回遵循 `navigation-and-localization.md`。
- 主要规格：`.trellis/spec/mobile/index.md`、`.trellis/spec/mobile/pwa-and-shell.md`、`.trellis/spec/mobile/backend-integration.md`、`.trellis/spec/backend/seconds-contracts.md`。

## Definition of Done

- Header、内部实时 ticker/K 线、多活动订单与逐单到期对账均完成，并通过源码合同测试、运行时浏览器验收、移动端全量质量门和进度记录。
