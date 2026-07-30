# Docker 部署

业务镜像发布到 `ghcr.io/jacqueshuang-fresnostate/rust-chain`。镜像同时包含：

- `/usr/local/bin/exchange-api`：Rust API，只监听容器回环地址 `127.0.0.1:8081`。
- `/usr/local/bin/exchange-migrate`：读取 `DATABASE_URL`，执行内置 SQLx migrations，
  在管理员表为空时创建首个后台管理员后退出。
- `/usr/share/nginx/html`：由 `web/` 锁定依赖构建的后台管理与代理门户静态资源。
- Nginx：在 `0.0.0.0:8080` 提供 SPA、`/uploads/` 静态文件，并把后端路径转发给 Rust。

默认命令由启用 subreaper 模式的 Tini 启动 supervisor，同时监管 Rust 与 Nginx；任一
进程退出时都会终止另一进程并退出容器。Tini 既可直接作为 PID 1，也可被 1Panel 或
Docker 自动提供的外层 init 包装。运行阶段整体使用非 root 用户（UID/GID `10001`），
并通过镜像内置的 `GET http://127.0.0.1:8080/health` 健康检查经 Nginx 确认 Rust API
状态。

supervisor 在启动 Rust 前会无条件导出 `APP_HOST=127.0.0.1` 和 `APP_PORT=8081`。
因此旧编排或平台即使注入 `APP_HOST=0.0.0.0`、`APP_PORT=8080`，也不能改变一体化镜像
内部的监听边界或与 Nginx 抢占 `8080`。Compose 示例仍显式声明正确值，作为可读部署
合同。

Nginx 会把 `/health`、`/api/v1/*`、`/admin/api/v1/*`、`/agent/api/v1/*`、
`/ws/*`、`/events/*`、`/docs`、`/openapi.json`、`/api/docs` 和
`/api/openapi.json` 原样转发给 Rust。`/login`、`/admin/*`、`/agent/*` 等浏览器路由
使用 SPA history fallback 返回后台 `index.html`，`/uploads/*` 则直接读取
`/app/uploads`。

## 原生双架构构建

原 Workflow 曾在单个 x86 runner 上通过 QEMU 串行构建 AMD64 和 ARM64。运行
`30418701410` 约 58 分钟后仍停留在 Rust crate 编译阶段并被取消，问题不是代码编译错误
或 GHCR 登录失败。

第一次原生 runner 修复尝试使用 Docker 官方可复用 Workflow，但运行 `30430170926`
在 ARM64 runner 解析远程 Git context 时遇到
`unknown API capability source.git.checksum`。当前 Workflow 因此改用本地 checkout
context 和显式矩阵：

- `linux/amd64` 分配到 `ubuntu-24.04`。
- `linux/arm64` 分配到 `ubuntu-24.04-arm`。

两个平台在各自的 GitHub-hosted runner 上并行构建，不再安装或使用 QEMU。发布 job
使用 `actions/checkout` 后以 `context: .` 构建，每个平台按 digest 推送并上传一个短期
digest artifact；只有两个平台都成功，最终 job 才会把 digest 合并为统一的多架构
manifest，并附加 branch、semver、SHA 和 `latest` 标签。构建缓存使用 `max` 模式和按架构
隔离的 `backend-image-<arch>` scope。

## 镜像标签

GitHub Actions 对 pull request 只执行 `linux/amd64`、`linux/arm64` 构建，不推送镜像。
PR job 只授予 `contents: read`，设置 `push: false`，且没有 `packages: write` 或 GHCR
登录凭据。按 digest 推送的平台 job 和最终 manifest job 才授予 `packages: write`，并通过
`GITHUB_TOKEN` 登录 GHCR。
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
2. `migrate` 等待 MySQL 健康，执行全部 migrations，引导首个管理员，并以状态码 0 退出。
3. `api` 等待四个依赖健康且 `migrate` 成功完成后启动集成的 Nginx 与 Rust 服务。

Compose 只把容器的 Nginx `8080` 映射到宿主机；`APP_HOST=127.0.0.1` 和
`APP_PORT=8081` 是 Rust 在容器内部的固定监听边界，不应映射到宿主机。

检查状态和日志：

