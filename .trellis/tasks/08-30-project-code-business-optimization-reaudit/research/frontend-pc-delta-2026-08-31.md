# Research: PC 前端 CURRENT HEAD 增量复审（2026-08-31）

- Query: 以 2026-08-30 `admin-pc-cross-layer.md` 为基线，仅复审当前 PC 前端（`pc/src`、`pc/tests`、`pc/package*`、Vite 与 Tauri 配置），识别仍存在或新识别的 routing/auth、API DTO、Decimal、请求错误/超时、WebSocket 生命周期与陈旧态、query/state ownership、i18n、无障碍、组件规模、测试/覆盖、生产构建和 Tauri 安全/更新交付问题。
- Scope: internal（当前检出内容的静态审计 + 无写入本地验证；未访问生产服务、未打包/启动 Tauri、未修改生产代码或既有研究）
- Date: 2026-08-31

## Findings

### 1. 口径、delta 定义与摘要

- 优先级沿用任务 PRD：P0 仅用于可直接影响资金、结算、价格时点或不可恢复数据正确性的风险；P1 用于业务可用性、跨进程可靠性和显著维护/交付风险；P2 用于体验、一致性、无障碍及长期治理（`.trellis/tasks/08-30-project-code-business-optimization-reaudit/prd.md:15-20`）。
- **延续**：2026-08-30 报告已记录且当前代码仍能静态证明；**新增识别**：该具体缺口未在 2026-08-30 PC 条目中展开，不等同于能够证明它是在 8 月 30 日之后提交；**已验证改善/未发现**：当前检查未复现旧疑点。
- 证据标签：
  - **确认的静态缺陷**：当前类型、控制流、模板或配置已足以证明缺口。
  - **本地确定性复现**：用 mock/无写入构建在本轮直接复现。
  - **运行时假设**：触发频率、具体 HTTP 错误、真实资金结果或平台行为仍需浏览器、后端 fixture、生产制品或网络故障注入确认。
- 本轮保留/新增 **4 项 P0、7 项 P1、3 项 P2**。新增最高优先级是充值地址选择的乱序响应可把旧地址/二维码置于新网络标签下，以及多数资金写请求在 10 秒超时后的人工重试会换幂等键。2026-08-30 的杠杆风险读模型与秒合约结算证据两个 P0 均未闭环。

### 2. 2026-08-30 PC 条目 delta 映射

| 2026-08-30 条目 | 当前状态 | 本轮定位 |
|---|---|---|
| APC-P0-01 杠杆页面绕过服务端权威风险 | **延续** | PCD-P0-03 |
| APC-P0-02 秒合约结算价/证据被丢弃 | **延续** | PCD-P0-04 |
| APC-P1-01 提现缺 `quote_id` | **延续** | PCD-P1-01 |
| APC-P1-02 错误、陈旧态与伪分页 | **延续** | PCD-P1-02 |
| APC-P1-04 会话双事实、redirect、Turnstile 生命周期 | **延续** | PCD-P1-03 |
| APC-P1-05 WebSocket 无沉默检测/freshness | **延续，并新增确定性 ABA 复现** | PCD-P1-04 |
| APC-P1-06 DTO/测试门禁不足 | **延续；测试文件增至 22 个，但全量直跑 5 项失败** | PCD-P1-06 |
| APC-P1-07 Decimal 经 `Number` | **延续** | PCD-P1-05 |
| APC-P1-09 Tauri 构建/更新链路 | **延续；新增确认 updater artifact 默认关闭** | PCD-P1-07 |
| APC-P2-01 guest 秒合约公开目录被认证墙挡住 | **延续** | PCD-P1-03 的次级影响 |

---

### PCD-P0-01 — 充值币种/网络请求没有 generation，旧响应可覆盖新选择并显示错误地址/二维码

- **Delta / 优先级**：新增识别；P0（充值地址与网络标签错配可能造成不可恢复的链上转账损失）。
- **证据类型**：乱序写入路径是**确认的静态缺陷**；用户是否在生产延迟窗口内提交到错误链是**运行时假设**。
- **代码证据**：
  - `selectCoin` 先写全局可变 `selectedCoin`，再等待网络列表；响应回来后无请求序号、AbortSignal 或当前币种复核，直接覆盖 `availableNetworks`，并且不等待地调用 `selectNetwork`（`pc/src/views/User/Recharge.vue:273-300`）。因此 A 币请求晚于 B 币返回时，A 的网络列表会覆盖 B 的当前选择。
  - `selectNetwork` 先写 `selectedNetwork/selectedNetworkKey`，随后用当时的全局 `selectedCoin.value` 请求地址；响应同样无 generation/coin/network 复核，直接覆盖 `walletData` 并异步生成二维码（`pc/src/views/User/Recharge.vue:302-345`）。同币种快速从网络 A 切到 B 时，A 的慢响应可最后提交。
  - 模板从 `walletData.address`/`qrCodeUrl` 渲染地址和二维码，但说明文字来自可变的 `selectedCoin/selectedNetwork`（`pc/src/views/User/Recharge.vue:62-99`），`copyAddress` 也会复制当前 `walletData`，所以状态来源并非一个原子快照。
  - Withdraw 存在同型选择竞态：币种网络列表和网络详情响应直接写共享状态，没有 superseded-request 检查（`pc/src/views/User/Withdraw.vue:331-376`）。
  - Vue Query 已注册（`pc/src/main.ts:8,60-63`、`pc/package.json:18`），但 `pc/src` 中没有 `useQuery/useMutation/QueryClient` 使用；目前没有由 query key 提供的取消、去重或“仅当前选择可提交”所有权。
- **影响**：最危险场景是界面标题/警告显示网络 B，地址与二维码却来自网络 A；用户按 B 链转账到 A 地址可能永久丢失资金。较轻场景是旧网络列表覆盖、loading 被旧请求提前关闭、二维码与文本短暂不一致。
- **整改**：
  1. 把 `{assetSymbol, networkKey}` 建成不可变选择快照和 query key；每次选择递增 generation，并 Abort/取消前一请求。
  2. 只有响应 key 与当前 key 完全相同时才提交 `availableNetworks/walletData/qrCodeUrl/loading`；QR 生成也带同一 generation。
  3. 地址、二维码、网络标签、币种、最小额和 fee 必须从同一个 `DepositAddressViewModel` 渲染；当前请求未完成时禁用 copy/QR。
  4. Withdraw 复用相同 selector controller，而不是再维护一套共享可变流程。
