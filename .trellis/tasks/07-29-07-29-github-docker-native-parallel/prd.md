# 修复 GitHub Docker 双架构构建超时

## Goal

消除单个 x86 GitHub runner 使用 QEMU 串行编译 AMD64/ARM64 导致的 60 分钟超时，让两个平台在原生 runner 上并行构建，并在 GHCR 生成统一多架构 manifest。

## What I Already Know

- 首次运行 `30418701410` 在 QEMU 构建步骤执行约 58 分钟后被取消。
- 失败日志仍处于 Rust crate 编译阶段，不是代码编译错误或 GHCR 登录错误。
- 仓库公开，GitHub 提供 `ubuntu-24.04` 与 `ubuntu-24.04-arm` 原生托管 runner。
- Docker 官方 `docker/github-builder/.github/workflows/build.yml@v1` 默认按平台分发 runner 并在 finalize 阶段合并 manifest。

## Requirements

- 使用 Docker 官方可复用 Workflow 分发 `linux/amd64`、`linux/arm64` 构建。
- 不再为这两个平台启用 QEMU。
- pull request 只构建不推送，不能获得 `packages: write`。
- `main`、`v*` 标签和手动触发继续发布 GHCR。
- 保留 branch、semver、SHA、`latest` 标签语义。
- 开启签名 GitHub Actions 构建缓存；发布镜像生成签名 provenance。

## Acceptance Criteria

- [x] Workflow YAML 和可复用 workflow 调用契约可解析。
- [x] PR job 仅有 `contents: read`、`id-token: write`，并设置 `push: false`。
- [x] publish job 增加 `packages: write`，使用 `GITHUB_TOKEN` 登录 GHCR。
- [x] 两个 job 都请求 `linux/amd64,linux/arm64`，并启用分布式原生构建。
- [x] Workflow 中不再调用 QEMU setup。
- [ ] GitHub 新运行在原生 AMD64/ARM64 runner 上并行构建成功。
- [ ] GHCR `latest` manifest 同时包含 `linux/amd64`、`linux/arm64`。

## Out Of Scope

- 不修改 Dockerfile、Compose 服务或后端业务代码。
- 不增加自托管 runner。
- 不改变镜像仓库名和已有标签消费方式。

## Technical Notes

- Docker 官方建议复杂多平台镜像分发到每个平台各自 runner，避免同机 QEMU 长时间构建。
- Docker GitHub Builder 默认把 Linux ARM64 映射到 `ubuntu-24.04-arm`，其他 Linux 平台映射到 `ubuntu-24.04`。
- `id-token: write` 用于签名 provenance 和 GitHub Actions cache，不授予仓库内容写权限。

## Definition Of Done

- Workflow 修复提交并推送到 `main`。
- 新 GitHub Actions 运行成功。
- 公开镜像 manifest 平台检查通过。
- 更新部署规范和 `docs/superpowers/PROGRESS.md`。
