# Research: admin、pc、mobile 前端与后端契约跨层审计

- Query: 只读审计 admin（`web/`）、`pc/`、`mobile/` 的路由、API client/DTO、状态管理、实时 WS 生命周期、错误处理、鉴权、i18n、组件复用、超大视图/CSS、重复逻辑、测试质量、构建边界，以及三端同一业务口径是否漂移。
- Scope: internal（仓库静态审计；前后端混合范围）
- Date: 2026-08-24

## Findings

### 结论摘要

- 共确认 **8 项 P0**、**12 项 P1**。P0 集中在 PC 杠杆平仓/风险展示、提现网络与手续费，以及批量操作的部分失败处理。
- 三端中，mobile 的杠杆产品能力、全仓风险、批量部分失败和公开行情 WS 生命周期最接近后端当前契约；PC 仍保留大量旧模型适配，已出现会改变资金操作意图的漂移；admin 的服务端授权边界存在，但前端动作授权粒度、传输韧性和资源打包边界需要收敛。
- 主要分类：**生产缺陷** 12 项、**结构债务** 10 项、**缺少测试/质量门禁** 9 项；同一发现可同时属于多个分类。

### 三端结构与边界概览

| 维度 | admin (`web/`) | pc | mobile |
|---|---|---|---|
| 路由 | `web/src/app/router.tsx:11-56` 分离登录/管理壳；`web/src/admin/routes.tsx:56-155` 管理资源路由，多数页面懒加载 | `pc/src/router/index.ts:4-167`；页面懒加载，但无统一鉴权 guard，依赖页面内 guest state | `mobile/src/router/index.ts:42-97`；Hash 路由、页面懒加载，鉴权依赖 guest state；`mobile/src/core/navigation.ts:43-72` 限制内部重定向 |
| API/DTO | `web/src/api/client.ts::apiRequest` + React Query；通用资源使用 `ApiRecord` | Axios + `pc/src/api/backendAdapters.ts` 旧模型适配层 | Axios + 按领域拆分 API；杠杆能力/风险 DTO 较完整 |
| 状态 | React Query 与页面本地 `useState/useEffect` 并存 | Pinia store 为主，存在吞错和 token 双事实源 | Pinia 管会话/主题/导航，复杂页面另有显式 generation/lifecycle 状态 |
| 鉴权 | `AuthShell` + 服务端 admin action 权限 | 页面级 `useAuthRequired`，token 与 store 分裂 | guest state + 安全 redirect；登录页消费 redirect |
| 实时 | admin 行情 socket 为一次性连接 | STOMP 管理公开/私有主题，但缺沉默检测和完整租约回收 | 公开行情有 watchdog/backoff/租约；私有流另有 REST 对账兜底 |
| 构建 | Docker 只构建此端 | 独立 Vite/Tauri，未进入主镜像/CI 门禁 | 独立 Vite/PWA/Tauri，未进入主镜像/CI 门禁 |

### P0

#### P0-01 PC 平仓按钮文案与实际仓位方向相反

- **分类**：生产缺陷；缺少测试。
- **证据**：`pc/src/components/trade/ContractOrderForm.vue:91-105` 把按钮 0 标成“平多”、按钮 1 标成“平空”；同文件 `submitOrder`（`:322-363`）明确 0=平空、1=平多。`pc/src/api/contract.ts::resolveOpenPositionId`（`:232-240`）也以 0 查 short、1 查 long。
- **影响**：用户点击“平多”会选择 short 仓位，点击“平空”会选择 long 仓位；在同时持有双向仓位时会直接执行相反的资金操作。
- **建议**：取消整数方向在 UI 层的隐式语义，按钮直接绑定 `{ positionId, action: close }`；短期至少修正文案与调用方向，并让成功提示来自实际关闭仓位。
- **验收**：构造同时存在 long/short 的后端 fixture；点击“平多”只请求 long 的 `position_id`，点击“平空”只请求 short 的 `position_id`；请求、成功提示、刷新后仓位三者一致。
- **工作量**：S，0.5–1 天。

#### P0-02 PC 展示“限价/部分平仓”，后端实际始终全量市价平仓

