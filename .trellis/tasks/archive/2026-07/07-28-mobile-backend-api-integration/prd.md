# 移动端真实后端接口联调

## Goal

让 `mobile/` 的 PWA、开发环境与 Tauri 原生构建都通过统一且可部署的配置连接当前 Rust 后端，修复 HTTP、鉴权刷新、首次强制 2FA 和行情 WebSocket 的实际协议偏差，并以真实启动的后端完成接口冒烟验证。

## What I already know

- Rust 用户接口统一挂载在 `/api/v1`，健康检查位于 `/health`。
- 移动端已经按业务域拆分 API 文件，但生产构建在没有环境变量时回退到 `http://127.0.0.1:8080`。
- 该回退对公开 PWA 和真实手机都不成立，因为它会访问终端设备自身。
- 移动端行情 WebSocket 当前使用 `/ws/public`，开发服务器只代理 `/api/v1`，因此开发模式不能稳定连接。
- Rust 后端同时提供 `/api/v1/ws/public` 以及业务别名，并接受 JSON 订阅命令和文本 `ping`。
- 强制登录 2FA 模式会为未绑定 TOTP 的用户返回 setup challenge，但当前没有公开的 challenge setup/confirm 接口。
- 静态审计确认移动端所有 HTTP 路径与方法均存在；剩余问题集中在配置、鉴权边界和响应适配。
- PWA 不得缓存 API、鉴权、钱包、订单或 WebSocket 数据。
- MySQL、MongoDB、Redis 和 RabbitMQ 本地依赖可用；后端可在独立端口启动，避免误接其他服务。

## Assumptions

- PWA 默认采用同源反向代理部署，生产 API 域名可通过 `VITE_BACKEND_API_DOMAIN` 覆盖。
- Tauri 生产构建必须显式提供可从设备访问的 HTTPS 后端域名，不再静默使用 loopback。
- 后端改动仅限补齐首次强制 2FA challenge 的最小公开契约，不修改其他业务规则和数据库结构。
- 不在 service worker 中增加任何金融接口缓存或离线提交队列。

## Requirements

- HTTP URL、WebSocket URL 和健康检查 URL 由同一套运行时配置生成。
- PWA 未配置域名时使用当前页面 origin 与 `/api/v1`。
- 开发模式通过 Vite 同源代理支持 HTTP 和 WebSocket。
- Tauri 构建通过环境变量连接真实后端；缺失配置时提供可诊断错误，不回退到设备 loopback。
- 鉴权请求继续发送 Bearer token，401 只刷新一次，刷新失败后清理会话。
- 登录、注册、2FA、找回密码和 refresh 请求不携带旧 Bearer token，也不触发刷新重放。
- 首次强制 2FA challenge 必须支持生成 TOTP secret/QR、确认验证码、消费 challenge 并签发用户 token。
- 现货、合约、秒合约、钱包、借贷、理财、安全中心和公共行情端点的路径、方法及关键 DTO 与 Rust 路由一致。
- 新币购买必须锁定后端交易对的计价资产，并按计价余额除以执行价计算可买数量。
- 新闻详情保留后端支持的安全富文本与图片块；消息中心明确为平台公告，不伪装成个人站内信。
- 预测订单展示后端 `order_no`；历史合约仓位通过稳定产品/交易对映射显示名称。
- 访客现货页面不请求需要登录的合约产品接口。
- 网络错误信息需区分未配置、超时、断网和后端返回错误。
- 为 URL 解析、代理、WebSocket 订阅和关键接口契约增加自动化测试。

## Acceptance Criteria

- [x] PWA 生产构建不再包含 `http://127.0.0.1:8080` 作为默认 API 地址。
- [x] 开发环境的 `/api/v1/*` HTTP 和 `/api/v1/ws/*` WebSocket 都能代理到 Rust 后端。
- [x] 移动端 WebSocket 能完成订阅确认、接收 ticker、心跳和重连。
- [x] 首次强制 2FA 用户能在登录 challenge 内完成绑定并获得 token，不再返回登录页循环。
- [x] 鉴权 bootstrap 401 不触发 refresh；受保护请求仍只刷新并重放一次。
- [x] 所有移动端 API 路径均能在 Rust 用户路由中找到，或有明确兼容说明。
- [x] 真实后端启动后，健康检查、公共配置、市场、新闻、受保护接口 401 和 WebSocket 冒烟验证通过。
- [x] 移动端测试、类型检查、PWA 构建和 Tauri Web 构建通过。
- [x] PWA 产物的 service worker 不缓存 API/WebSocket 响应。
- [x] 进度、移动端接口配置契约和任务记录已更新。

## Definition of Done

- 自动化测试覆盖新增配置与协议分支。
- `npm --prefix mobile test`、`type-check`、`build:pwa`、`build:tauri` 通过。
- 最贴近改动的 Rust 路由/WebSocket 测试和真实服务冒烟通过。
- `docs/superpowers/PROGRESS.md` 记录完成内容与验证结果。
- 变更提交，Trellis 任务归档并记录开发日志。

## Out of Scope

- 首次强制 2FA challenge 以外的后端业务规则、数据库 schema、管理端和 PC 端功能改造。
- 使用虚构账户完成真实资产写入或交易。
- 在 service worker 中缓存市场、钱包、订单或用户数据。
- 本任务中部署或配置正式生产反向代理、DNS、TLS 与密钥。

## Technical Notes

- 前端入口：`mobile/src/config/app.ts`、`mobile/src/api/client.ts`、`mobile/src/api/marketSocket.ts`、`mobile/vite.config.ts`。
- 后端路由入口：`src/lib.rs`，用户路由统一 nest 到 `/api/v1`。
- WebSocket 协议：`src/modules/events/routes.rs`、`src/modules/events/service.rs`、`src/modules/events/presentation.rs`。
- PWA 约束：`.trellis/spec/mobile/pwa-and-shell.md`。
- 鉴权约束：`.trellis/spec/backend/auth-sessions.md`。

## Research References

- `research/mobile-backend-contract-audit.md`：移动端端点、DTO、鉴权和 WebSocket 对照审计。