```bash
docker compose --env-file docker-compose.env -f docker-compose.example.yml ps
docker compose --env-file docker-compose.env -f docker-compose.example.yml logs migrate
curl --fail http://127.0.0.1:8080/health
```

更新已部署镜像时重新执行 `pull` 和 `up -d`。SQLx migration runner 是幂等的，
每次更新都会先确认 migrations 已应用，再启动新 API 容器。

## 初始化首个后台管理员

项目内置以下默认管理员，数据库全新且 `admin_users` 为空时由一次性的 `migrate` 服务
自动创建：

```dotenv
BOOTSTRAP_ADMIN_USERNAME=admin
BOOTSTRAP_ADMIN_PASSWORD=Qaz123456@
BOOTSTRAP_ADMIN_ROLE_NAME=super_admin
```

三个环境变量未配置或为空时使用上述内置值；设置非空值可以覆盖对应默认值。覆盖账号或
角色格式非法、覆盖密码少于 8 个或超过 128 个字符时，迁移器以非零状态退出并阻止 API
启动。账号会去除首尾空白、转为小写，并限制为 3–32 位字母、数字或下划线；角色名会转为
小写并限制为 1–64 位字母、数字、下划线或连字符。

`admin_users` 为空时，迁移器在同一事务内创建或复用角色，并使用项目现有 Argon2 helper
保存密码哈希。若已经存在任意管理员，则整段引导跳过，不创建额外角色，也不修改现有
账号、角色或密码。并发迁移器通过数据库命名锁串行执行该步骤，日志和错误信息不会包含
明文密码。

`Qaz123456@` 是公开默认密码。生产环境应在首次初始化前通过
`BOOTSTRAP_ADMIN_PASSWORD` 覆盖它；数据库已经存在管理员后重新设置该变量不会重置密码。
三个引导变量只传给 `migrate`，不会传给长期运行的 `api` 容器。

## 通过 1Panel 部署，第三方服务独立管理

当 MySQL、MongoDB、Redis 和 RabbitMQ 由你在 1Panel 中单独安装和维护时，使用
[`docker-compose.1panel.example.yml`](../../docker-compose.1panel.example.yml)。这份
配置不会创建任何第三方服务，只运行：

- `migrate`：执行内置 SQLx migrations，成功后以状态码 `0` 退出。
- `api`：等待 `migrate` 成功后启动，并持续提供后台页面、静态上传和 API。

两个容器默认加入 1Panel 的外部 `1panel-network`。请先确认独立安装的第三方服务也加入
该网络；如果使用其他网络，把 `ONEPANEL_NETWORK` 改成实际网络名。依赖服务在其他服务器
时不需要加入本机 Docker 网络，连接 URL 应填写可从 API 容器访问的内网域名或 IP。

准备环境变量：

```bash
cp docker-compose.1panel.env.example docker-compose.1panel.env
cp docker-compose.1panel.example.yml docker-compose.1panel.yml
# 生成 JWT_SECRET
openssl rand -hex 32
# 生成恰好 32 个 ASCII 字符的 CREDENTIAL_ENCRYPTION_KEY
openssl rand -hex 16
```

推荐通过 1Panel 编排环境或 `docker-compose.1panel.env` 提供变量。YAML 锚点只是复用
映射内容，不会把 `x-api-environment` 中直接填写的值注册为 `${DATABASE_URL}`。如果
确实要把值直接写进 Compose，应把 `DATABASE_URL` 和 `RUST_LOG` 写在
`x-common-environment`，让 `api` 与 `migrate` 同时复用；不要只修改 API 环境而保留
迁移器的 `${DATABASE_URL}`。管理员引导变量则必须保留在 `migrate.environment`，不得
移入公共或 API 环境锚点。外部网络名称应填写 `1panel-network` 或真实网络名，不要误写
成带前导连字符的 `-1panel-network`。

至少需要确认这些完整连接 URL：

```dotenv
DATABASE_URL=mysql://用户:密码@MySQL容器名:3306/数据库名
MONGODB_URI=mongodb://用户:密码@MongoDB容器名:27017/?authSource=admin
MONGODB_DATABASE=数据库名
REDIS_URL=redis://:密码@Redis容器名:6379/0
RABBITMQ_URL=amqp://用户:密码@RabbitMQ容器名:5672/%2f
```