- **分类**：生产缺陷；结构债务；缺少测试。
- **证据**：`pc/src/components/trade/ContractOrderForm.vue:59-78,355-361` 收集数量/比例并提交 `volume`；`pc/src/components/trade/ContractOrders.vue:148-202,450-472` 提供市价/限价、价格、数量、25/50/75/100%。`pc/src/api/contract.ts::closePosition`（`:83-92`）丢弃 `type/price/volume`，只向 `/margin/positions/{id}/close` 发送 `{}`。后端 `src/modules/margin/routes.rs::close_position`（`:377-396`）仅接收路径 ID，`src/modules/margin/application/lifecycle.rs::close_margin_position`（`:43-117`）结算整个仓位。
- **影响**：用户选择 25% 或限价仍会立刻按服务端标记价全平，属于交易意图与实际执行不一致。
- **建议**：当前契约下立即移除部分数量、百分比和限价控件，明确标注“市价全平”；若产品确需部分/限价平仓，应先新增后端订单契约、冻结/撮合/幂等与剩余仓位模型，再开放 UI。
- **验收**：短期版本 DOM 和请求中均不存在部分/限价承诺，确认文案明确全平；后续功能版需证明部分成交后剩余数量、保证金、利息、风险快照及幂等重放均正确。
- **工作量**：短期 S，0.5 天；完整能力 L，5–10 天。

#### P0-03 PC 无视产品能力和用户设置，始终以逐仓下单

- **分类**：生产缺陷；跨端口径漂移；缺少测试。
- **证据**：`pc/src/components/trade/ContractOrderForm.vue:7,342-350` 固定展示并提交 `isolated`；`syncLeverageFromServer`（`:232-244`）读取设置后只应用 leverage，丢弃 `marginMode`。PC API 已支持读取/切换模式：`pc/src/api/contract.ts:146-178`。后端模式设置契约位于 `src/modules/margin/routes.rs:250-276`，产品能力位于 `src/modules/margin/presentation.rs:146-190`。mobile 会交集产品/服务端能力并提交选择值：`mobile/src/api/trading.ts:212-260,374-415`。
- **影响**：PC 用户即使保存了全仓模式，下一笔仍以逐仓开仓；资金域、强平和风险口径与 mobile 不一致。
- **建议**：以产品 `margin_modes` ∩ capabilities 为可选集合，先加载用户 setting，404 才回退产品默认值；切换成功后再更新本地状态，下单发送当前状态。
- **验收**：cross/isolated 两套 fixture 下，请求体、刷新后的选中项和后端 setting 一致；不支持的模式不渲染且不可构造请求。
- **工作量**：M，2–4 天。

#### P0-04 PC 把仓位伪装成钱包并在客户端编造风险参数

- **分类**：生产缺陷；结构债务；跨端口径漂移；缺少测试。
- **证据**：后端 `/margin/wallets` 明确返回 `wallets/positions/cross_accounts`：`src/modules/margin/presentation.rs:193-215`。PC DTO `pc/src/api/backendAdapters.ts:745-756` 不声明 `cross_accounts`；`mapMarginWalletsToContractWallets`（`:1576-1608`）拼接钱包和仓位，`mapMarginPositionToContractWallet`（`:1832-1859`）把 `margin_amount` 当余额、entry price 当 current price，并把手续费/维持保证金率设为 0。`pc/src/stores/contract.ts:254-291` 又用 `||` 把 0 改成 0.0001/0.005；`pc/src/components/trade/ContractOrders.vue:286-369` 据此自行算 PnL/风险。mobile 保留 `crossAccounts` 并使用服务端风险：`mobile/src/api/trading.ts:274-337`。
- **影响**：PC 的余额、现价、盈亏、保证金率和全仓聚合可能错误；多交易对共享同一保证金资产时尤其会低估或重复计算风险。
- **建议**：建立严格的 `MarginWalletSnapshot { wallets, positions, crossAccounts }`，禁止仓位转钱包；展示服务端 position risk/cross account risk，缺值显示未知而不是默认费率。
- **验收**：多交易对、同保证金币种、long/short 混合 fixture 下，PC 与后端/mobile 的 equity、PnL、maintenance margin、margin ratio 逐字段一致；风险请求失败时显示“不可用/陈旧”，不得显示 0 或编造值。
- **工作量**：L，5–10 天。

#### P0-05 PC 提现网络为硬编码，后端也未校验资产—网络启用关系

