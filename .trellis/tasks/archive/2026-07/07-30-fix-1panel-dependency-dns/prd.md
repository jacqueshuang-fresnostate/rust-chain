# 修复 1Panel 外部依赖 DNS

## 目标

根据服务器实际容器名称修正 1Panel Compose 的外部依赖地址，并明确 API 容器与 MongoDB、Redis、RabbitMQ 的网络连通要求。

## 范围

- 将 RabbitMQ 主机名从不存在的 `rabbit` 修正为实际容器名 `rabbitmq`。
- 保留 MySQL、MongoDB、Redis 的现有连接参数。
- 验证 Compose 展开后的连接主机名和启动门禁。
- 提供将外部依赖接入 `1panel-network` 的服务器命令。

## 非目标

- 不改动第三方容器配置或数据。
- 不在本机模拟用户的 1Panel 生产网络。