- **验证**：组件测试使用 deferred promises：依次选择 A→B，让 B 先返回、A 后返回，断言最终 DOM/copy 值/QR payload 全部只属于 B；分别覆盖跨币种、同币多网络、QR promise 乱序、取消、失败后旧数据保留但标陈旧。端到端使用两个可识别 fixture 地址，任何时刻标签与地址 key 必须一致。

### PCD-P0-02 — 多数资金 mutation 每次调用生成新幂等键，10 秒超时后的同意图重试不安全

- **Delta / 优先级**：新增识别；P0（可重复开仓、下注、申购、借款或提现意图）。
- **证据类型**：客户端不能保持 retry key 是**确认的静态缺陷**；服务器在响应丢失前是否已提交、第二请求是否形成第二笔资金动作是**待故障注入的运行时结果**。
- **代码证据**：
  - 所有请求共用 10 秒 timeout（`pc/src/api/request.ts:147-156`）；超时没有 HTTP status，拦截器只处理 401/403/精确 500 后继续 reject（`:179-207`）。服务器在 10 秒前提交、响应在 10 秒后到达时，客户端无法判断结果。
  - 已有正确 helper 会按规范化业务意图保留失败键、只在成功后释放（`pc/src/api/idempotency.ts:14-44`）；当前只用于 spot 下单（`pc/src/api/exchange.ts:14,33-43`）和 margin transfer（`pc/src/api/contract.ts:17,140-154`），且缓存只在内存中，页面 reload 后仍丢失。
  - 每次调用换键的资金路径：margin open（`pc/src/api/contract.ts:72-80,260-262`）、seconds 下单（`pc/src/api/second.ts:53-61,159-160`）、earn 申购（`pc/src/api/finance.ts:76-88,123-125`）、loan 申请（`pc/src/api/loan.ts:139-147`）、new-coin 申购（`pc/src/api/activity.ts:73-89,243-245`）、prediction 下单（`pc/src/views/Prediction.vue:417-436`）。提现页面不提供 key（`pc/src/views/User/Withdraw.vue:434-448`），mapper 每次随机生成（`pc/src/api/backendAdapters.ts:1245-1256`）。
  - 当前行为测试只验证 helper、spot 和 margin transfer 的源码顺序（`pc/tests/idempotency.test.ts:7-33`）；上述其他 mutation 没有“响应丢失后重放同 key”的测试。
- **影响**：超时/断网后按钮重新可用，用户按原参数重试时发出不同 key；如果首笔已提交，后端会把第二 key 视为新业务意图。对 margin/seconds/earn/loan/new-coin 等可直接产生二次扣款或风险敞口。Prediction 是否因 quote 单次消费而被额外保护、withdraw 是否因 quote/state machine 被保护，需要各端点 fixture 证明，客户端目前没有统一保证。
- **整改**：为每个资金命令创建持久化 mutation intent（稳定序列化业务字段 + client intent ID + idempotency key）；网络失败/timeout/5xx 保留 key，成功或用户明确改变意图才 rotate。超时后先用 key/订单查询接口 reconciliation，再允许新意图。对 reload/崩溃恢复，至少把 pending intent 安全地持久化到 session-scoped storage，并设置过期/用户隔离。
- **验证**：每个 mutation 做 commit-before-timeout、response-drop、offline-after-send、刷新页面、双击、同 key 异参冲突测试；断言同一意图所有重试 key 相同、最终资金/订单仅一笔，参数改变才产生新 key。把这些测试加入标准全量脚本而非源码正则。

### PCD-P0-03 — 杠杆风险/PnL 仍由客户端旧钱包模型拼装，服务端权威 cross/risk 数据未进入 UI

- **Delta / 优先级**：延续 APC-P0-01；P0。
- **证据类型**：**确认的静态缺陷**；实际误差大小依赖多仓、利息、mark 与生产数据，需运行时 fixture 量化。
- **代码证据**：
  - PC 已声明 `BackendMarginCrossAccount`，`BackendMarginWalletsResponse` 也允许 `cross_accounts`（`pc/src/api/backendAdapters.ts:774-788`），但 `mapMarginWalletsToContractWallets` 只拼 `wallets + positions`，没有消费 cross account（`:1616-1659`）。
  - position mapper 把 entry price 当 current price、realized PnL 当总 PnL，并把 fee/maintenance/risk 字段设成 0/null（`:1883-1918`）；store 又把 0 fee/rate 替换成 0.0001/0.005（`pc/src/stores/contract.ts:272-327`）。
  - UI 遍历全部 wallet/position，自行以 WS thumb 计算 PnL、margin rate 和 cross rate（`pc/src/components/trade/ContractOrders.vue:237-321`）；若 WS thumb 不存在，会回退 mapper 的 entry price（`:239-242`），看起来像实时零盈亏而不是“价格陈旧/不可用”。
  - 额外 scope 假设：表格展示全部产品的 `contractStore.wallets`（`:237-321`），而“全部平仓/撤单”只传当前 `activeCoinId`（`:417-477`）。静态上可确认可见集合与 action scope 不同；“close all”产品口径是否预期为当前产品仍需产品规范确认。
- **影响**：用户可能看到与服务端强平/转账事实源不同的 equity、未实现盈亏和保证金率；失去行情时还会显示无陈旧提示的零 PnL。批量动作若被理解为表格全量，则可能遗留其他可见杠杆仓位。
- **整改**：建立强类型 authoritative margin read model，原样保留 decimal strings、risk snapshot 与 `observed_at`；cross 直接消费 account snapshot，单仓消费 risk endpoint，删除客户端权威公式与费率默认。可见列表与批量 action 必须使用同一个显式 scope（当前产品或全账户）并在按钮文字中说明。
- **验证**：同保证金币种的多 pair、多空混合、利息、部分行情缺失/陈旧 fixture；逐字段比对后端 snapshot，任何缺失/过期 mark 不得回退 entry price 或显示绿色 live。增加表格集合与 close-all request scope 一致性测试。

### PCD-P0-04 — 秒合约响应仍丢弃 settlement price/行情证据并在 JS 本地重算 profit