- **分类**：生产缺陷；跨端口径漂移；缺少测试。
- **证据**：`pc/src/api/wallet.ts::fetchCoinNetworks`（`:163-187`）仅在 deposit 模式查询后端，withdraw 直接回退硬编码；`supportedDepositNetworks`（`:297-305`）为 USDT 等返回固定 ETH/Base/BTC/Tron/Solana。`pc/src/views/User/Withdraw.vue:331-374,434-448` 展示并提交该值。后端创建提现 `src/modules/wallet/application.rs::create_withdrawal_request`（`:674-762`）只加载资产费率规则，未查询资产—网络配置；`validate_withdrawal_request`（`:979-1023`）只做网络枚举归一。`src/modules/wallet/infrastructure/withdrawals.rs:172-213` 随后写申请并冻结本金+服务端费用。
- **影响**：PC 可提交资产并未启用的网络，后端仍可能创建申请并冻结资金，之后只能由审核/网关失败路径人工释放；mobile 与 PC 网络列表也可能不同。
- **建议**：后端在安全凭据消耗和冻结前校验“活跃资产 + 活跃提现网络 + 资产白名单”；新增明确的 withdrawal-network 查询契约。PC/mobile 都只使用该事实源，失败时 fail closed，禁止内置生产回退。
- **验收**：禁用/未关联网络返回 4xx，且无提现单、无账本、available/frozen 不变、验证凭据未消费；两端网络列表与后端配置精确一致。
- **工作量**：M，2–4 天。

#### P0-06 mobile 丢弃阶梯手续费，确认页金额与服务端实际冻结额不一致

- **分类**：生产缺陷；跨端口径漂移；缺少测试。
- **证据**：后端资产 DTO 含 `withdraw_fee_tiers`：`src/modules/wallet/presentation.rs:393-404`，查询也加载该列：`src/modules/wallet/infrastructure/deposits.rs:594-617`。mobile DTO `mobile/src/api/wallet.ts:116-124` 未声明阶梯，映射只保留固定费（`:179-191`）；`mobile/src/views/WithdrawView.vue:37-40,207-209,274-279` 用固定费计算并展示，提交见 `:95-107`。后端以服务端阶梯重新计费：`src/modules/wallet/infrastructure/withdrawals.rs:116-143`，客户端 fee 不参与实际计费：`src/modules/wallet/application.rs:979-1023`。PC 已有阶梯计算：`pc/src/api/wallet.ts:394-412`。
- **影响**：用户确认的手续费/到账额可能低于后端最终冻结和收取值，属于资金披露错误；金额越过阶梯边界时最明显。
- **建议**：优先新增服务端提现 quote，返回标准化金额、权威 fee、total_reserved 和有效期；提交绑定 quote。过渡期至少完整映射阶梯并共享与后端一致的区间规则。
- **验收**：固定费、闭区间边界、开放尾档、无命中回退的 fixture 中，确认页 fee/到账额与创建响应及 `total_reserved` 完全一致；配置变化后过期 quote 不可提交。
- **工作量**：M，2–3 天。

#### P0-07 PC 仓位/委托加载失败被呈现为空数据或无提示陈旧数据

- **分类**：生产缺陷；结构债务；缺少测试。
- **证据**：`pc/src/stores/contract.ts::loadCurrentOrders`（`:161-207`）、`loadHistoryOrders`（`:210-251`）、`loadWallets`（`:254-295`）仅 `console.error`，没有 error/freshness 状态且不再抛出。`pc/src/components/trade/ContractOrders.vue:16-24,416-427` 在 loading 结束后直接渲染 `no_data`；已有旧值时则继续展示且无陈旧标识。
- **影响**：首次故障会让用户误以为没有杠杆敞口；刷新故障会把旧仓位伪装成实时数据，可能影响是否补保证金或平仓的决定。
- **建议**：每类数据使用判别状态 `idle/loading/success/error/stale`、时间戳和 retry；失败不得转换为空数组，刷新失败保留旧数据但显著标陈旧。
- **验收**：首次 5xx 显示错误与重试而非“无数据”；刷新 5xx 保留旧仓位并显示时间；恢复后替换快照并清除 stale；测试覆盖登出/切产品后的旧请求失效。
- **工作量**：M，2–3 天。

#### P0-08 PC 将批量平仓的部分失败统一提示为成功

- **分类**：生产缺陷；跨端口径漂移；缺少测试。
- **证据**：后端明确允许部分成功并要求读取 `positions` 与 `failures`：`src/modules/margin/routes.rs::close_all_positions`（`:399-418`）。PC `pc/src/api/contract.ts::closeAllPositions`（`:95-99`）透传响应，`pc/src/stores/contract.ts::submitCloseAllPositions`（`:337-347`）返回数据，但 `pc/src/components/trade/ContractOrders.vue:482-496` 丢弃结果并总是 success toast。mobile 的映射保留 failures：`mobile/src/api/trading.ts::mapMarginBatchAction`（`:453-464`），对应 UI 测试要求部分失败提示：`mobile/tests/contract-pencil-selected-parity.test.ts:178-180`。
- **影响**：仍有杠杆仓位暴露时用户会收到“平仓成功”，可能离开页面而继续承担行情/强平风险。
- **建议**：PC 建立强类型 `MarginBatchActionResult`；按全成功、部分成功、全失败分别提示，列出失败仓位并立即刷新权威快照。
- **验收**：混合成功/失败 fixture 显示成功数、失败数和剩余 ID；失败项仍在列表中；`failures.length > 0` 时绝不出现纯成功提示。
- **工作量**：S，0.5–1 天。

