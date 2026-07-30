# 修复一体化镜像端口冲突与嵌套 Tini

## Goal

修复 1Panel 部署最新一体化镜像时出现的 `Address already in use (os error 98)` 和
嵌套 Tini 警告，使镜像在旧 Compose 环境变量或平台自动注入 init 的情况下仍能稳定启动。

## What I Already Know

- Nginx 固定监听容器 `0.0.0.0:8080`，并把请求转发到 `127.0.0.1:8081`。
- Rust 仍读取 `APP_HOST` 和 `APP_PORT`；旧部署残留 `APP_PORT=8080` 时会与 Nginx 抢占端口。
- 用户日志中的 Tini 进程 PID 为 7，说明 1Panel/Docker 在镜像 Tini 外又启动了 init。
- `default_settlement_mode` 的 `VARBINARY` 解码警告是独立的数据库兼容问题，不会产生端口占用。
- 当前 AMD64、ARM64 和 manifest 发布任务均已成功，用户服务器正在运行已发布的一体化镜像。

## Requirements

- 默认 supervisor 启动前强制导出 `APP_HOST=127.0.0.1` 和 `APP_PORT=8081`。
- 不允许外部遗留环境变量改变集成镜像内部的 Rust/Nginx 端口契约。
- 镜像 Tini 使用 subreaper 模式；作为 PID 1 或被平台 init 包装时均不产生功能缺口。
- Compose 示例继续显式声明正确内部地址和端口，作为可读部署合同。
- `command: ["/usr/local/bin/exchange-migrate"]` 继续绕过 supervisor，不启动 Nginx。
- 更新部署文档、容器规范和进度记录。

## Acceptance Criteria

- [x] `APP_PORT=8080` 和 `APP_HOST=0.0.0.0` 注入默认容器时，Rust 实际仍监听 `127.0.0.1:8081`。
- [x] Nginx 继续独占容器 `8080`，`GET /health` 经 Nginx 返回成功。
- [x] 使用外层 Docker init 启动时不再输出 Tini 非 PID 1/subreaper 警告。
- [x] 直接覆盖镜像 command 时不会启动 supervisor 或 Nginx。
- [x] 完整 Compose 启动、迁移和健康检查通过。
- [x] Dockerfile、shell、Compose、Rust 和后台构建检查通过。

## Out Of Scope

- 不处理预测市场 `default_settlement_mode` 的 MySQL `VARBINARY` 解码兼容问题。
- 不修改业务 API、Worker、数据库 migrations 或前端页面。
- 不改变外部公开端口 `8080` 或 1Panel 宿主机端口映射。

## Research References

- [`research/root-cause.md`](research/root-cause.md)

## Definition Of Done

- 本地构建新镜像并在带完整依赖的隔离 Compose 中复现旧变量场景后验证通过。
- 规范和进度记录已同步。
- 修复提交推送到 `main` 并确认 GitHub 多架构 Workflow 已触发。
