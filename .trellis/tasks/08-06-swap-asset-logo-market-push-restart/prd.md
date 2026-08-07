# 补充闪兑资产图片并修复重启后行情推送

## Goal

闪兑页面使用后端资产元数据展示真实币种图片，并确保 API/容器重启后无需再次进入后台手动重载即可恢复公开行情推送。

## What I already know

- `SwapView` 已复用 `AssetMark`，但未传入 `src`；钱包账户 DTO 已包含 `logoUrl`。
- 当前登录用户的闪兑页面实测 USDT/BTC 只显示字母回退标记，DOM 中没有资产图片。
- 远程 `/api/v1/ws/public` 可以建立连接并返回订阅确认，但 12 秒内没有 ticker 数据；两次 REST ticker 快照的 `observed_at` 完全相同。
- API 启动时优先读取数据库行情订阅配置；数据库没有启用配置时回退 `MARKET_FEED_*` 环境变量。
- 当前 1Panel 与示例 Compose 没有提供 `MARKET_FEED_SYMBOLS`、`MARKET_FEED_INTERVALS`、`MARKET_FEED_PROVIDERS`，所以数据库配置缺失或启动加载失败后会把行情循环永久置为禁用，直到人工重载。
- 远程公开市场当前包含 `BTC-USDT`，因此部署示例可用 `BTCUSDT` 作为明确的兜底交易对，同时允许环境变量覆盖。

## Requirements

- 闪兑支付资产、获得资产和资产选择弹层均传入对应账户的真实 `logoUrl`。
- 图片缺失或加载失败时继续使用 `AssetMark` 现有无损回退，不伪造远程图片地址。
- 1Panel、标准部署示例及 env 示例补充 `MARKET_FEED_SYMBOLS`、`MARKET_FEED_INTERVALS`、`MARKET_FEED_PROVIDERS`。
- 生产示例默认使用 `BTCUSDT`、`1m,5m,15m,1h,1d` 与 `bitget`，并允许部署者覆盖。
- 当前 1Panel 配置添加中文注释，明确数据库启用配置优先、环境变量用于重启兜底。
- 不修改行情协议、交易逻辑、钱包余额或后台手动重载行为。

## Acceptance Criteria

- [x] 闪兑主卡片两侧和选择器资产行在存在真实 `logoUrl` 时渲染 `<img>`。
- [x] 资产图标映射按规范化币种符号查找，且图片失败仍保留字母回退。
- [x] 1Panel API 容器重启时，即使数据库没有启用的行情配置，也会从 Compose 环境获得有效订阅交易对。
- [x] 标准 Compose、1Panel Compose 与对应 env 示例的行情变量保持一致。
- [x] 移动端类型检查、聚焦测试、完整测试、PWA 构建及 Compose 配置解析通过。
- [x] Ego Browser 在登录态闪兑页确认支付、获得和选择器展示真实资产图片且无横向溢出。

## Definition of Done

- 完成生产代码、部署示例、测试与进度记录。
- 执行最贴近改动的移动端、Compose 和 Git diff 验证。
- 使用 Ego Browser 复核真实运行页面。

## Technical Approach

- 使用 `accounts` 中已适配的 `logoUrl` 建立按大写 symbol 查找的资产元数据，不新增请求和后端 DTO。
- 为 `selectedPair` 两端和 `pickerAssets` 统一传入真实图片地址。
- 在 Compose API 环境中提供行情启动兜底变量；数据库启用配置仍由现有启动逻辑优先使用。
- 增加源代码合同测试与 `docker compose config` 解析验证，防止变量再次从部署样例丢失。

## Decision (ADR-lite)

**Context**: 运行中的远程服务可以接受公开 WebSocket 订阅，却没有任何行情帧；现有启动代码在数据库配置不存在时只依赖未配置的 `MARKET_FEED_*` 环境变量。

**Decision**: 保留数据库配置优先级，在所有生产部署样例中提供显式、可覆盖的行情环境兜底；闪兑图片直接复用钱包账户返回的真实资产元数据。

**Consequences**: 容器重启不再依赖人工点击“重载行情”，且不会增加闪兑首屏请求；若部署者希望订阅其他交易对，可通过环境变量或后台启用的数据库配置覆盖默认值。

## Out of Scope

- 不重构行情提供商、WebSocket 协议或 Redis 缓存。
- 不新增资产图片上传功能。
- 不修改 Pencil 文件和本轮已有的资产页/邀请入口实现。