### P1

#### P1-01 PC 鉴权状态存在双事实源，登录后 redirect 被丢弃

- **分类**：生产缺陷；结构债务；缺少测试。
- **证据**：`pc/src/composables/useAuthRequired.ts:8-21` 的 computed 直接读取无响应性的 localStorage；`pc/src/utils/authStorage.ts:20-84` 同时兼容独立 token 与持久化 Pinia；`pc/src/stores/user.ts:19-45,73-96` 又维护 token ref。刷新逻辑 `pc/src/api/request.ts:104-139` 只写存储。登录成功 `pc/src/views/auth/Login.vue:421-430` 固定跳 `/`，没有消费 `useAuthRequired` 写入的 redirect。
- **影响**：组件内登录状态可能不刷新，refresh 后 store/token 不一致，受保护动作登录后无法返回原流程。
- **建议**：只保留一个 Pinia session owner；API/WS 通过其 token provider 取值，并实现受白名单约束的 redirect 消费及跨标签同步。
- **验收**：覆盖启动恢复、refresh、logout、跨标签、登录状态响应性和恶意外链 redirect；登录后返回原内部路由。
- **工作量**：M，2–4 天。

#### P1-02 PC 实时连接缺沉默检测、完整租约回收和可信状态展示

- **分类**：生产缺陷；结构债务；缺少测试。
- **证据**：`pc/src/api/stomp.ts:103-125,327-359` 没有 ping/独立入站 watchdog；最后公开订阅取消时 `:282-291` 不关闭仅由公开主题持有的 socket；重连 `:426-442` 固定 3 秒、无指数退避/jitter。`pc/src/api/socket.ts:1-86` 还有一套重复实现。`pc/src/components/layout/Footer.vue:1-33` 无条件显示绿色 “Stable Connection” 和静态成交额。mobile 的公共流基线见 `mobile/src/api/webSocketLiveness.ts:1-40` 及 `mobile/tests/market-ticker-stream.test.ts:257-261`。
- **影响**：半开连接会长期显示陈旧行情；页面卸载后残留连接；服务恢复时固定重连形成惊群；UI 对用户谎报健康状态。
- **建议**：合并为单一 transport manager，公开/私有 topic 使用引用计数租约；实现 ping、与 ping 独立的入站沉默 watchdog、指数退避+jitter、generation 隔离，并把真实连接/陈旧状态暴露给 UI。
- **验收**：OPEN 但静默会关闭并重连/重订阅；最后租约释放后无 socket/timer；卸载后不重连；旧连接回调无效；Footer 能显示 connecting/offline/stale/live。
- **工作量**：L，4–7 天。

#### P1-03 admin 动作权限只做资源级 ANY 判断，未细分到按钮

- **分类**：生产缺陷（操作体验）；结构债务；缺少测试。
- **证据**：`web/src/admin/access.ts:83-161` 将 endpoint 映射到一组 mutation 权限；`web/src/admin/resources/resourceConfigs.tsx:1453-1466` 只要命中任一权限就显示整个资源的 actions/rowActions。钱包行同时渲染 review/broadcast/confirm/fail：`web/src/admin/resources/actions/wallet.tsx:1213-1303`。服务端仍按具体动作授权：`src/modules/admin/service/access_control.rs:86-112,240-262`。
- **影响**：review-only 管理员会看到 write 等无权按钮并在提交后收到 403；虽未形成服务端越权，但会误导高权限操作和审计流程。
- **建议**：每个 action descriptor 声明精确权限；按钮、批量动作与快捷入口均按同一 catalog 过滤，后端继续作为最终权威。
- **验收**：read/review/write/operate/settle 角色 fixture 逐按钮断言；不可见动作不能发请求；直接构造请求仍由后端拒绝。
- **工作量**：M，3–5 天。

#### P1-04 admin 请求与行情 socket 缺统一韧性策略