- **Delta / 优先级**：延续 APC-P0-02；P0。
- **证据类型**：`closePrice` 恒 0 是**确认的静态缺陷**；本地 profit 与真实 ledger 差值需后端 fixture。
- **代码证据**：`BackendSecondsOrder` 仍没有 settlement price、tick/source/observed/version 等结算字段（`pc/src/api/backendAdapters.ts:655-675`）；mapper 固定 `closePrice: 0`（`:1515-1535`），并根据 stake × payout rate 本地计算 profit（`:1847-1850`）。金额在 store 再次经 `Number`（`pc/src/stores/second.ts:246-263`）。这与 seconds spec 要求持久化精确 settlement price、按资产 precision 结算相冲突（`.trellis/spec/backend/seconds-contracts.md:69-109,141-152`）。
- **影响**：历史订单无法审计结果使用的价格/来源/观测时点；本地浮点和后端精度截断可能显示不同收益。
- **整改**：transport DTO 原样保留 settlement evidence 与服务端 payout/net PnL decimal text；未结算/旧记录缺字段显示 `--/未知`，不得显示 0 或当前行情。结果页提供 source、observed time 和 ledger 对账标识。
- **验证**：win/loss/未结算/旧 payload、边界精度、结算行情来源与陈旧性 fixtures；断言 PC entry/settlement/payout 与后端 order/ledger 逐字符一致。

---

### PCD-P1-01 — 提现 DTO 仍缺服务端 quote，网络目录、费用和金额模型仍是客户端旧契约

- **Delta / 优先级**：延续 APC-P1-01；P1（服务端应在冻结前拒绝，因此目前不是错误动账 P0）。
- **证据类型**：**确认的静态契约断裂**；实际 HTTP status/message 待本地集成。
- **代码证据**：PC 的 backend/Pc 请求类型无 `quote_id`，且 Pc amount/fee 是 `number`（`pc/src/api/backendAdapters.ts:124-145`）；mapper 只将本地 amount/fee 转字符串并生成随机 key（`:1245-1256`），`submitWithdraw` 直接 POST（`pc/src/api/wallet.ts:286-294`），页面也不获取 quote（`pc/src/views/User/Withdraw.vue:434-448`）。现有 adapter test 明确断言无 quote 的旧 payload（`pc/tests/backendAdapters.test.ts:431-469`）。跨层必填 quote 契约及冻结前校验证据见 2026-08-30 基线 `admin-pc-cross-layer.md` 的 APC-P1-01。
  - Withdraw 网络不会调用 backend deposit-network catalog；`purpose === 'withdraw'` 直接落入内建网络列表（`pc/src/api/wallet.ts:163-187,297-306`），未知字符串还会默认映射为 ETH（`:308-319`）。
  - fee tiers 和限制先转 `Number`，PC 本地算费（`:389-428`），与 quote 权威费率/版本不构成同一确认快照。
- **影响**：当前后端会拒绝 PC 提现；页面展示的 network、fee、net amount 与提交时服务器配置没有原子保证。未知 network 降为 ETH 还可能制造错误预览。
- **整改**：增加 withdrawal quote state machine：服务器目录→输入 decimal string→quote（含 fee/net/expiry/config version）→确认时提交同一 `quote_id` 与参数。移除生产内建网络 fallback 和未知→ETH；旧客户端在 quote 不可用时禁用提交而非降级。
- **验证**：正常、过期、网络禁用、配置版本变化、金额变化、重复点击、未知网络 fixtures；PC 请求必须含 quote_id，失败时零冻结/零 ledger。

### PCD-P1-02 — 请求错误仍被吞成空/旧数据，latest-N 被包装成分页；没有统一 query ownership

- **Delta / 优先级**：延续 APC-P1-02；P1。
- **证据类型**：错误控制流与页参数丢失是**确认的静态缺陷**；真实重复页数量/暴露频率待数据 fixture。
- **代码证据**：
  - HTTP interceptor 只有 401、403、精确 500 的 toast；网络错误、timeout、400/409/422/429/502/503/504 不形成统一 typed error（`pc/src/api/request.ts:179-207`）。
  - margin loads catch 后只写 console，无 `error/stale/lastSuccessfulAt`（`pc/src/stores/contract.ts:117-218,221-327`）；seconds 同样（`pc/src/stores/second.ts:155-184,187-231,286-305`）；spot 订单加载失败也只 console（`pc/src/components/trade/OrderHistory.vue:194-218`）。旧列表因此继续显示但没有陈旧标记，首次失败又与真正空数据相同。
  - seconds API 接受 `pageNo/pageSize` 但完全不发送，始终读取同一个 `/orders` 列表（`pc/src/api/second.ts:31-35,74-76,99-105`）；store 在 page > 0 时追加结果并以长度猜测 hasMore（`pc/src/stores/second.ts:187-226`）。margin history 固定 page 1/50，但 API 同样读取全量后本地过滤（`pc/src/stores/contract.ts:221-258`、`pc/src/api/contract.ts:233-242`），且 `createTime` 仍固定 0（`pc/src/api/backendAdapters.ts:1585-1603`）。
  - Vue Query plugin 注册但零使用（`pc/src/main.ts:8,62`；全树未发现 `useQuery/useMutation`），缓存、取消、请求去重、retry policy、staleTime 和错误边界均散落在页面/store。
- **影响**：断网/超时可能被解释为“没有订单”，或继续展示旧资金状态却看起来仍是 live；翻页可重复最新记录且访问不到更老历史；多个组件并发加载会发生 last-response-wins。
- **整改**：统一 `AsyncResource<T> = {data,status,error,lastSuccessfulAt,requestKey}`；失败保留数据并显式 stale，不以空数组替代。读请求按业务设 timeout/retry/cancel；资金 mutation 禁止自动重试并走 PCD-P0-02 reconciliation。后端支持真实 cursor 前移除伪 load-more，随后由 query key 持有 cursor 与筛选。
- **验证**：首屏失败/有旧数据失败/401 refresh/timeout/429/5xx、50/51/120 条、快速换 symbol 与乱序响应；断言 error 不显示 empty、旧数据有时间戳、跨页无重复且可访问最老记录。

### PCD-P1-03 — auth/routing 仍有双事实和 redirect 丢失；guest 秒合约测试与实际公开行为相反

