# 区块链交易所设计文档目录

本目录按功能拆分区块链交易所平台设计文档。总览文档保留在上一层：

`docs/superpowers/specs/2026-05-26-blockchain-exchange-platform-design.md`

## 文件索引

| 文件 | 内容 |
|---|---|
| `01-overview-architecture.md` | 项目定位、MVP 范围、技术架构、核心模块、路线图、关键约束 |
| `02-market-kline-storage.md` | Bitget / HTX 行情、Redis、MongoDB 按交易对拆分、K 线连续性与停机补偿 |
| `03-new-coin-lifecycle.md` | 新币预热、发行、派发、上市、申购、上市后认购、解禁、解禁矿工费、策略行情 |
| `04-wallet-spot-trading.md` | 资产账户、资产流水、现货下单、订单、成交、交易 API |
| `05-admin-agent-permissions.md` | 平台管理员后台、代理后台、邀请关系、权限边界、后台 API |
| `06-security-risk-testing.md` | 风控、安全、合规边界、测试与验收 |
| `07-flash-convert.md` | 闪兑、混合报价、资产划转、后台配置、风控验收 |

## 全局时间字段要求

- 项目所有对外暴露的时间字段必须使用时间戳语义，默认采用 Unix milliseconds 数字。
- REST API、WebSocket、RabbitMQ 事件 payload、Redis 缓存 payload 和 MongoDB 行情文档中的时间值不得输出本地化时间字符串。
- Rust 服务内部和 MySQL 持久化仍可使用 `DateTime<Utc>` / `TIMESTAMP(6)`，但进入或离开系统边界时必须转换为时间戳。

## 存储边界

| 数据类型 | 存储 |
|---|---|
| 用户、资产、订单、闪兑、后台配置、代理关系 | MySQL |
| 历史 K 线、策略行情结果、行情归档 | MongoDB |
| 实时 ticker、盘口、近期 K 线、会话和限流 | Redis |
| 行情、订单、资产、策略、审计事件 | RabbitMQ |
