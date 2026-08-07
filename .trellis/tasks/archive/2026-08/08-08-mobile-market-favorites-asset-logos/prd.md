# 补齐手机端真实资产图片与用户自选接口

## Goal

让手机端行情列表、行情详情/交易页和资产持仓统一消费后台上传并返回的真实 Logo；新增按用户持久化的市场自选接口，替换手机端当前分散的内存或 `localStorage` 自选状态，使首页、行情、现货交易与行情详情展示同一份服务端自选结果。

## Current findings

- 公共 `GET /api/v1/markets` 已返回 `trading_pairs.logo_url`，手机端 `marketMapper` 也会映射到 `MarketTicker.iconUrl`；线上 BTC-USDT 当前确实返回一个上传 URL，但该外部 URL 实测 GET 返回 HTTP 500，因此 `AssetMark` 会诚实退回字母占位。
- 公共市场响应没有返回基础资产与报价资产各自的 `assets.logo_url`，交易对专属图失效时客户端没有第二个后台图片来源。
- `GET /api/v1/wallet/accounts` 已联表返回 `assets.logo_url`；`GET /api/v1/margin/wallets` 当前没有返回 `logo_url`，导致仅存在于杠杆钱包中的持仓只能显示字母。
- 用户自选没有后端表或接口：`MarketsView` 只保存当前组件内存，`TradeView` / `MarketDetailView` 各自使用 `localStorage`，首页“自选”固定返回空列表，四处状态不一致且不能跨设备同步。

## Requirements

### 1. Backend market and wallet Logo contract

- 公共市场响应保留交易对专属 `logo_url`，并新增来自后台资产配置的 `base_logo_url` 与 `quote_logo_url`。
- 查询必须直接联表读取 `trading_pairs.logo_url`、基础资产 `assets.logo_url` 与报价资产 `assets.logo_url`，不内置币种静态图片或第三方币标服务。
- 手机端交易对 Logo 优先使用后台交易对专属图片；专属图片加载失败时只允许回退到同一响应中的基础资产图片，最后才使用现有字母占位。
- 杠杆钱包响应新增 `logo_url`，来源必须是 `assets.logo_url`；手机端映射到 `WalletAccount.logoUrl`。
- 资产页合并现货/杠杆持仓时继续优先使用钱包响应里的真实 Logo，不按 symbol 猜测文件路径。

### 2. Authenticated user favorites API

- 新增 `user_market_favorites` 表，至少包含用户、交易对和创建时间；`(user_id, trading_pair_id)` 必须唯一，用户或交易对删除时级联清理。
- 新增受 `UserAuth` 保护的接口：
  - `GET /api/v1/user/market-favorites`：返回当前用户仍处于 active 状态的自选交易对。
  - `PUT /api/v1/user/market-favorites/:symbol`：按规范化交易对 symbol 幂等添加。
  - `DELETE /api/v1/user/market-favorites/:symbol`：幂等移除。
- 添加前必须验证交易对真实存在且 active；用户之间数据必须隔离；重复添加和重复删除不报业务冲突。
- 自选响应至少包含 `market_id`、规范化 `symbol`、交易对/基础资产/报价资产 Logo 字段，便于客户端直接消费服务端事实。

### 3. Shared mobile favorites state

- 新增单一 API 适配器和 Pinia store，统一负责加载、查询、添加、删除、并发去重和失败回滚。
- 登录成功或应用恢复登录态时加载服务端自选；退出登录或会话失效时清空，不再读取或写入旧的市场自选 `localStorage`。
- 首页、自选行情分类、行情列表星标、现货交易 Header 星标和行情详情星标必须共享同一 store。
- 首页“自选”展示真实已收藏且当前仍有行情的交易对，不再固定为空。
- 未登录用户点击添加自选时进入登录页，并携带当前页面内部 redirect；不生成匿名伪自选。
- 保存中的星标要防止同一 symbol 重复提交；失败时恢复原状态并保留可重试能力。

### 4. UI and accessibility

- 保留 Pencil 当前布局、Logo 尺寸、明暗主题和 Lucide 星标，不调整无关页面结构。
- 后台图片缺失或两个后台图片地址都加载失败时，保留 `AssetMark` 的字母占位和可访问名称，不显示破图。
- 星标按钮继续满足至少 44×44px、键盘焦点、`aria-pressed` 与保存中禁用语义。

## Acceptance Criteria

- [x] `/markets` 返回交易对、基础资产和报价资产三类后台 Logo 字段，现有 `logo_url` 兼容不变。
- [x] `/margin/wallets` 每个钱包条目返回对应后台资产 `logo_url`。
- [x] 用户自选迁移、GET/PUT/DELETE 路由、鉴权、唯一键和用户隔离测试完成。
- [x] 手机端首页、行情页、交易页和行情详情共享服务端自选，旧 `hippo-mobile-market-favorites` 逻辑被移除。
- [x] 行情交易对优先显示后台交易对图片，失败后使用后台基础资产图片；资产持仓显示钱包返回图片。
- [x] 手机端定向测试、全量测试、类型检查和 PWA 构建通过；Rust 格式检查和相关路由测试通过。

## Out of scope

- 不改 PC 端的本地自选实现。
- 不替换管理员已经保存的 Logo URL，也不引入静态币种图标库或外部币标 API。
- 上传存储域名自身返回 HTTP 500 属于部署/存储可用性问题；本任务保证客户端按后台字段加载并提供同源后台图片回退，不伪造图片。
- 不重构行情 WebSocket、K 线、订单簿或交易下单逻辑。

## Definition of Done

- 后端迁移、查询、接口、手机端 store/API/页面接入和回归测试完成。
- 运行相关 Rust 路由测试、`cargo fmt --check`、`npm --prefix mobile run type-check`、`npm --prefix mobile test`、`npm --prefix mobile run build:pwa` 和 `git diff --check`。
- 使用 Ego Browser 在 390px 视口验证首页/行情自选同步、交易对 Logo 回退以及资产持仓 Logo。
- 更新对应 Trellis 规范和 `docs/superpowers/PROGRESS.md`。