- **Delta / 优先级**：延续 APC-P1-04，并补充路由/guest 证据；P1（guest 目录本身为 P2 影响）。
- **证据类型**：双事实、非响应式 computed、redirect 丢失和 guest 阻断是**确认的静态缺陷**；Turnstile orphan 数量待浏览器复现。
- **代码证据**：
  - Pinia 用 `token` ref 判登录（`pc/src/stores/user.ts:19-45`），`useAuthRequired` 却创建没有任何响应式依赖的 `computed(() => Boolean(readAuthToken()))`（`pc/src/composables/useAuthRequired.ts:8-28`）。localStorage 与持久 Pinia 又有优先级/条件写入逻辑（`pc/src/utils/authStorage.ts:20-75`）。
  - refresh 只调用 `writeAuthTokens`（`pc/src/api/request.ts:104-139`），不原子更新 Pinia token，也不通知仍在线的 private WS token 轮换；失败时 `window.location.href='/login'` 丢弃来源路由（`:14-18`）。
  - `goToLogin` 正确写 `redirect=route.fullPath`（`pc/src/composables/useAuthRequired.ts:15-21`），登录成功却固定 `router.push('/')`（`pc/src/views/auth/Login.vue:421-430`）。
  - Router 没有 route meta/global guard/catch-all（`pc/src/router/index.ts:4-167`）；UserLayout 仅在组件层渲染 AuthRequired（`pc/src/views/User/UserLayout.vue:83-87`）。这能防止私有子页面 mount，但不会形成统一导航/redirect 契约，未知 URL 也没有 404。
  - Turnstile `reset` 后把 widget ID 清空而不是保留或 remove（`pc/src/views/auth/Login.vue:205-230`），后续 initialize/unmount 无法按 ID 清理旧 widget（`:260-305,326-327,370-372`）。
  - 秒合约 mounted 在 guest 时立即 return，模板整页只显示 AuthRequired（`pc/src/views/SecondOptions.vue:310-352`）；测试名称却声称保留公开行情，同时实际断言这个整页 guard（`pc/tests/guest-auth-states.test.ts:69-80`），与 public product spec（`.trellis/spec/backend/seconds-contracts.md:158-163`）相反。
- **影响**：refresh、跨标签、直接写 storage 或登出时不同组件可拥有不同登录视图；深链接登录后回首页；Turnstile 重试可能遗留 widget；访客无法发现本应公开的 seconds 产品/行情。
- **整改**：单一响应式 SessionStore 原子持有并持久化 access/refresh/user，storage 只是 adapter；refresh 同步 store 与 private WS，跨标签处理 storage event。统一 route meta/guard 和仅接受同源内部路径的 redirect consumer；增加 404。Turnstile 采用 generation-aware lifecycle。Seconds 始终加载公开数据，仅保护余额、订单和 mutation。
- **验证**：冷启动、refresh 成功/失败、跨标签、logout、direct `/user/...`、query/hash 深链、恶意外部 redirect、未知路由；Turnstile slow load/reset/unmount 后始终仅一个 widget；guest seconds 断言公开 API 被调用而私有 API 为 0。

### PCD-P1-04 — WebSocket token 轮换存在旧 socket 覆盖新 socket 的 ABA；无 heartbeat/freshness，UI 还固定宣称 Stable Connection

- **Delta / 优先级**：APC-P1-05 延续，并新增确定性 ABA 复现；P1。
- **证据类型**：ABA 已**本地确定性复现**；heartbeat/freshness 缺失与固定 live 文案是**确认的静态缺陷**；代理半开频率待网络故障注入。
- **代码证据**：
  - `openSocket/openPrivateSocket` 的 handlers 直接修改共享 client，不校验 handler 所属 socket 或 generation（`pc/src/api/stomp.ts:103-147`）。token 变化先 close 旧 socket、立即打开新 socket（`:86-101`）；旧 `onclose` 若异步迟到，会把新 socket 的 `connected=false/socket=null` 并安排第三次 reconnect。
  - 本轮 mock 复现：新 socket 已 OPEN 时 `isConnected(private)=true`；触发旧 socket 的迟到 close 后变 false，1ms 后 socket 数从 2 增至 3，而第二个 OPEN socket 已失去引用。现有 Mock 的 `close()` 同步触发 close（`pc/tests/stomp.test.ts:45-54`），因此 token/reconnect tests（`:326-430`）没有覆盖真实异步 close。
  - 客户端只忽略收到的 `pong`，从不发送 ping，也无入站 watchdog（`pc/src/api/stomp.ts:327-373`）；重连固定 3000ms、无指数退避/jitter/online/visibility（`:57-64,426-442`）。
  - 最后 public subscription 被移除时只 unsubscribe/delete，不关闭 client（`:282-291`）；Contract/Seconds 页面 unmount 也只释放 subscription（`pc/src/views/Contract.vue:373-376`、`pc/src/views/SecondOptions.vue:341-345`）。Home 进入后 connect spot，但没有 unmount cleanup（`pc/src/views/Home.vue:167-182`）。
  - `isConnected()` 只是非响应式 pull method（`pc/src/api/stomp.ts:209-215`），src UI 没有消费者。Footer 无条件画绿色脉冲并写 `Stable Connection`（`pc/src/components/layout/Footer.vue:1-8`）。Home 还显示固定 `$1,245,678,901`、`+12.5%`（`pc/src/views/Home.vue:33-49`），NewsTicker 是 2024 年硬编码英文数据（`pc/src/components/home/NewsTicker.vue:43-73`），Footer 的 24h volume 也是固定常量（`pc/src/components/layout/Footer.vue:15-17`）。
- **影响**：token 轮换可泄漏一个仍 OPEN 但不再受控的 socket、创建重复连接并暂停私有刷新；半开连接会无限展示旧行情；断线时用户仍看到绿色“稳定连接”和无来源的市场数字/旧新闻，无法判断资金页面是否陈旧。
- **整改**：每次 open 分配 generation，所有 handler 先验证 `client.socket === sourceSocket && generation === current`；旧 socket 迟到事件只清理自身。加入 heartbeat、独立 inbound watchdog、指数退避+jitter、online/visibility 恢复、最后订阅引用计数关闭。向组件暴露响应式 `live/stale/offline,lastMessageAt,lastRestSyncAt`；行情/资金 UI 不得用假常量或 entry price 冒充 live，新闻复用已有 public news API。
- **验证**：fake timer + delayed old close/message、token 轮换、OPEN 后沉默、网络恢复、最后订阅释放、反复 mount/unmount；断言单业务最多一条受控连接、旧 generation 不改状态、无遗留 timer。UI 测试断网/时钟推进后显示 stale/offline，禁止出现静态 Stable Connection/生产新闻 fixture。