容器名必须使用 1Panel 中实际显示的名称或网络别名，不要假定一定是 `mysql`、`mongodb`、
`redis`、`rabbitmq`。密码包含 `@`、`:`、`/`、`#`、`%` 等字符时，必须先做 URL
percent-encoding；也可以使用只含十六进制字符的随机密码。

在上传到服务器前可先展开检查配置：

```bash
docker compose \
  --env-file docker-compose.1panel.env \
  -f docker-compose.1panel.example.yml \
  config
```

1Panel v2 可在「容器 → 编排」中通过编辑器、服务器上的 Compose 路径或编排模板创建部署。
推荐把 Compose 内容和环境变量保存在 1Panel 管理的编排中，或者把
`docker-compose.1panel.env` 作为同目录 `.env` 文件；不要把真实环境文件提交到 Git。
具体入口参考 [1Panel 编排文档](https://1panel.cn/docs/v2/user_manual/containers/compose/)。

部署前确保 `1panel-network` 已存在且四个依赖都可访问，然后创建编排。正常状态为：

1. `hippo-exchange-migrate` 日志显示 migrations 完成，容器状态为已退出且退出码为 `0`。
2. `hippo-exchange-api` 随后启动并变为 healthy。
3. `curl --fail http://127.0.0.1:8080/health` 返回成功。

宿主机端口映射应始终指向容器的 Nginx `8080`，避免直接暴露 Rust 的内部 `8081`。可按
实际拓扑选择一种代理方式：

- 反向代理访问宿主机：目标填写 Compose `ports` 左侧配置的实际地址和端口。
- 反向代理容器已加入 `1panel-network`：目标填写
  `http://hippo-exchange-api:8080`。

需要从其他主机访问时，应调整 Nginx 的宿主机端口绑定并配置防火墙；不要把
`APP_HOST` 改为 `0.0.0.0` 或把 `8081` 映射到宿主机。生产后台、移动端和 PWA 均应通过
HTTPS 域名访问页面、`/api/v1`、`/health` 和 WebSocket。

更新镜像时，先在 1Panel 拉取目标标签，再重建编排。建议修改 `EXCHANGE_IMAGE` 固定到
新的 semver 或 `sha-*` 标签；镜像变化会重新创建 `migrate`，迁移成功后才会重建 API。
回滚应用镜像不会回滚数据库 schema，回滚前必须确认旧版本与当前 migrations 兼容。

如果 `api` 没有启动，先检查 `migrate` 的退出码和日志；如果迁移成功但 API 反复退出，
再依次从 API 容器验证 MongoDB、Redis 和 RabbitMQ 的主机名、端口、密码、数据库索引、
`authSource` 和 TLS 参数。外部依赖不属于该 Compose，1Panel 不会通过本编排替它们执行
健康检查或启动排序。

## 单独运行命令

只执行迁移：

```bash
docker run --rm \
  --network your-network \
  --env DATABASE_URL='mysql://user:password@mysql:3306/exchange' \
  ghcr.io/jacqueshuang-fresnostate/rust-chain:1.2.3 \
  /usr/local/bin/exchange-migrate
```

不提供引导变量时，上述命令使用内置的 `admin / Qaz123456@`；生产环境可通过同名
`--env` 参数覆盖。

启动默认集成服务时，使用 `--env-file` 提供与
[`docker-compose.env.example`](../../docker-compose.env.example) 等价的运行时配置，
并确保 MySQL、MongoDB、Redis 和 RabbitMQ 已可访问。不要覆盖镜像命令即可由 Tini 和
supervisor 同时启动 Nginx 与 Rust。覆盖镜像 command 时，Tini 会直接执行指定命令，
不会先启动 supervisor；因此上面的迁移命令只运行 `exchange-migrate`，不会启动 Nginx。

## 启动回归镜像验收

仓库没有独立的容器轻量测试框架。发布前可用完整 Compose 执行以下回归验收；临时覆盖
会故意注入旧监听变量并启用外层 Docker init：

```bash
docker build --tag rust-chain:startup-regression .

startup_override="$(mktemp)"
cat >"$startup_override" <<'YAML'
services:
  api:
    init: true
    environment:
      APP_HOST: 0.0.0.0
      APP_PORT: "8080"
YAML

export EXCHANGE_IMAGE=rust-chain:startup-regression
export API_PORT=18080
docker compose \
  --project-name exchange-startup-regression \
  --env-file docker-compose.env.example \
  -f docker-compose.example.yml \
  -f "$startup_override" \
  up --detach --wait

curl --fail http://127.0.0.1:18080/health
docker compose \
  --project-name exchange-startup-regression \
  --env-file docker-compose.env.example \
  -f docker-compose.example.yml \
  -f "$startup_override" \
  exec --no-TTY api bash -Eeuo pipefail -c '
    grep --quiet "0100007F:1F91" /proc/net/tcp
    grep --quiet "00000000:1F90" /proc/net/tcp
    ! grep --quiet "00000000:1F91" /proc/net/tcp
  '
if docker compose \
  --project-name exchange-startup-regression \
  --env-file docker-compose.env.example \
  -f docker-compose.example.yml \
  -f "$startup_override" \
  logs api | grep --fixed-strings "Tini is not running as PID 1"; then
  exit 1
fi

docker image inspect rust-chain:startup-regression \
  --format '{{json .Config.Entrypoint}} {{json .Config.Cmd}}'
docker run --rm rust-chain:startup-regression \
  /bin/bash -Eeuo pipefail -c 'test ! -e /tmp/exchange-nginx/nginx.pid'

docker compose \
  --project-name exchange-startup-regression \
  --env-file docker-compose.env.example \
  -f docker-compose.example.yml \
  -f "$startup_override" \
  down --volumes
rm -f "$startup_override"
```

监听断言中的 `/proc/net/tcp` 十六进制地址分别对应 `127.0.0.1:8081`、
`0.0.0.0:8080` 和禁止出现的 `0.0.0.0:8081`。镜像检查应显示入口为
`["/usr/bin/tini","-s","--"]`，默认命令为
`["/usr/local/bin/exchange-supervisor"]`；command 覆盖检查则确认不会生成 Nginx PID
文件。

## 私有 GHCR 包登录

公开包可直接拉取。私有包需要具有 `read:packages` 权限的 GitHub personal access token：

```bash
printf '%s' "$GHCR_TOKEN" | docker login ghcr.io --username YOUR_GITHUB_USER --password-stdin
docker pull ghcr.io/jacqueshuang-fresnostate/rust-chain:1.2.3
```

GitHub Actions 发布流程使用仓库自带的 `GITHUB_TOKEN`，不需要保存额外 GHCR 密钥。

## 密钥与持久化

提交到仓库的环境文件只包含占位值，复制后的 `docker-compose.env` 和
`docker-compose.1panel.env` 已被 `.gitignore` 和 `.dockerignore` 排除。部署前至少替换
以下值：

- MySQL root/应用密码、MongoDB root 密码、RabbitMQ 密码。
- `JWT_SECRET`：使用高熵随机值。
- `CREDENTIAL_ENCRYPTION_KEY`：必须恰好为 32 字节。该值用于 AES-256-GCM，
  一旦保存过加密凭据就必须保持稳定，否则旧数据无法解密。
- `BOOTSTRAP_ADMIN_PASSWORD`：内置公开默认值为 `Qaz123456@`，生产初始化前应覆盖；只提供
  给一次性迁移容器，不得加入 API 环境。

不要提交实际的环境文件。生产环境应从受控的 secret manager 或受限权限文件注入这些值。
完整 Compose 需定期备份 `mysql-data`、`mongo-data`、`redis-data`、`rabbitmq-data` 和
`uploads-data` 命名卷；1Panel Compose 只拥有 `hippo-exchange-uploads`，第三方服务数据
应通过各自的 1Panel 应用或数据库备份策略处理。更换数据库或消息队列密码时，需要同步
更新连接 URL。

Compose 把 `uploads-data` 挂载到应用容器的 `/app/uploads`。如果后台启用可选的本地上传
provider，请把其 `local_root` 精确配置为 `/app/uploads`，并把 `public_base_url` 配置为
同域的 `/uploads/` 路径；镜像内 Nginx 会直接提供该目录。未使用本地 provider 时，这个卷
保持为空。绑定宿主机目录时，必须确保 UID/GID `10001:10001` 对目录具有读写权限。
