# 构建 Rust 与后台前端一体化 Nginx 镜像

## Goal

让 GitHub Actions 发布的同一个 GHCR 镜像同时包含 Rust 后端、后台管理前端和 Nginx，
1Panel 部署一个业务镜像即可同时访问后台页面与 API，同时继续使用同一镜像执行数据库迁移。

## What I Already Know

- 后台前端位于 `web/`，构建命令是 `npm run build`，产物目录是 `web/dist`。
- 后台使用 Browser Router，页面路径为 `/login`、`/admin/*`、`/agent/*`。
- API 与 WebSocket 已使用可与页面区分的固定前缀。
- 现有 Compose 对外暴露容器 `8080`，该端口必须保持兼容。
- `exchange-migrate` 必须继续作为同一镜像的命令覆盖单独运行。

## Requirements

- Dockerfile 使用独立 Node 阶段安装锁定依赖并构建 `web/`。
- 最终 Debian 镜像安装 Nginx 与 Tini，继续以 UID/GID `10001:10001` 非 root 运行。
- Nginx 在 `0.0.0.0:8080` 提供后台静态资源和 SPA history fallback。
- Rust API 在 `127.0.0.1:8081` 运行，不直接暴露到容器外。
- Nginx 代理 `/health`、三组版本化 API、WebSocket、事件和 OpenAPI 文档路径。
- `/app/uploads` 可由 Nginx通过 `/uploads/` 提供静态访问。
- Nginx 必须正确传递客户端地址、Host、协议和 WebSocket Upgrade 头。
- 默认启动命令同时监管 Rust 与 Nginx；任一退出时终止另一进程并退出容器。
- `command: ["/usr/local/bin/exchange-migrate"]` 仍只启动迁移器。
- 完整 Compose 与 1Panel Compose 将 Rust 内部端口改为 `8081`，对外映射仍为 `8080`。
- GitHub 双架构原生构建、digest 合并和标签行为保持不变。
- 更新容器部署文档和可执行规范。

## Acceptance Criteria

- [x] `npm --prefix web run build` 成功。
- [x] Nginx 配置语法检查成功。
- [x] Compose 两个版本均可完整展开。
- [x] 构建后的镜像包含两个 Rust 二进制、后台 `index.html`、Nginx、Tini 和 supervisor。
- [x] 镜像仍以 `10001:10001` 运行并只公开 `8080`。
- [x] 在可用依赖环境中，`GET /health` 返回 Rust JSON，`GET /login` 和深层后台路由返回 SPA。
- [x] WebSocket/API 前缀由 Nginx 转发，静态资源由 Nginx直接提供。
- [x] migration 命令覆盖不会启动 Nginx。
- [x] Docker Workflow 双架构发布合同未回退。

## Out Of Scope

- 不把 PC 客户端或移动端构建产物加入镜像。
- 不把 MySQL、MongoDB、Redis、RabbitMQ 加入业务镜像。
- 不更改后台 UI、Rust API 路由或业务功能。
- 不更改 1Panel 中第三方容器的安装方式。

## Research References

- [`research/container-architecture.md`](research/container-architecture.md)

## Definition Of Done

- 聚焦构建、配置、容器运行验证全部通过。
- 文档、Trellis 容器规范和进度记录已更新。
- 不覆盖或回退工作区中的其他用户改动。
