# Docker 部署

后端镜像发布到 `ghcr.io/jacqueshuang-fresnostate/rust-chain`。镜像同时包含：

- `/usr/local/bin/exchange-api`：默认启动命令，监听 `0.0.0.0:8080`。
- `/usr/local/bin/exchange-migrate`：只读取 `DATABASE_URL`、执行内置 SQLx migrations 并退出。

运行阶段使用非 root 用户（UID/GID `10001`），并通过镜像内置的
`GET http://127.0.0.1:8080/health` 健康检查报告 API 状态。

## 镜像标签

GitHub Actions 对 pull request 只执行 `linux/amd64`、`linux/arm64` 构建，不推送镜像。
PR job 只授予 `contents: read`；发布 job 才额外授予 `packages: write`。
`main`、`v*` 标签和手动触发会发布镜像：

- `main`：`main`、`latest` 和 `sha-<短提交号>`。
- 稳定版 `v1.2.3`：`1.2.3`、`1.2`、`1`、`latest` 和 `sha-<短提交号>`。
- 手动触发：当前分支、SHA；在默认分支触发时还会更新 `latest`。

生产环境建议固定 semver 或 SHA 标签，避免不可控地跟随 `latest`。

## 使用 Compose 启动

复制模板并替换所有 `change-me` 值。密码建议使用只含十六进制字符的随机值，
这样由 Compose 生成的数据库连接 URL 不需要额外转义：

```bash
cp docker-compose.env.example docker-compose.env
# 生成数据库、消息队列和 JWT 密钥
openssl rand -hex 32
# 生成恰好 32 个 ASCII 字符的 CREDENTIAL_ENCRYPTION_KEY
openssl rand -hex 16
docker compose --env-file docker-compose.env -f docker-compose.example.yml config
docker compose --env-file docker-compose.env -f docker-compose.example.yml pull
docker compose --env-file docker-compose.env -f docker-compose.example.yml up -d
```

启动顺序由 Compose 条件控制：

1. MySQL、MongoDB、Redis 和 RabbitMQ 通过各自健康检查。
2. `migrate` 等待 MySQL 健康，执行全部 migrations，并以状态码 0 退出。
3. `api` 等待四个依赖健康且 `migrate` 成功完成后启动。

检查状态和日志：

```bash
docker compose --env-file docker-compose.env -f docker-compose.example.yml ps
docker compose --env-file docker-compose.env -f docker-compose.example.yml logs migrate
curl --fail http://127.0.0.1:8080/health
```

更新已部署镜像时重新执行 `pull` 和 `up -d`。SQLx migration runner 是幂等的，
每次更新都会先确认 migrations 已应用，再启动新 API 容器。

## 单独运行命令

只执行迁移：

```bash
docker run --rm \
  --network your-network \
  --env DATABASE_URL='mysql://user:password@mysql:3306/exchange' \
  ghcr.io/jacqueshuang-fresnostate/rust-chain:1.2.3 \
  /usr/local/bin/exchange-migrate
```

直接启动 API 时，使用 `--env-file` 提供与
[`docker-compose.env.example`](../../docker-compose.env.example) 等价的运行时配置，
并确保 MySQL、MongoDB、Redis 和 RabbitMQ 已可访问。不要覆盖镜像命令即可使用默认
`exchange-api`。

## 私有 GHCR 包登录

公开包可直接拉取。私有包需要具有 `read:packages` 权限的 GitHub personal access token：

```bash
printf '%s' "$GHCR_TOKEN" | docker login ghcr.io --username YOUR_GITHUB_USER --password-stdin
docker pull ghcr.io/jacqueshuang-fresnostate/rust-chain:1.2.3
```

GitHub Actions 发布流程使用仓库自带的 `GITHUB_TOKEN`，不需要保存额外 GHCR 密钥。

## 密钥与持久化

提交到仓库的环境文件只包含占位值，复制后的 `docker-compose.env` 已被
`.gitignore` 和 `.dockerignore` 排除。部署前至少替换以下值：

- MySQL root/应用密码、MongoDB root 密码、RabbitMQ 密码。
- `JWT_SECRET`：使用高熵随机值。
- `CREDENTIAL_ENCRYPTION_KEY`：必须恰好为 32 字节。该值用于 AES-256-GCM，
  一旦保存过加密凭据就必须保持稳定，否则旧数据无法解密。

不要提交实际的 `docker-compose.env`。生产环境应从受控的 secret manager 或受限权限文件
注入这些值，并定期备份 `mysql-data`、`mongo-data`、`redis-data`、
`rabbitmq-data` 和 `uploads-data` 命名卷。更换数据库或消息队列密码时，需要同步更新连接
URL 所依赖的值。

Compose 把 `uploads-data` 挂载到 API 容器的 `/app/uploads`。如果后台启用可选的本地上传
provider，请把其 `local_root` 精确配置为 `/app/uploads`，并让反向代理或静态文件服务把同一
数据卷映射到所配置的 `public_base_url`；API 本身不会自动公开该目录。未使用本地 provider
时，这个卷保持为空。
