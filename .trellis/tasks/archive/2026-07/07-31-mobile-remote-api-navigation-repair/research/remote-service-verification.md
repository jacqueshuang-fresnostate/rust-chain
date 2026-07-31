# 远程服务验证记录

验证日期：2026-07-31

目标服务：`https://hipoex.cllbmz.kdns.fr`

## 实测结果

| 检查项 | 请求 | 结果 |
| --- | --- | --- |
| 市场列表 | `GET /api/v1/markets` | HTTP 200，返回 BTC-USDT 等市场数据 |
| 登录配置 | `GET /api/v1/auth/login/config` | HTTP 200 |
| Tauri CORS 预检 | `OPTIONS /api/v1/auth/login`，`Origin: tauri://localhost` | HTTP 200，允许来源、方法和请求头 |
| Tauri CORS 实际请求 | `GET /api/v1/markets`，`Origin: tauri://localhost` | HTTP 200，返回 `access-control-allow-origin: *` |
| 公共行情 WebSocket | Upgrade `/api/v1/ws/public` | HTTP 101 Switching Protocols |
| 健康检查 | `GET /health` | HTTP 403，Cloudflare Managed Challenge |

## 结论

- 业务 API、跨域访问和 WebSocket 满足手机端 PWA/Tauri 直连要求。
- `/health` 的 403 来自 Cloudflare 挑战，不代表业务 API 不可用。
- 手机端不能把 `/health` 作为启动或展示业务页面的前置条件。
- 手机端实际市场列表端点为 `/api/v1/markets`，不是
  `/api/v1/markets/tickers`。