### PCD-P1-05 — 资金 DTO/输入/显示仍把 Decimal text 转为 IEEE-754 `Number`

- **Delta / 优先级**：延续 APC-P1-07；P1；若实际资产允许高精度大额资金输入，应升 P0。
- **证据类型**：精度损失机制是**确认的静态缺陷**；生产可达金额/scale 待资产配置补证。
- **代码证据**：通用 formatter 第一行即 `Number(value)`，金额通常只显示 2/4 位、低价最多 6 位（`pc/src/utils/format.ts:5-18`）。全局 adapter `toNumber` 把非法/缺失值静默变 0（`pc/src/api/backendAdapters.ts:2082-2093`）。资金写 DTO 广泛使用 number，例如 spot（`pc/src/api/exchange.ts:20-27`）、margin（`pc/src/api/contract.ts:22-31,46-53`）、seconds（`pc/src/api/second.ts:21-29`）、withdraw（`pc/src/api/backendAdapters.ts:135-145`）和 earn（`pc/src/api/finance.ts:76-81`）。Withdraw fee 也在浮点中相乘（`pc/src/api/wallet.ts:389-412`）。
  - 本轮确定性验证：`Number('9007199254740992.000000000000000001') === Number('9007199254740992.000000000000000002')`，两者都变成 `9007199254740992`。
  - 项目 precision spec 允许资产 `precision_scale=0..=18` 并要求按目标资产精度截断（`.trellis/spec/backend/wallet-amount-precision.md:12-21`）。
- **影响**：极小非零显示为 0，大数/18 位小数在序列化前已变成另一合法十进制；`invalid → 0` 还会把契约缺字段伪装成业务零值。
- **整改**：transport/domain 金额一律 decimal string；输入以字符串按资产 precision 校验，计算使用 Decimal 库；formatter 接收 DecimalText 并按资产精度/列规则输出。缺失、非法、0 必须三态区分。
- **验证**：0/2/8/18 scale、极小非零、>2^53、费率 tier 边界、PnL/fee；请求前后字符串严格相等，错误值不得落成 0，显示/ledger 按明确舍入规则一致。

### PCD-P1-06 — 没有标准全测/coverage/lint 门禁；当前 22 个测试直跑 5 项失败，构建不包含类型或测试

- **Delta / 优先级**：延续 APC-P1-06/09；P1。相较 8 月 30 日记录的 21 个测试文件，当前为 22 个，但门禁仍只覆盖一个文件。
- **证据类型**：**确认的静态/本地验证缺陷**。
- **代码证据与验证**：
  - `pc/package.json:6-12` 只有 `test:margin`，没有 `test`、coverage、lint、e2e 或 release script；`build` 只是 `vite build`。Tauri `beforeBuildCommand` 也只调用它（`pc/src-tauri/tauri.conf.json:6-10`）。
  - 当前 22 个 `*.test.ts` 中 17 个使用 `readFileSync`，大量测试只匹配源文本；例如 router test 通过 regex 判断字符串存在（`pc/tests/router-paths.test.ts:1-50`），不能验证路由行为、焦点、竞态或真实组件状态。
  - `tsconfig.json:28-29` 只 include `src`，测试文件不在 `vue-tsc` 类型检查范围。
  - 本轮 `node --experimental-strip-types --test tests/*.test.ts`：97 项，92 pass、5 fail。一个失败来自 `backendAdapters.test.ts:88-92`，四个来自 `stomp.test.ts:60-75,255-285,360-425`；它们期望 `127.0.0.1`，实际由 `APP_CONFIG` 回退到 `https/wss://hipoex.cllbmz.kdns.fr`（`pc/src/config/app.ts:1-6`），表明环境契约和测试已漂移。
  - `npm run test:margin` 只运行 `contract-margin-actions.test.ts`，当前 11/11 pass，无法阻断上述 5 项或 P0-01/P0-02。
  - 无写入 `vue-tsc --noEmit -p tsconfig.json` 当前通过；无写入 Vite production build 当前也通过（49 chunks/15 assets，最大 raw JS 约 418 KB、377 KB、170 KB）。这证明当前源码可编译，不证明发布门禁完整。
- **影响**：已发生的 quote DTO、seconds settlement、URL config 和 WebSocket 生命周期漂移不会由标准 npm/Tauri build 阻断；源码 regex 测试会在行为错误时继续通过。缺 coverage 也无法知道资金分支是否进入门禁。
- **整改**：提供唯一 `npm test` 运行全部测试，加入组件行为测试环境、coverage 阈值和 lint；测试 tsconfig 纳入 CI。`build:release` 顺序执行 type-check→全测→Vite build；Tauri beforeBuild/CI 只调用该 release gate。配置模块改成可注入 factory，测试明确提供 origin，不依赖真实生产 fallback。
- **验证**：标准 `npm test` 0 fail；故意破坏 quote、settlement、generation、redirect 或 i18n key 时对应行为测试失败；coverage 至少按核心资金 modules 设 branch 阈值；构建缺/非法 origin 必须 fail closed。

### PCD-P1-07 — Tauri updater 仍不可交付，capability/process/artifact/pubkey 均未闭环；CSP 为空