- **分类**：生产缺陷；结构债务；缺少测试。
- **证据**：`web/src/app/providers.tsx:13-24` 全局 mutation retry=1；登录/2FA mutation 位于 `web/src/auth/LoginPage.tsx:155-182`，未单独关闭重试。`web/src/api/client.ts:30-43,84-90` 无 timeout/AbortSignal，成功响应统一假定 JSON；`web/src/admin/resources/AdminResourcePage.tsx:225-258` 只用 active boolean 忽略结果，没有取消请求。`web/src/api/marketTickerSocket.ts:51-69` 无 reconnect、heartbeat、沉默检测或 freshness。
- **影响**：登录/2FA POST 可被自动重复；请求可能无限等待；合法空响应可触发 JSON 解析错误；后台价格预览冻结时无提示。
- **建议**：mutation 默认 retry=0，仅对显式幂等动作开启；client 支持 deadline、AbortSignal、204/空体/content-type；行情复用有状态的 WS lifecycle。
- **验收**：登录/2FA 遇 5xx 只发一次；超时返回类型化错误；200 空体和 204 正常；路由切换能 abort；行情静默后显示 stale 并按退避重连。
- **工作量**：M，3–5 天。

#### P1-05 admin 通用资源形成 mega-chunk，关键 DTO 仍是弱类型记录袋

- **分类**：结构债务；构建边界；缺少测试。
- **证据**：`web/src/admin/routes.tsx:9-29` 的通用资源路由都导入同一个 `resourceConfigs`；`web/src/admin/resources/resourceConfigs.tsx:3-34` 静态导入所有领域 action，文件共 1469 行。`web/src/api/types.ts:25-47` 使用大量可选字段的 `PageResponse`/`ApiRecord`；对应测试 `web/src/admin/resources/resourceConfigs.test.tsx` 达 4555 行。
- **影响**：访问单一资源会携带无关领域动作代码；后端字段漂移可在编译期漏过；配置与巨型测试成为冲突热点。
- **建议**：按领域拆分 route-level config/action chunk，保留表格原语复用；P0/P1 mutation 使用端点专属 DTO，逐步退出 `ApiRecord`。
- **验收**：Vite manifest 显示 users 路由不依赖 wallet/earn action chunk；关键 mutation 不再接受 `ApiRecord`；设置 bundle budget 和领域级测试。
- **工作量**：L，5–10 天。

#### P1-06 PC 缺省配置会静默连接固定生产域名

- **分类**：生产缺陷；构建边界；缺少测试。
- **证据**：`pc/src/config/app.ts:1-6` 在环境变量缺失时回退 `https://hipoex.cllbmz.kdns.fr`，`pc/src/api/request.ts:20-30` 的 API 都由此派生。mobile 已有生产 fail-closed 校验：`mobile/src/config/backend.ts:34-76`。
- **影响**：preview/Tauri 或其他环境构建可能误连真实租户；REST/WS 来源也难以在制品层审计。
- **建议**：生产/native 构建强制提供并验证 backend origin，禁止 http/loopback/路径污染；开发代理仅在 dev 模式启用，REST/WS 从同一配置派生。
- **验收**：生产构建在缺失、非法、http、loopback 配置时失败；dev proxy 可用；制品可追溯其 REST/WS origin。
- **工作量**：S–M，1–2 天。

#### P1-07 主交付流水线未构建 pc/mobile，也没有三端统一测试门禁

- **分类**：构建边界；缺少测试。
- **证据**：`Dockerfile:6-17,63` 只构建并复制 `web` 产物；`.github/workflows/docker-image.yml::jobs.build.steps` 仅执行镜像构建。`pc/package.json:6-11` 没有 test/lint script 且 build 不含 type-check，尽管 `pc/tests/` 已有测试；`mobile/tsconfig.json:21-22` 排除 tests，mobile 测试依赖 `node --test --experimental-strip-types`。仓库未发现 Playwright/Cypress E2E 配置。
- **影响**：PC/mobile 的类型错误、死测试和构建失败可进入主分支；当前多个资金语义缺陷没有交付门禁拦截。
- **建议**：建立矩阵 CI：web lint/typecheck/test/build；pc typecheck/test/build/Tauri smoke；mobile typecheck/test/PWA build/Tauri smoke。资金操作新增最小浏览器 E2E；source-regex 测试只作为结构约束，不替代行为测试。
- **验收**：每端故意加入一个失败测试、类型错误和构建错误均能阻断 PR；三端脚本可在干净环境执行；P0 场景至少有请求级/交互级行为测试。
- **工作量**：M，2–4 天。

#### P1-08 PC i18n 单体化且仍有硬编码用户文案

