# 修复 Admin API 同源配置启动崩溃

## Goal

修复 GitHub Actions 发布的一体化 Docker 镜像中，Admin 静态资源因构建阶段缺少
`VITE_API_SAME_ORIGIN` 而在浏览器启动时抛错的问题，确保镜像内 Nginx 提供的 Admin、
REST 与 WebSocket 默认采用同源访问。

## Requirements

- 一体化镜像构建 Admin 时必须显式注入 `VITE_API_SAME_ORIGIN=true`。
- Vite 变量必须在 `npm run build` 阶段注入；不得误导部署者在 Compose 容器运行环境中设置。
- 保留 Admin 对独立 API Origin 的严格配置校验，不以隐式运行时回退掩盖错误部署。
- 增加自动化合同检查，防止 Dockerfile 再次产出缺少同源配置的不可用前端制品。
- 更新 Docker 部署文档，说明同源配置已经固化在镜像构建阶段。

## Acceptance Criteria

- [x] 不依赖本地 `web/.env`，Dockerfile 的 `web-builder` 阶段显式提供同源开关。
- [x] 使用与镜像一致的干净生产环境构建后，Admin 页面加载不再抛出
      `VITE_API_SAME_ORIGIN 必须显式设置为 true 或 false`。
- [x] 同源构建的 REST URL 保持 `/admin/api/v1/*`，WebSocket 使用页面所在 Origin。
- [x] Docker 构建合同测试、Admin 配置测试、类型检查及生产构建通过。
- [x] `docs/superpowers/PROGRESS.md` 记录修复和验证结果。

## Definition of Done

- 自动化测试覆盖镜像构建期变量合同。
- Admin 类型检查、配置聚焦测试和生产构建通过。
- 部署文档与实际镜像行为一致。
- 变更通过 `git diff --check`。

## Technical Approach

在 Dockerfile 的 `web-builder` 阶段定义默认值为 `true` 的构建参数，并仅在执行 Vite
生产构建时将其映射为 `VITE_API_SAME_ORIGIN`。一体化镜像没有独立 API Origin，因而不
注入 `VITE_API_BASE_URL`。使用静态合同测试约束变量必须在构建命令作用域内出现，同时
保留 `web/src/config/backend.ts` 的 fail-closed 校验。

## Decision (ADR-lite)

**Context**：Vite 的 `VITE_*` 变量在编译期替换，Compose 的运行期 `environment` 无法修改
已经生成的 JavaScript。GitHub checkout 不包含被忽略的本地 `web/.env`，因此原 Dockerfile
会稳定构建出启动即抛错的资源。

**Decision**：由一体化镜像 Dockerfile 显式拥有 Admin 同源构建合同，而不是弱化应用配置
校验或依赖开发机 `.env`。

**Consequences**：默认 GHCR 镜像始终通过内置 Nginx 同源访问 Rust；需要独立 API Origin
的非一体化 Admin 部署仍须在自己的前端构建流程中显式提供两项 Vite 配置。

## Out of Scope

- 不修改 Mobile、PC 或 Rust API 行为。
- 不把 Vite 构建变量改造成容器启动时动态配置。
- 不改变 Nginx 现有代理路径。

## Technical Notes

- 根因记录：[`research/root-cause.md`](research/root-cause.md)
- Admin 规范：`.trellis/spec/admin/backend-origin.md`
- 部署合同：`docs/deployment/docker.md`