- **Delta / 优先级**：延续 APC-P1-09；P1。新增确认 `createUpdaterArtifacts` 未设置且 schema 默认 false。
- **证据类型**：配置缺口是**确认的静态缺陷**；具体 OS 上的错误文本、endpoint 可用性与签名安装需打包运行时验证。
- **代码证据**：
  - AppUpdater 调用 updater `check/downloadAndInstall`、dialog `ask` 和 process `relaunch`（`pc/src/components/common/AppUpdater.vue:1-58`）。
  - Rust 只注册 updater/dialog（`pc/src-tauri/src/lib.rs:3-16`）；Cargo 只有 updater/dialog、没有 `tauri-plugin-process`（`pc/src-tauri/Cargo.toml:20-27`），尽管 JS package 存在（`pc/package.json:19-23`）。本地 plugin-process README 明确要求 Cargo dependency 与 `.plugin(tauri_plugin_process::init())`（`pc/node_modules/@tauri-apps/plugin-process/README.md:25-66`）。
  - capability 只有 `core:default`（`pc/src-tauri/capabilities/default.json:1-10`）；生成 schema 明确 updater `check/download/install` 需要 `updater:default` 或细分 allow，dialog ask 需要 dialog permission（`pc/src-tauri/gen/schemas/desktop-schema.json:2148-2157,2231-2259`）。当前调用会被 ACL 拒绝。
  - updater `pubkey` 仍是 `YOUR_PUBLIC_KEY_HERE`（`pc/src-tauri/tauri.conf.json:39-45`）。bundle 未设置 `createUpdaterArtifacts`（`:28-38`），而锁定 CLI schema 的默认值是 false、说明为“不生成 updater 和签名”（`pc/node_modules/@tauri-apps/cli/config.schema.json:82-91,2092-2099`）。
  - Tauri CSP 为 null（`pc/src-tauri/tauri.conf.json:24-26`）；窗口最小 1600×900（`:13-21`），小屏/缩放下可访问性需 OS 验证。
  - Footer 的“检查更新”不调用 updater，只把文字改成 `Updates unavailable`（`pc/src/components/layout/Footer.vue:22-33`）；真实 updater 失败仅 console（`pc/src/components/common/AppUpdater.vue:47-49`）。
- **影响**：可编译的桌面包仍无法完成检查→授权→下载→安装→重启；发布流程也不会生成签名 updater artifact。用户看不到真实失败，CSP 又没有为带 native IPC 的壳建立最小防护层。
- **整改**：补 Rust process plugin/dependency；按最小权限加入 updater check/download/install、dialog ask、process relaunch capability；配置真实公钥、`createUpdaterArtifacts`、签名 secret 管理和 channel manifest；建立限制 API/WS/更新域及必要资源的 CSP。Footer 复用同一 updater store，版本来自 package/Tauri metadata。
- **验证**：三目标 OS staging channel 做签名 artifact、无更新、取消、下载、安装、relaunch、坏签名、回滚、离线与 capability deny/allow smoke；构建产物 manifest 记录版本/API/WS/updater origin。缺 key/pubkey/artifact/capability 时 release gate 失败。

---

### PCD-P2-01 — i18n 键不对称、硬编码英文和宿主 locale 使语言切换不完整

- **Delta / 优先级**：新增细化；P2。
- **证据类型**：**确认的静态缺陷**。
- **代码证据**：
  - AST 盘点：English 1054 个 leaf keys、Chinese 1057 个；English 缺 `market.high/low/turnover`。这些 key 被 Contract/Trade/LaunchpadTrade/SecondOptions 直接使用，例如 `pc/src/views/Contract.vue:27-31`、`pc/src/views/Trade.vue:27-39`；中文定义存在于 `pc/src/i18n/index.ts:1157-1174`，英文 `market` 段（`:54-80`）没有对应项。English UI 会回显 key/产生 missing-key warning。
  - Finance 的 login/amount/min/max toast 硬编码英文（`pc/src/views/Finance.vue:250-276`）；Footer 和 NewsTicker 也硬编码英文（`pc/src/components/layout/Footer.vue:1-33`、`pc/src/components/home/NewsTicker.vue:43-68`）。
  - 11 个业务页面/组件直接调用无 locale 参数的 `toLocaleString()`，包括 spot/margin/seconds/wallet 历史（如 `pc/src/components/trade/OrderHistory.vue:194`、`ContractOrders.vue:334-336`、`pc/src/views/User/Withdraw.vue:419`）；Recharge 数字使用 `Intl.NumberFormat(undefined)`（`pc/src/views/User/Recharge.vue:417`）。因此应用 locale 与宿主 OS locale 可不同。
  - setting 可切 `en/zh`（`pc/src/stores/setting.ts:7-35`），但 document `<html lang>` 永远是 `en`（`pc/index.html:2`），没有同步代码。
  - 全部文案集中在 2222 行单文件（`pc/src/i18n/index.ts`），没有 parity/literal-key CI。
- **影响**：英语交易页出现 `market.high` 等内部 key；中文模式仍出现英文 toast/新闻/日期格式，读屏器语言也错误。
- **整改**：按 locale/domain 拆文件并建立 schema/parity check；所有用户文案走 key，日期/数字统一传 setting locale 与 timezone policy；locale watcher 同步 `document.documentElement.lang`。真实新闻走 public news locale contract。
- **验证**：AST parity 0 差异、全部 literal `t()` key 存在；en/zh 两轮组件快照/E2E，断言无 key 回显、无硬编码业务英文、日期/数字和 `<html lang>` 同步。

### PCD-P2-02 — 核心交易控件和模态框仍缺键盘/焦点/语义契约

- **Delta / 优先级**：新增细化；P2。
- **证据类型**：**确认的静态缺陷**；读屏器/平台窗口实际体验待浏览器与 Tauri 手测。
- **代码证据**：
  - spot order type 用三个带 `@click` 的 `<span>`，没有 button/role/tabindex/keydown（`pc/src/components/trade/OrderForm.vue:25-31`）；Contract pair trigger 也是 clickable div（`pc/src/views/Contract.vue:10`），Footer updater 是 clickable span（`pc/src/components/layout/Footer.vue:9-12`）。
  - 多个关键 modal 只是 fixed div，无 `role=dialog`、`aria-modal`、焦点 trap、Escape 或关闭后 focus return：撤单（`pc/src/components/trade/OrderHistory.vue:64-92`）、平仓（`pc/src/components/trade/ContractOrders.vue:132-173`）、转账/杠杆/模式（`pc/src/components/trade/ContractOrderForm.vue:119-192`）、seconds result（`pc/src/views/SecondOptions.vue:665`）。关闭 `×` 也没有 aria-label（如 `OrderHistory.vue:69`、`ContractOrders.vue:138`）。
  - Login 的账户、密码和 2FA labels 没有 `for`，inputs 也无 id；只有 remember checkbox 正确关联（`pc/src/views/auth/Login.vue:18-81`）。
  - 全树只有 Transaction 日期弹窗明确使用 `role=dialog` 和 Escape（`pc/src/views/User/Transaction.vue:56-58`），可作为项目内模式。
  - 原生窗口 `minWidth=1600/minHeight=900`（`pc/src-tauri/tauri.conf.json:13-21`）会抵消许多 responsive class；1366×768/高缩放设备可能无法完整访问操作区。
