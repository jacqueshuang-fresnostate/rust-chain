# 修复 GitHub Docker 构建前端镜像认证失败

## 背景

GitHub Actions 在解析 Dockerfile 第 1 行 `# syntax=docker/dockerfile:1.7` 时，先从 Docker Hub 拉取远程 Dockerfile frontend。Docker Hub 的 `auth.docker.io/token` 返回 502，导致构建尚未进入 Rust、Node 或业务镜像阶段就失败。

## 目标

- 让集成镜像构建不再依赖 Docker Hub 上独立的 `docker/dockerfile` frontend 镜像解析。
- 保留当前 BuildKit 构建所需的 cache mount 与 `COPY --chmod` 能力。
- 增加 Dockerfile 契约回归，避免未来重新引入远程 syntax frontend。
- 同步容器交付规范与进度记录，保留可追溯验证结果。

## 非目标

- 不更改 Rust、Node、Nginx、Compose 或发布镜像的业务运行时逻辑。
- 不引入 Docker Hub 账号、静态凭据或未经验证的镜像代理。

## 验收标准

1. Dockerfile 不包含 `# syntax=docker/dockerfile:*` 远程 frontend 指令，使用 GitHub Buildx/BuildKit 自带 frontend。
2. Dockerfile 仍包含 npm/Cargo cache mount 及带权限的静态文件复制，现有同源构建契约继续通过。
3. Docker image contract、source integrity、格式检查与差异检查通过；本机若无 Docker，明确记录未执行真实镜像构建。
