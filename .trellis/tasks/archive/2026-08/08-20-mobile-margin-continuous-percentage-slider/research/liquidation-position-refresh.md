# 杠杆强平后的手机端持仓同步

## 根因

- 强平由后端 `margin_liquidation` worker 异步执行，成功后把 `margin_positions.status` 从 `opened` 原子迁移为 `liquidated`。
- `GET /api/v1/margin/wallets` 通过 `list_user_margin_wallets` 返回当前杠杆钱包，以及仅筛选 `status = 'opened'` 的持仓，因此它可以作为工作台账户状态的 REST 权威来源。
- `TradeView.vue` 首次加载和用户主动下单/平仓/撤单后会调用 `fetchMarginWallets()`，但现有 5 秒定时器只调用 `loadMarginPositionRisks()`。worker 强平不是当前页面发起的 mutation，所以本地 `marginPositions` 不会再次替换，终态仓位会永久残留。

## 修复设计

1. 保留 `/margin/wallets` 为钱包与活动持仓的唯一对账来源，不根据风险接口的 404 或 `shouldLiquidate` 在客户端猜测终态。
2. 杠杆页面连接 `/api/v1/ws/private?token=<access-token>`；服务端根据 token 自动订阅唯一 `private:user:<user_id>`，客户端不发送自定义频道。收到 `margin.position.liquidated` 只触发 REST 对账，不直接相信事件金额或在本地拼资金结果。
3. 私有广播是进程内、易失且不重放的提示。连接首次打开、断线重连、页面恢复可见时都立即对账，同时保留每 5 秒的静默 REST 兜底，确保 API 重启或断线期间最终收敛。
4. 私有连接使用文本 `ping`/`pong` 保活、当前 socket 身份保护、有界指数退避和幂等停止；退出、切到现货或卸载必须清理 socket、心跳和重连任务。重连读取最新持久化 access token，兼容 HTTP 刷新令牌后 Pinia 值尚未同步的窗口。
5. 周期任务使用单飞门禁，避免慢网下叠加请求；显式 mutation 后刷新仍可创建更新版本并让旧周期结果失效。
6. 请求生命周期至少绑定访问令牌、当前交易模式和组件存活状态，防止退出、换号、contract→spot→contract ABA 或卸载后的迟到响应回写。
7. 静默失败保留最后一次成功状态；首次加载失败仍使用既有错误 UI。页面恢复可见时立即对账，以覆盖移动端挂起期间错过的 worker 状态变化。
8. 风险快照始终按最新活动持仓 ID 裁剪，已不在权威响应中的强平仓位必须同时丢弃风险缓存。

## 不采用的表面修复

- 只在风险接口失败时从数组删除仓位：网络错误和不支持风险接口都可能失败，不能证明仓位已强平。
- 仅缩短风险轮询间隔：风险响应不包含完整钱包/活动持仓集合，无法修复状态源缺失。
- 只依赖 `margin.position.liquidated` 私有 WebSocket：该广播是进程内、无历史和无重放的提示，断线或 API 重启会漏事件；接入后仍必须回到 REST 对账。

## Break-loop 分析

### 1. 根因分类

- **B：跨层合同缺失**。后端异步 worker、私有事件、账户 REST 快照与 Mobile 页面各自可用，但页面把“刷新风险指标”误当成“刷新账户状态”，没有定义异步终态如何传播并最终收敛。
- **D：测试覆盖缺口**。此前测试只锁定 5 秒风险请求，没有覆盖强平由页面外部发生、私有推送丢失、账号/模式 ABA 和迟到响应回写。

### 2. 之前表面修复为何失效

1. 只定时调用单仓风险接口：该接口没有活动持仓全集，也没有钱包快照，无法证明仓位终态或资金结果。
2. 只在用户主动下单/平仓后刷新：worker 强平不是当前页面发起的 mutation，因此不会进入这些刷新点。
3. 只增加实时事件：进程内广播不持久化、不重放，服务重启和移动端挂起会漏消息，不能承担金融状态真源。

### 3. 预防机制

| 优先级 | 机制 | 具体动作 | 状态 |
| --- | --- | --- | --- |
| P0 | 架构 | 事件只作刷新提示，`/margin/wallets` 统一返回钱包与 opened 持仓权威快照 | DONE |
| P0 | 运行时 | 私有 WS 的 open/reconnect/event 与 5 秒单飞轮询共同触发 REST 对账 | DONE |
| P0 | 竞态防护 | 请求绑定 generation、token、mode、visibility 与组件生命周期 | DONE |
| P1 | 测试 | 覆盖漏消息兜底、繁忙提示合并、后台失败保留、退出/换号/ABA/卸载迟到响应 | DONE |
| P1 | 文档 | 将私有提示与账户对账合同写入 backend/mobile 规范和索引 | DONE |

### 4. 系统性扩展

- 其他由 worker 或管理员异步改变的资金页面也应检查是否只刷新“局部详情”而没有重新读取“集合/账户权威快照”。
- 任何进程内 WebSocket 事件都应明确标注是 **hint** 还是 **state**；当前项目的私有广播统一按 lossy hint 处理。
- 显式 mutation 后的前台刷新和后台轮询必须共享同一对账函数，但拥有不同 loading/error 语义，避免两套映射长期漂移。

### 5. 知识沉淀

- 已更新 `.trellis/spec/backend/realtime-websockets.md`。
- 已更新 `.trellis/spec/mobile/backend-integration.md` 与 `.trellis/spec/mobile/pwa-and-shell.md`。
- 已更新 backend/mobile 规范索引，后续私有推送或异步资金状态改动必须先读取这些合同。