- **影响**：键盘用户不能切换订单类型/交易对，模态框焦点可落到背景且无法可靠退出；读屏器缺字段名与 dialog 边界，小屏原生窗口可能遮挡资金操作。
- **整改**：原生 `<button>`/关联 label；统一 accessible Dialog/Select/Tabs primitives，含初始焦点、trap、Escape、focus return 和 aria name；移除过大的最小窗口或给交易区提供可滚动响应布局。
- **验证**：axe + keyboard-only E2E 覆盖 login、下单、撤单、平仓、转账；Tab 顺序、Escape、焦点回归、读屏名称均断言。Tauri 在 1366×768、1440×900 和 200% scaling smoke。

### PCD-P2-03 — 巨型 adapter/i18n/view 与散落 async state 提高契约和竞态修复成本

- **Delta / 优先级**：延续结构热点并量化；P2。
- **证据类型**：**确认的静态维护风险**。
- **代码证据**：当前最大文件：`pc/src/i18n/index.ts` 2222 行、`pc/src/api/backendAdapters.ts` 2101 行、`pc/tests/backendAdapters.test.ts` 1407 行、`pc/src/views/Prediction.vue` 1039 行、`pc/src/api/stomp.ts` 730 行、`pc/src/views/SecondOptions.vue` 734 行、`pc/src/stores/contract.ts` 535 行、`pc/src/components/trade/ContractOrderForm.vue` 502 行。`backendAdapters.ts` 同时承载 auth、wallet、spot、margin、seconds、news、earn 等 transport 和 view mapping；i18n 两种语言共存一文件；页面同时管理 API、selector、timer、modal 和渲染。
- **影响**：一个契约改动容易跨 2k 行 adapter 漏字段；源码 regex 测试鼓励“出现某字符串即通过”；异步状态缺少边界，直接造成 PCD-P0-01、P1-02、P1-04 类问题。
- **整改**：按业务拆 `transport generated types → domain adapter → query/mutation service → view model`；保留兼容 façade 逐域迁移，优先 wallet/seconds/margin。i18n 按 locale/domain 懒加载；复杂页面把 selector、mutation state machine、socket manager 和 dialog 拆成可单测 composables/components。不要以一次性重写方式处理。
- **验证**：为每个 façade 建 golden contract tests；设新增文件/组件复杂度预算和禁止资金 DTO 使用 `any/number` 的 lint/type rule；拆分前后行为快照、bundle chunk 与核心 coverage 不回退。

### 3. 本轮已验证改善 / 未发现新增阻断

1. `vue-tsc --noEmit -p pc/tsconfig.json` 当前通过；当前 `pc/src` 没有类型错误。
2. Vite production build 通过 programmatic `build({write:false})`，没有写 `dist`；产出 49 chunks/15 assets。主 chunk 约 418,081 bytes raw，两个 chart chunks 约 376,672/170,469 bytes raw。当前没有构建失败，但也没有 bundle budget。
3. 在无写入 production build 中注入 `TAURI_SIGNING_PRIVATE_KEY` 与 `_PASSWORD` sentinel，最终 assets 均未包含 sentinel；因此本轮**没有**把 `vite.config.ts:18` 的 `TAURI_` envPrefix 单独判为已发生的 production signing-key 泄漏。后续仍应保持 renderer 只读取显式非秘密变量。
4. `pc/postcss.config.js:1-6` 当前只是标准 Tailwind + Autoprefixer，没有发现旧任务上下文中提及的异常构建逻辑。
5. `npm run test:margin` 当前 11/11 通过；精确仓位 ID、capability∩product mode、批量失败汇总和 transfer risk fail-closed 等既有保护仍在。它们不覆盖本文件的新 P0/P1。

### 4. Validation record

| 验证 | 结果 |
|---|---|
| `python3 ./.trellis/scripts/task.py current --source` | active task 确认为 `.trellis/tasks/08-30-project-code-business-optimization-reaudit` |
| `./node_modules/.bin/vue-tsc --noEmit -p tsconfig.json --pretty false`（cwd `pc`） | PASS，exit 0 |
| `node --experimental-strip-types --test tests/*.test.ts` | FAIL：97 total / 92 pass / 5 fail；1 个 API origin、4 个 WS origin 期望漂移 |
| `npm run test:margin` | PASS：11/11 |
| Vite JS API `build({build:{write:false}})` | PASS：49 chunks / 15 assets；无磁盘 production artifact |
| 同构建注入 Tauri signing sentinels | PASS：private key/password sentinel 均未进入输出 |
| private WS delayed-old-close mock | REPRODUCED：新 socket OPEN 后旧 close 将 connected 置 false，socket 2→3，OPEN socket 失去引用 |
| i18n TypeScript AST parity | en 1054、zh 1057；en 缺 `market.high/low/turnover`；无重复 key |
| literal `t/$t` key 扫描 | 1140 个 literal calls / 844 unique；确认上述 3 个 English 缺 key（`trade.` 为动态拼接的扫描假阳性） |
| Decimal collision | 两个不同 18 位 decimal text 经 Number 后相等 |
| 测试结构盘点 | 22 test files、97 test calls、17 files 使用 `readFileSync` |

## Files Found

