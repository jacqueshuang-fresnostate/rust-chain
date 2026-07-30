# 修复用户更新的 1Panel Compose

## 目标

修复用户刚更新的 `docker-compose.1panel.yml`，使 Compose 能正确解析，并让迁移容器与 API 使用同一组数据库和日志配置。

## 范围

- 保留现有镜像、容器名、端口、上传目录及第三方服务连接配置。
- 将 `DATABASE_URL`、`RUST_LOG` 提取到公共 YAML 环境锚点。
- 让 `migrate` 与 `api` 复用公共锚点，避免宿主机变量缺失。
- 修正 1Panel 外部网络名称。
- 使用 `docker compose config` 和结构化断言验证。

## 非目标

- 不创建或修改 MySQL、MongoDB、Redis、RabbitMQ 服务。
- 不连接用户的生产 1Panel 环境。
- 不修改后端业务代码。