- **分类**：生产缺陷；结构债务；缺少测试。
- **证据**：`pc/src/i18n/index.ts:3-2209` 在单文件维护 en/zh，默认/回退语言见 `:2205-2209`；`pc/src/views/User/Finance.vue:250-276` 与 `pc/src/components/layout/Footer.vue:1-33` 存在硬编码英文。mobile 已拆分 locale：`mobile/src/i18n/index.ts:1-62`。admin 的中文单语属于 `.trellis/spec/admin/ui-system.md` 既定范围，不记为缺陷。
- **影响**：中文 PC 在异常/状态区仍显示英文；大字典难以审查 key 和插值参数漂移。
- **建议**：按 locale/领域拆包，加入递归 key 与插值占位符对等检查；用户可见文案禁止裸字面量（品牌、单位除外）。
- **验收**：en/zh key 集和 placeholder 完全相同；切换语言后 Finance 错误、Footer 状态实时更新；静态扫描无未豁免硬编码。
- **工作量**：M，2–4 天。

#### P1-09 核心交易契约未纳入 OpenAPI，重复手写适配已产生漂移

- **分类**：结构债务；跨端口径漂移；缺少测试。
- **证据**：`src/openapi.rs:53-185` 的路径注册未覆盖完整 spot/margin/seconds/earn/loan/prediction 用户契约；`Cargo.toml:47-48` 已有 OpenAPI 工具但未覆盖关键域。`pc/src/api/backendAdapters.ts` 共 2038 行并大量使用 `any`；mobile 在 `mobile/src/api/newCoin.ts:225`、`prediction.ts:150`、`loan.ts:108`、`earn.ts:85`、`trading.ts:422`、`seconds.ts:89` 重复生成 idempotency key；三端也分别重复 error-message/时间归一逻辑。
- **影响**：字段、枚举、Decimal、部分失败和能力声明靠人工同步；本报告中的 margin/fee 漂移正是该结构的产物。
- **建议**：先补齐资金域 OpenAPI，生成共享的 transport DTO 包；各端只保留展示/domain mapper。集中定义 Decimal text、错误码、幂等键和时间戳策略，并在 CI 做 schema diff。
- **验收**：spot/margin/wallet/seconds 的请求响应由生成类型约束；破坏性 schema diff 阻断 CI；三端共享 golden contract fixtures，枚举/部分失败/Decimal 不再各自猜测。
- **工作量**：XL，10–20 天，可按领域分批。

#### P1-10 mobile 超大视图/CSS 已超过可维护边界，复用集中在表象而非生命周期

- **分类**：结构债务；组件复用；缺少测试。
- **证据**：`mobile/src/views/TradeView.vue:1-5935` 同时承载 REST、WS、保证金对账、双布局、弹窗和样式；`mobile/src/views/SecondsView.vue:1-2818`、`AssetsView.vue:1-2046` 同类。`mobile/src/styles/prototype-base.css:1-8034` 与 `prototype-parity.css:1-3707` 为巨型全局样式。对照项：`pc/src/i18n/index.ts:1-2212`、`web/src/styles.css:1-2721` 也已形成热点。
- **影响**：生命周期、业务动作与视觉变更互相牵连；合并冲突和回归面过大，难以对 WS 清理、旧请求失效等关键行为做隔离测试。
- **建议**：按“数据生命周期 composable + 业务 workspace + dialog/sheet + token/layer CSS”拆分；保留页面为编排层，避免仅把模板机械切成无语义小组件。
- **验收**：每个提取的 lifecycle 有 start/stop、切账户、切产品、旧回调失效测试；TradeView 编排层目标低于 1500 行；320/390/desktop 视觉回归通过；CSS 有 layer/token 与 specificity 预算。
- **工作量**：XL，10–15 天。

#### P1-11 mobile 仍逐笔模拟现货批量撤单，丢失后端部分失败结果

- **分类**：生产缺陷；跨端口径漂移；缺少测试。
- **证据**：`mobile/src/api/trading.ts::cancelAllSpotOrders`（`:205-209`）注释称后端无批量端点，实际用 `Promise.allSettled` 逐笔调用并只抛第一个错误。后端已提供 `DELETE /spot/orders`：`src/modules/spot/routes.rs:47-60,170-184`，结果含 `orders/failures`：`src/modules/spot/application/cancellation.rs:55-77`；契约见 `.trellis/spec/backend/spot-orders.md:74-107`。PC 已使用该端点：`pc/src/api/exchange.ts:58-69`。
- **影响**：产生 N 个请求，失败结果不可完整呈现，刷新/重试口径与 PC、后端不一致。
- **建议**：mobile 直接调用批量端点并映射完整 successes/failures，按部分成功刷新和提示；删除过时注释。
- **验收**：一次网络请求完成撤单；混合 fixture 精确显示成功/失败 ID 与数量；重放不重复解冻；测试不再只断言源码包含旧逐笔调用。
- **工作量**：S，约 1 天。

