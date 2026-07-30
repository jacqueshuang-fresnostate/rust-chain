# 新增 1Panel 外部依赖 Compose 配置

## Goal

在保留现有全栈 Compose 示例的同时，新增一套可直接导入 1Panel 的后端部署配置。该配置只运行项目自身的数据库迁移器和 API，不创建 MySQL、MongoDB、Redis 或 RabbitMQ，由环境变量连接用户在 1Panel 中独立安装和维护的服务。

## Requirements

- 新增可公开提交的 `docker-compose.1panel.example.yml`，本地含真实值的
  `docker-compose.1panel.yml` 保持忽略，不修改现有本地依赖 Compose 和完整部署示例。
- 只定义 `migrate` 与 `api` 两个业务服务。
- 使用 GHCR 多架构后端镜像，支持通过环境变量固定 semver 或 SHA 标签。
- MySQL、MongoDB、Redis 和 RabbitMQ 使用完整连接 URL，不在 Compose 中创建第三方容器、数据卷或健康检查。
- 两个业务服务加入 1Panel 的外部 Docker 网络，默认网络名为 `1panel-network` 且允许覆盖。
- `api` 必须等待 `migrate` 成功退出后再启动。
- API 继续监听容器内 `0.0.0.0:8080`，默认只绑定宿主机 `127.0.0.1:8080`，允许按部署拓扑覆盖。
- `/app/uploads` 使用独立持久化 Docker volume。
- 提供无真实凭据的环境变量示例，并记录 1Panel 导入、反向代理、迁移、更新和排障步骤。
- 不把任何真实密码、令牌或私有地址提交到仓库。

## Acceptance Criteria

- [x] 1Panel Compose 只包含 `migrate` 和 `api`。
- [x] Compose 解析后两个服务均使用相同镜像和外部 `1panel-network`。
- [x] API 包含全部必需后端环境变量并等待迁移成功。
- [x] 迁移器只接收 `DATABASE_URL` 和日志配置。
- [x] 环境变量示例包含全部必填 URL、密钥、网络、端口和上传卷名称。
- [x] `docker compose config` 可使用示例环境文件成功解析。
- [x] 部署文档明确第三方服务由 1Panel 独立安装，并说明容器名/LAN 地址两种连接方式。
- [x] 容器交付规范和进度记录已同步。

## Out Of Scope

- 不创建或配置 MySQL、MongoDB、Redis、RabbitMQ。
- 不修改后端代码、数据库 migrations、Docker 镜像构建或 GitHub Workflow。
- 不部署 PC、管理后台、手机 PWA 或反向代理容器。

## Definition Of Done

- 配置、示例和文档完成。
- Compose 结构和展开结果通过自动校验。
- 更新 `.trellis/spec/backend/container-delivery.md` 与 `docs/superpowers/PROGRESS.md`。
