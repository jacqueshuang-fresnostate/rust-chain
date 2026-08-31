# Admin 与 Mobile 前端完善最终关闭记录（2026-09-01）

## 范围与结论

- 实施范围：`web/**`、`mobile/**` 及必要的 CI/spec/task/progress 文档。
- 明确排除：`pc/**`；最终 `git status --short -- pc/` 为空。
- Admin：FAD-P0-01、FAD-P1-01..07、FAD-P2-01..04 共 **12/12 关闭**。
- Mobile：FMD-P1-01..07、FMD-P2-01..05 共 **12/12 关闭**；Trade/Seconds 最后一条
  端到端 Decimal DTO 阻塞已由 Phase A/Phase B 关闭。
- 本地代码和制品门禁结论：通过。真实生产依赖的运行时 smoke 单列在文末，不作为代码
  缺陷伪装或替代自动化证据。

## Admin 关闭证据

1. 资金幂等意图可恢复、按规范化 Decimal 和会话隔离；timeout/response-drop/reload 复用
   同一 key，明确成功或失败才完成意图。
2. session owner、generation/CAS、跨标签退出、站内 redirect、登录/2FA 单次 mutation 和
   Turnstile 重试生命周期已收口。
3. read/review/operate/write/settle 精确权限矩阵覆盖资源动作与独立页面；严格 DTO、共享
   可取消目录查询、共享行情 lease/watchdog/freshness 和 ARIA Tabs 均进入测试。
4. 行动域按需加载，中文标题与显式 API 环境配置已落地，并建立 coverage、生产策略和
   raw/gzip bundle 预算。

最终门禁：lint、typecheck、60 个测试文件 **430/430**、production-policy **14/14**、
coverage、build、budget 全部通过；覆盖率为 statements 85.61%、branches 81.78%、
functions 85.33%、lines 92.87%。初始 JS 1,632,551 B raw / 444,748 B gzip，最大异步
Quill 205,074 B / 60,072 B，总 JS 2,352,107 B / 673,168 B，CSS 593,245 B /
72,995 B，均低于门限。

Ego Browser：登录页及 `/admin/users` 在 1440/1024/768px 无根级溢出或破图；选中侧栏为
不透明深色背景+白字；9 列资源表每列有 resize handle，第一列实拖 160px→208px；操作
按钮未换行。截图：`/private/tmp/admin-users-final-regression-fixed.png`。

## Mobile 关闭证据

1. 行情冷启动 single-flight、lease/freshness、会话 logout-wins、Orders generation/Abort、
   稳定错误码和私有 WS topic lease/watchdog/REST 对账已完成。
2. 现货、杠杆、秒合约、闪兑、借贷、理财、预测、新币、钱包 mutation 及 wallet/trading/
   ticker/depth DTO 使用 DecimalText 权威；覆盖 `9007199254740993.000000000000000001`、
   `0.000000000000000001`、JSON number 与指数形式拒绝。
3. 批量撤单消费 `orders[]/failures[]`；未知业务枚举保留未知语义；KYC Blob URL 生命周期、
   PWA 更新/安装恢复、Tauri CSP、路由 title/focus/announcement、Seconds 键盘 listbox 和
   44px 触控目标已进入门禁。
4. 首屏舞台资源由约 1.7MB PNG 替换为约 100.2KiB WebP；建立 PWA/Tauri 制品、bundle、
   source-size 与行为测试治理预算。

最终 `npm --prefix mobile run release:gate`：应用/测试类型检查、全量 **607/607**、PWA 和
Tauri 双构建、制品断言及全部预算通过。PWA JS 1355.4KiB raw / 458.6KiB gzip，入口
438.0KiB / 149.4KiB；Tauri JS 1350.6KiB / 457.2KiB，入口 432.9KiB / 147.9KiB；CSS
591.3KiB / 111.3KiB，均低于门限。

Ego Browser：320/390/448px 的首页、现货、秒合约，以及 390px 的订单、资产、KYC 登录
回跳均无文档横向溢出、破图或重复 ID；转场完成后关键路由均为单一
`main#main-content`，title 与 announcement 正确。截图：
`/private/tmp/mobile-final-after-decimal.png`。

## 部署环境补证（非代码阻塞）

- 使用真实 Cloudflare Turnstile site/secret 做慢网、token 过期与失败重试 smoke。
- 使用真实公网 REST/WS 做行情静默重连、私有 topic 断线恢复和权限五角色 smoke。
- 在已安装 PWA、签名 Android/iOS/Desktop 制品上做更新、离线、读屏和低端设备性能 smoke。
- 上述项目属于部署/设备证据，不改变本次本地 release gate 已通过的结论。
