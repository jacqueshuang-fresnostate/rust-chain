# GitHub Actions 构建并发布 Docker 镜像

## Goal

为 Rust 后端建立可复现的容器交付链路：GitHub Actions 自动构建多架构镜像并发布到 GitHub Container Registry，同时提供一个能在全新环境完成依赖启动、数据库迁移和 API 健康检查的 Docker Compose 示例。

## What I Already Know

- 后端二进制名为 `exchange-api`，默认监听 `0.0.0.0:8080`，健康检查为 `GET /health`。
- API 启动时必须连接 MySQL、MongoDB、Redis 和 RabbitMQ。
- 当前应用启动过程不会自动执行 SQLx migrations。
- 仓库已有仅启动四个依赖服务的 `docker-compose.yml`，但没有应用 Dockerfile 或根目录 GitHub Workflow。
- 目标 GitHub 仓库为 `jacqueshuang-fresnostate/rust-chain`，GHCR 默认镜像名可使用 `ghcr.io/jacqueshuang-fresnostate/rust-chain`。

## Requirements

- 新增多阶段 Dockerfile，使用锁定依赖构建 release 二进制，运行阶段使用非 root 用户。
- 镜像同时包含 API 与一次性 SQLx migration runner，但默认入口保持 API。
- 新增 `.dockerignore`，禁止把 `.env`、Git 元数据、构建产物、前端依赖和本地工具状态发送到构建上下文。
- GitHub Workflow 在 pull request 中只构建，在 `main`、`v*` 标签和手动触发时发布到 GHCR。
- Workflow 使用 `GITHUB_TOKEN` 和最小必要权限，发布 `linux/amd64`、`linux/arm64` 镜像，并生成 branch、semver、SHA 和 `latest` 标签。
- Compose 示例使用已发布镜像，包含 MySQL、MongoDB、Redis、RabbitMQ、一次性 migration 和 API 服务。
- Compose 使用健康检查、`service_healthy` 和 `service_completed_successfully` 消除启动竞态，使用命名卷保留数据。
- 提供不含真实凭据的环境变量模板和部署说明。

## Acceptance Criteria

- [x] `cargo check --all-targets` 能编译 API 和 migration runner。
- [x] `docker build` 能在本机生成可运行镜像。
- [x] 容器以非 root 用户运行，镜像内同时存在 `exchange-api` 与 `exchange-migrate`。
- [x] `docker compose config` 能使用示例环境文件完整解析。
- [x] 全新 Compose 栈能执行 migrations，API 最终通过 `/health`。
- [x] Workflow YAML 可解析，GHCR 登录、标签、缓存、双架构和 push 条件符合预期。
- [x] 文档说明镜像标签、启动命令、私有包登录和必要密钥。

## Out Of Scope

- 不在本任务中构建 PC、Web 或 Mobile 前端镜像。
- 不部署到 Kubernetes、云主机或第三方容器平台。
- 不把生产密钥写入仓库或 GitHub Workflow。
- 不修改现有业务接口、交易逻辑或数据库 migration 内容。

## Technical Notes

- 相关文件：`Cargo.toml`、`Cargo.lock`、`src/main.rs`、`src/config.rs`、`src/infra/*.rs`、`migrations/`、`docker-compose.yml`。
- GitHub 官方建议 GHCR 使用 `GITHUB_TOKEN`，job 权限至少包含 `contents: read` 和 `packages: write`。
- Docker Compose 必须等待依赖健康，而不是只依赖容器启动顺序。

## Definition Of Done

- 实现、容器构建、Compose 端到端启动与健康检查均通过。
- 更新容器部署规范和 `docs/superpowers/PROGRESS.md`。
- 改动提交并推送到当前 GitHub `main` 分支。