- `.trellis/tasks/08-30-project-code-business-optimization-reaudit/research/admin-pc-cross-layer.md` — 2026-08-30 PC/Admin 跨层基线与后端契约证据。
- `.trellis/tasks/08-30-project-code-business-optimization-reaudit/prd.md` — 本轮优先级、delta 和验证口径。
- `pc/src/views/User/Recharge.vue`、`Withdraw.vue` — 充值/提现 selector、金额、请求与呈现状态。
- `pc/src/api/wallet.ts`、`backendAdapters.ts` — wallet/network/withdraw DTO，以及全 PC 业务 adapter 中心。
- `pc/src/api/request.ts`、`pc/src/utils/authStorage.ts`、`pc/src/stores/user.ts`、`pc/src/composables/useAuthRequired.ts` — timeout、refresh、storage 与响应式 session 边界。
- `pc/src/router/index.ts`、`pc/src/views/auth/Login.vue`、`pc/src/views/User/UserLayout.vue` — 路由、redirect、guest guard 与 Turnstile 生命周期。
- `pc/src/api/idempotency.ts`、`exchange.ts`、`contract.ts`、`second.ts`、`finance.ts`、`loan.ts`、`activity.ts` — mutation 幂等策略与业务请求。
- `pc/src/api/stomp.ts`、`pc/tests/stomp.test.ts` — public/private socket state machine 与现有 mock 覆盖。
- `pc/src/stores/contract.ts`、`second.ts`、`pc/src/components/trade/ContractOrders.vue`、`OrderHistory.vue` — 风险、历史、错误/陈旧态与分页。
- `pc/src/views/Home.vue`、`pc/src/components/home/NewsTicker.vue`、`pc/src/components/layout/Footer.vue` — 生产首页/页脚的固定“实时”内容。
- `pc/src/i18n/index.ts`、`pc/src/stores/setting.ts` — locale 文案、设置与格式化边界。
- `pc/src/components/trade/OrderForm.vue`、`ContractOrderForm.vue`、`ContractOrders.vue`、`OrderHistory.vue` — 核心交易交互和 dialog accessibility。
- `pc/package.json`、`package-lock.json`、`tsconfig.json`、`vite.config.ts`、`postcss.config.js` — scripts、锁定版本、类型/构建配置。
- `pc/src-tauri/tauri.conf.json`、`Cargo.toml`、`src/lib.rs`、`capabilities/default.json` — 原生 bundle、插件、ACL、CSP 与 updater 配置。
- `pc/src-tauri/gen/schemas/desktop-schema.json`、`pc/node_modules/@tauri-apps/cli/config.schema.json` — 当前锁定 Tauri 版本生成的 capability 与 updater artifact 默认值。

## Code Patterns

- **可变选择 + 无 generation 的 async commit**：先改全局 selected state，再 await，任何响应都能写回；造成充值/提现乱序错配。
- **timeout 后换 idempotency key**：mutation 每次调用即时生成 key，失败未保留；仅 spot/transfer 使用 retry-stable helper。
- **Decimal string → Number → String**：adapter/store/form 过早转浮点，非法值还被归一为 0。
- **权威 DTO → 旧 view model 丢字段/填默认值**：cross risk、settlement evidence、quote 在 adapter 边界消失。
- **catch→console，旧数据无时间语义**：empty/error/stale 未建模；latest-N 又被伪装成 page。
- **共享 socket client 无 source identity**：旧 handler 可以覆盖新 generation；连接状态没有响应式 freshness consumer。
- **源码正则代替行为测试**：17/22 test files 读取源码，无法覆盖竞态、焦点、导航和真实组件状态。
- **JS plugin 存在但 native delivery 未闭环**：package import、Rust registration、ACL、artifact signing 和 UI 状态各自分离。

## External References / Versions

- 本轮未联网；外部行为只采用仓库锁定依赖自带 README、config schema 和生成 capability schema，不据此声称生产 endpoint 可用。
- 锁定版本（`pc/package-lock.json`）：Vue 3.5.27、Vue Router 4.6.4、Pinia 2.3.1、Axios 1.13.3、TanStack Vue Query 5.92.9、Vite 5.4.21、TypeScript 5.9.3、vue-tsc 2.2.12、Tauri CLI 2.9.6、updater 2.9.0、process 2.3.1、dialog 2.6.0。
- Tauri process 的 Rust registration 要求来自锁定包 README（`pc/node_modules/@tauri-apps/plugin-process/README.md:25-66`）。
- Tauri updater/dialog capability 与 updater artifact 默认值来自当前生成/锁定 schema（`pc/src-tauri/gen/schemas/desktop-schema.json:2148-2259`、`pc/node_modules/@tauri-apps/cli/config.schema.json:2092-2099`）。

## Related Specs

- `.trellis/spec/backend/auth-sessions.md:15-48,63-68` — token 格式、refresh 后更新与失败清理契约。
- `.trellis/spec/backend/realtime-websockets.md:7-16,55-68,157-188` — PC WS 路由、ping/pong 与 REST reconciliation。
- `.trellis/spec/backend/wallet-amount-precision.md:12-21,63-68` — 0..18 scale 与提现 fee/amount 权威精度。
- `.trellis/spec/backend/margin-trading-actions.md:19-27,65-79,88-105` — risk、Decimal string、幂等和批量 action 语义。
- `.trellis/spec/backend/seconds-contracts.md:69-109,141-163` — settlement evidence、precision 和 guest public catalog。
- `.trellis/spec/backend/public-news-contract.md:6-28,42-52` — public news locale/content 契约，替代 Home 硬编码 news fixture。
- `.trellis/spec/backend/error-handling.md:53-58` — 稳定错误 code/message 的 frontend 边界。

## Caveats / Not Found

1. 按研究代理约束未执行 git operation，因此“新增识别”只表示未在 2026-08-30 报告中记录，不能归因到某个 8 月 30 日后的 commit。当前审计对象是本轮可见检出内容。
2. 本轮严格保持 PC 前端范围；withdraw/margin/seconds 的后端必填字段与权威语义引用 2026-08-30 已落盘跨层研究和 Trellis specs，没有重新扩展审计后端实现。
3. 未访问真实 API/WS/updater endpoint，未启动浏览器或 Tauri，未执行会写 `dist/target` 的普通 build/cargo check。充值真实丢款概率、mutation 二次提交、半开代理行为、CSP exploitability、updater OS 错误和窗口小屏表现仍需上述运行时验证。
4. Vite production build 使用 `write:false`；可证明当前 bundle pipeline 成功和输出形状，但不是安装包签名、资源打包或三平台 smoke。
5. 全量测试的 5 个失败均由测试预期 localhost、代码 fallback 真实域名触发；这证明测试/config 不一致，不单独证明生产域名自身不可用。
6. 未发现当前 production bundle 含注入的 Tauri signing sentinel，也未发现异常 PostCSS；不要把这两项当成当前已证实漏洞。
7. 未修改任何生产文件、现有 research、spec、进度文件或其他任务目录。