#### P1-12 PC 杠杆历史把订单类型和时间固定为错误值

- **分类**：生产缺陷（展示/审计）；跨端口径漂移；缺少测试。
- **证据**：`pc/src/api/backendAdapters.ts::BackendMarginPosition`（`:717-739`）不声明 `order_type/limit_price/created_at`；`mapMarginPositionsToContractOrders`（`:1548-1564`）固定 `type: 0`、`createTime: 0`。`pc/src/components/trade/ContractOrders.vue:99-121` 却把 type 0 展示为限价并渲染时间。mobile 显式映射订单类型/限价：`mobile/src/api/trading.ts:432-450`。
- **影响**：市场单可在 PC 历史中显示为限价单，时间显示 epoch/空值；从 mobile 创建的限价仓位在 PC 无法准确复核。
- **建议**：稳定后端用户仓位的 `order_type/limit_price/created_at|opened_at` 契约，PC DTO 与 mapper 原样保留；未知值显示未知，不做枚举默认猜测。
- **验收**：market/limit、pending/opened/closed fixtures 的类型、冻结价格和时间逐字段正确；PC/mobile 对同一仓位显示一致。
- **工作量**：M，2–4 天（含后端契约补全）。

### 覆盖面归纳

- **路由/鉴权**：三端均已做页面懒加载；admin 有统一受保护壳，PC/mobile 主要依赖页面 guest state。mobile 的安全内部 redirect（`mobile/src/core/navigation.ts:43-72`）和登录消费 redirect（`mobile/src/views/LoginView.vue:100-113`）可作为 PC 修复基线。
- **API/DTO/错误**：mobile 的领域 DTO 优于 PC 旧适配层，但提现费仍漏字段；admin 通用资源复用度高，但弱类型和请求取消不足。三端 refresh 都有单飞倾向，但 PC session 状态未统一。
- **状态管理**：最大风险不是框架选择，而是“失败=空/旧值无标识”和请求 generation。mobile 多处已有 stale-generation 模式；PC 杠杆 store 尚未采用。
- **实时 WS**：mobile 公开行情具备 watchdog/backoff/清理基线；PC 缺入站沉默检测与租约闭环；admin ticker 为一次性连接。mobile 私有流 `mobile/src/api/privateUserStream.ts:239-276` 也没有独立入站沉默 watchdog，但交易页有周期 REST 对账，暂列后续强化而非本轮 P1。
- **i18n/复用/CSS**：PC 的语言包与 mobile 的交易页分别成为“单文件事实中心”；admin 的通用资源页则是过度中心化。应复用 transport/lifecycle/contract，而不是继续扩大一个配置或视图文件。
- **测试**：mobile 有较多源码正则契约测试（例如 `mobile/tests/contract-pencil-selected-parity.test.ts:118-180`），能防结构回退但不能证明点击行为和真实请求；PC 已有测试目录却没有标准 test script；三端均缺资金主路径 E2E。
- **构建**：主 Docker/CI 事实边界仅覆盖 admin web 与 Rust；PC/mobile 是独立可交付产品，但未进入同等质量门禁。

### 建议执行顺序

1. **先止血（1–3 天）**：P0-01、P0-02 的误导控件、P0-08 部分失败提示、P0-06 手续费披露；同时为这些场景补请求级行为测试。
2. **资金事实源（1–2 周）**：P0-03/04/05/07，统一 margin snapshot、withdrawal network/quote、错误与 freshness 状态。
3. **交付门禁（同周并行）**：P1-06/07，先让 pc/mobile 的 typecheck/test/build 真正阻断合并。
4. **结构收敛（后续迭代）**：P1-02/05/09/10，按领域拆分，不做一次性大迁移。

## Files Found

- `web/src/app/router.tsx` — admin 顶层登录/受保护路由。
- `web/src/admin/routes.tsx` — admin 资源路由与动态导入边界。
- `web/src/api/client.ts` — admin HTTP client、刷新与错误解析。
- `web/src/admin/access.tsx` — admin 前端权限推导。
- `web/src/admin/resources/resourceConfigs.tsx` — 通用资源配置和动作聚合点。
- `web/src/api/marketTickerSocket.ts` — admin 行情 WebSocket。
- `pc/src/router/index.ts` — PC 路由表。
- `pc/src/api/request.ts`、`pc/src/utils/authStorage.ts`、`pc/src/stores/user.ts` — PC 请求刷新与会话存储。
- `pc/src/api/backendAdapters.ts` — PC 对 Rust 后端的旧模型适配集中层。
- `pc/src/api/contract.ts`、`pc/src/stores/contract.ts` — PC 杠杆 API 与状态。
- `pc/src/components/trade/ContractOrderForm.vue`、`ContractOrders.vue` — PC 下单/平仓 UI。
- `pc/src/api/stomp.ts`、`pc/src/api/socket.ts` — PC 两套实时连接实现。
- `pc/src/api/wallet.ts`、`pc/src/views/User/Withdraw.vue` — PC 提现网络、费用与提交。
- `mobile/src/router/index.ts`、`mobile/src/core/navigation.ts` — mobile 路由和安全导航。
- `mobile/src/api/trading.ts` — mobile 现货/杠杆契约适配。
- `mobile/src/api/wallet.ts`、`mobile/src/views/WithdrawView.vue` — mobile 提现 DTO 与确认页。
- `mobile/src/api/webSocketLiveness.ts` — mobile 公共 WS 活性策略。
- `mobile/src/views/TradeView.vue`、`mobile/src/styles/prototype-base.css` — mobile 最大业务视图和样式热点。
- `src/modules/margin/routes.rs`、`presentation.rs`、`application/lifecycle.rs` — 杠杆路由、DTO、结算事实源。
- `src/modules/wallet/application.rs`、`infrastructure/withdrawals.rs` — 提现校验、计费和冻结事实源。
- `src/modules/spot/routes.rs`、`application/cancellation.rs` — 现货批量撤单契约。
- `src/openapi.rs` — 当前 OpenAPI 路径注册边界。
- `Dockerfile`、`.github/workflows/docker-image.yml` — 主交付构建边界。

## External References / Versions

- 未联网检索外部资料；结论只依赖当前仓库代码、清单和 Trellis specs。
- `web/package.json`：React 19.2.6、React Router 7.16、TanStack Query 5.100.14、Semi Design 2.99.2、Vite 8、Vitest 4。
- `pc/package.json`：Vue 3、Pinia、Vue Query、Vite 5、Tauri；当前 scripts 不含测试入口。
- `mobile/package.json`：Vue 3、Pinia、Vite/PWA/Tauri；测试采用 Node test runner + experimental type stripping。
- `Cargo.toml:47-48`、`src/openapi.rs`：仓库已有 OpenAPI 生成基础，但核心交易路径覆盖不完整。

## Related Specs

- `.trellis/spec/guides/cross-layer-thinking-guide.md` — 跨层数据流与契约核对方法。
- `.trellis/spec/guides/code-reuse-thinking-guide.md` — 复用边界与重复逻辑判断。
- `.trellis/spec/admin/ui-system.md`、`.trellis/spec/admin/auth-turnstile.md` — admin UI/鉴权约束。
- `.trellis/spec/mobile/backend-integration.md`、`.trellis/spec/mobile/navigation-and-localization.md`、`.trellis/spec/mobile/pwa-and-shell.md` — mobile 请求、实时、导航、i18n、壳层契约。
- `.trellis/spec/backend/auth-sessions.md`、`.trellis/spec/backend/realtime-websockets.md` — 会话与 WS 生命周期。
- `.trellis/spec/backend/margin-trading-actions.md`、`.trellis/spec/backend/spot-orders.md`、`.trellis/spec/backend/wallet-amount-precision.md` — 杠杆、现货批量动作、钱包精度/费用事实源。

## Caveats / Not Found

- 本轮为静态只读审计；按用户要求未运行会写入 dist/cache 的 build/test，也未连接真实数据库、Redis、链网关或浏览器，因此没有宣称运行时复现或 bundle 实测值。
- 行号以 2026-08-24 当前工作树为准；后续编辑可能偏移，已同时给出关键函数/组件符号。
- 未发现 Playwright/Cypress 等三端 E2E 配置；未发现将 PC/mobile 纳入主 Docker workflow 的步骤。
- 未核对线上资产/网络/费率实际配置；P0-05 判断基于代码路径证明“创建提现前不存在资产—网络关联校验”，实际暴露范围取决于线上配置和审核流程。
- 未使用外部文档；依赖版本与契约均以仓库 manifest、源码和 Trellis specs 为准。
