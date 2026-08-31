# Research: 管理后台前端 CURRENT HEAD 增量复审（2026-08-31）

- Query: 审计当前管理后台前端（web/src、前端测试、package/Vite/ESLint 配置），识别相对 2026-08-30 复审仍存在或新识别的问题；重点覆盖鉴权与权限、API DTO/错误契约、React Query 生命周期、mutation/幂等、金额精度、表格/表单/弹窗可访问性、列宽拖拽、i18n/copy、实时连接、超大配置/组件、路由拆包、测试质量和生产构建/安全。
- Scope: mixed（当前仓库静态证据、只读验证、官方外部文档）
- Date: 2026-08-31
- Baseline: .trellis/tasks/08-30-project-code-business-optimization-reaudit/research/admin-pc-cross-layer.md

## Findings

### 1. 结论与判定口径

本轮记录 1 项 P0、7 项 P1、4 项 P2。这里的“新增识别”仅表示未出现在 2026-08-30 基线报告中；由于研究代理按约束未执行任何 git 操作，不能据此断言问题由某个具体提交新引入。

- **已确认（静态）**：当前源码、配置、依赖实现或可重复的本地只读探针已证明该机制存在。
- **运行时假设**：影响成立还依赖真实网络故障、后端行为、角色数据、浏览器环境、代理/CDN 配置或部署安全头；本文明确与静态事实分开。

| ID | 优先级 | 相对基线 | 摘要 |
| --- | --- | --- | --- |
| FAD-P0-01 | P0 | 新增识别，且属于此前 P0 修复的边界缺口 | 充值幂等键没有按十进制语义规范化，且只保存在行组件内存中 |
| FAD-P1-01 | P1 | APC-P1-03 延续并扩展 | 资源权限仍以“任一写能力”放开全部操作，独立页面大多只做读权限路由守卫 |
| FAD-P1-02 | P1 | APC-P1-04 延续并扩展 | 管理员访问查询跨身份复用，登出不清缓存，跨标签页与原目标回跳仍不完整 |
| FAD-P1-03 | P1 | 新增识别 | 全局 mutation retry 会自动重放登录/2FA，并复用一次性 Turnstile token |
| FAD-P1-04 | P1 | APC-P1-06 延续并扩展 | 宽松 DTO 把契约漂移降级为空数据，行级资源选项产生重复且不可取消的请求 |
| FAD-P1-05 | P1 | APC-P1-07 延续，已补直接复现 | 金额格式化和边界比较继续转换为 JavaScript Number |
| FAD-P1-06 | P1 | APC-P1-05 延续 | 行级行情 WebSocket 无重连、心跳和 freshness 状态 |
| FAD-P1-07 | P1 | 新增安全姿态项 | access/refresh token 均持久化到 localStorage，前端登出仅本地清理 |
| FAD-P2-01 | P2 | 新增识别 | 秒合约 Tabs 的 aria-controls 指向不存在的 tabpanel，重复字段命名不唯一 |
| FAD-P2-02 | P2 | 基线 caveat 已实测确认 | 路由懒加载已有改善，但首屏入口和共享 resourceConfigs chunk 仍过大且无预算门禁 |
| FAD-P2-03 | P2 | 新增测试治理项 | 全量测试通过，但生产 retry、故障注入、契约/a11y/覆盖率和包体门禁存在系统缺口 |
| FAD-P2-04 | P2 | 基线环境 caveat 延续 | 浏览器标题仍为英文品牌文案，环境变量名与运行时代码不一致 |

---

### FAD-P0-01 — 充值幂等键未按十进制语义规范化，且无法跨组件重建

- **优先级**：P0
- **相对基线**：新增识别；直接影响 2026-08-30 已宣称完成的充值幂等修复边界。
- **判定**：
  - **已确认（静态）**：相同十进制值的不同文本表示会生成不同 intent；待确认键只保存在行组件的内存 Map 中。
  - **运行时假设**：是否实际发生二次入账，需通过“服务端已提交但响应丢失”及组件重挂载/页面刷新故障注入确认。

**证据**

1. web/src/shared/idempotency.ts:8-20 的 canonicalValue 对字符串只执行 trim；没有把 “25.50” 与 “25.5” 归一为同一十进制值。
2. web/src/shared/idempotency.ts:22-43 的待处理键仅存放于进程内 Map，没有持久化、服务端查询或恢复协议。
3. web/src/admin/resources/actions/users.tsx:85-89 在每个 UserRechargeAction 行组件中用 useRef 创建独立 manager；行卸载、账号切换或页面刷新都会丢失待确认键。
4. web/src/admin/resources/actions/users.tsx:107-127 使用 trim 后的原始 amount 构造 business intent，发起请求后只在成功分支 complete。
5. web/src/api/client.ts:30-42 未配置请求 timeout/AbortSignal，无法为“不确定结果”建立统一的超时、恢复和核对路径。
6. web/src/shared/idempotency.test.ts:5-16 只覆盖同一字面量的重试与成功后轮换；web/src/admin/resources/resourceConfigs.test.tsx:2131-2182 只覆盖成功的一次请求。规范要求的十进制等值、响应丢失、重挂载/刷新、409 冲突均未覆盖。
7. 直接加载当前 helper 的只读探针得到：

       canonicalRequestIntent({ amount: "25.50", ... })
       !==
       canonicalRequestIntent({ amount: "25.5", ... })

   实际输出为 equal=false。

**影响**

- 已确认：前端把十进制语义相同的两次输入识别为两个逻辑命令；组件生命周期也会无条件生成新键。
- 运行时条件成立时：第一次充值已在服务端提交但响应丢失，操作员重输等值金额、表格刷新导致行重挂载或浏览器刷新后再提交，可能携带新幂等键，从而失去服务端按键去重保护。这是直接资金风险。

**修复建议**

1. 用字符串十进制算法按业务精度规范化金额，禁止经过 Number；冻结 user_id、asset_id、规范化 amount、trim 后 reason。
2. 将未决命令存入可恢复的命令日志（至少 sessionStorage/IndexedDB，并按管理员会话、用户、资产隔离），并提供按幂等键查询/核对结果的服务端流程。
3. 明确定义 timeout、网络中断、取消、重挂载和 409 时的状态机；只有确认失败且可安全新建逻辑命令时才轮换键。
4. 补充测试：25.50/25.5 同键、18 位小数、服务端提交后响应丢失、组件重挂载、页面恢复、参数变化换键、409 显式反馈。

**验证**

- 当前 idempotency helper 的等值金额探针已稳定复现 key 不相等。
- 现有相关单测通过只能证明“同字面量、同组件、失败后重试”路径，不能覆盖上述故障模型。

---

### FAD-P1-01 — 写权限仍按能力并集放大，独立页面多为只读权限守卫

- **优先级**：P1
- **相对基线**：APC-P1-03 延续，并确认问题不只存在于通用资源页。
- **判定**：
  - **已确认（静态）**：前端显示逻辑没有按具体动作权限逐项判断。
  - **运行时假设**：真实低权限角色是否可见这些按钮、点击后返回何种 403，取决于服务端角色数据与最终鉴权；没有据此主张服务端鉴权绕过。

**证据**

1. web/src/admin/access.tsx:158-162 把一个资源端点映射为 write、review、settle、operate 四类权限。
2. web/src/admin/resources/resourceConfigs.tsx:1453-1467 只用 some 判断是否具有任意动作权限；条件一旦成立，资源配置中的批量动作和行动作整体可见，而不是逐动作匹配 capability。
3. web/src/admin/routes.tsx:32-53 的 guardedLazyRoute 只接受并校验读取权限；多组独立管理页由 web/src/admin/routes.tsx:68-77、85-107、125-155 以该方式挂载。
4. 支持工作台是少数正向例外：web/src/admin/support/AdminSupportPage.tsx:1-10 显式计算写权限。
5. 安全策略路由位于 web/src/admin/routes.tsx:143-146；web/src/admin/security/SecurityPolicyPage.tsx:173-193 可发送 PATCH，web/src/admin/security/SecurityPolicyPage.tsx:406-420 的保存入口没有对应写权限条件。
6. KYC 复核在 web/src/admin/kyc/KycManagementPage.tsx:333-359 发送 PATCH，配置保存位于 :619-640，复核动作入口位于 :753-774；页面内未发现按 review/write capability 的显式门控。

**影响**

- 菜单/页面和操作按钮与角色职责不一致，形成“可见但必然失败”的工作流。
- 审核、结算、运营、配置写入无法在 UI 层维持最小权限和职责分离；误触、告警噪声及支持成本上升。
- 服务端仍应是最终安全边界，本项不把前端隐藏按钮当作安全授权。

**修复建议**

1. 为每个 action 声明 requiredPermission，而不是为 endpoint 声明能力并集。
2. 通用资源页对 batch action、row action、编辑/删除分别判断权限。
3. 独立页面同时设置 route read gate 和页面内 write/review/settle/operate gate；无权限时禁用或不渲染，并提供一致原因。
4. 建立角色矩阵测试，至少覆盖 read-only、review-only、operate-only、write-only 和多权限组合。

**验证**

- 静态检索 hasAdminPermission 的使用点后，除支持工作台外，所列独立页面未形成完整的动作级权限闭环。

---

### FAD-P1-02 — 管理员访问查询跨身份复用，登出、跨标签页与原目标回跳生命周期不完整

- **优先级**：P1
- **相对基线**：原目标丢失问题延续；新增确认 Query 缓存跨身份窗口、瞬时错误被当作 403，以及跨标签页不同步。
- **判定**：
  - **已确认（静态）**：固定 query key、30 秒 staleTime、登出不清 QueryClient、无 storage 事件监听、登录固定跳 dashboard。
  - **运行时假设**：旧角色 UI 实际暴露时长和发生频率取决于账号切换速度与请求时序；新 token 下服务端仍应拒绝越权请求。

**证据**

1. web/src/admin/access.tsx:168-190 使用固定 queryKey [admin-access]、staleTime 30 秒且 retry=false；key 未包含管理员身份或会话世代。
2. web/src/admin/layout/AdminLayout.tsx:188-194 登出只清 authStore 并导航，没有 removeQueries/resetQueries/clear。
3. TanStack Query 核心只读探针先写入旧管理员 fresh 数据，再用相同 key 和 staleTime 获取，结果 fetches=0，返回旧管理员对象，证明身份切换窗口内不会重新取数。
4. web/src/auth/authStore.ts:54-91 只有本标签页内订阅，没有 window.storage 或 BroadcastChannel；另一标签页登录/登出不会即时驱动当前树更新。
5. web/src/admin/auth/RequireAdmin.tsx:7-23 即使保存来源也仅保存 pathname；web/src/admin/auth/LoginPage.tsx:129-142 没有消费该 state，成功后固定导航到 dashboard，查询参数与 hash 同样丢失。
6. web/src/admin/access.tsx:169-187 对访问查询禁用重试，并把非 401 的失败也落入无权限页面，使网络/5xx 与真实 403 在用户界面中不可区分。
7. web/src/admin/auth/RequireAdmin.test.tsx:9-16 每个测试创建新 QueryClient，未覆盖账号切换缓存；web/src/admin/auth/LoginPage.test.tsx:41-55 总是从 /login 开始，未覆盖深链接回跳。

**影响**

- 快速退出旧管理员再登录新管理员时，导航、角色名和动作可见性可短暂使用旧缓存。
- 跨标签页会话状态不一致；深链接登录后丢失工作上下文。
- API 暂时故障被展示成“无权限”，误导值班人员和故障诊断。

**修复建议**

1. query key 包含稳定的管理员 subject 或会话 generation；登出和账号切换时移除全部身份相关缓存。
2. 在 storage 事件或 BroadcastChannel 上同步会话变更，并在同步后取消旧请求、重置 QueryClient。
3. 保存经过校验的站内 pathname+search+hash，登录成功后消费并清除；拒绝外部 open redirect。
4. 区分 401、403、网络、5xx；瞬时错误进入可重试错误态，而不是权限拒绝页。
5. 增加“管理员 A fresh 缓存 → 登出 → 管理员 B 登录”的集成测试。

**验证**

- TanStack Query 探针已确认固定 fresh key 会直接返回旧数据且不调用 fetcher。

---

### FAD-P1-03 — 全局 mutation retry 自动重放登录与 2FA

- **优先级**：P1
- **相对基线**：新增识别。
- **判定**：
  - **已确认（静态）**：生产 QueryClient 默认会把失败 mutation 再执行一次；登录与 2FA 未覆盖 retry。
  - **运行时假设**：重复锁定计数、重复挑战或 token replay 的具体结果依赖认证后端与 Turnstile 是否启用。

**证据**

1. web/src/app/providers.tsx:15-24 为全部 mutations 配置 retry=1。
2. 当前 useMutation 使用集中在 web/src/admin/auth/LoginPage.tsx:155-182；登录和 2FA mutation 都未显式设置 retry=false。
3. web/src/api/adminAuth.ts:33-41、64-68 对应 POST 请求，不具备前端可见的幂等键。
4. web/src/admin/auth/LoginPage.test.tsx:41-44 在测试 QueryClient 中把 mutations.retry 改为 false，因此测试环境主动规避了生产策略。
5. TanStack Query 核心探针使用 retry=1、retryDelay=0 且 mutationFn 持续失败，实际调用次数为 2。
6. Cloudflare Turnstile 官方服务端验证文档明确 token 为单次使用；已使用 token 再验证会返回 timeout-or-duplicate。

**影响**

- 无效凭据或瞬时错误可能触发两次登录调用，认证失败计数、审计日志和限流消耗可能翻倍。
- 启用 Turnstile 时，第一次请求消耗 token 后，自动重试复用同一 token，第二次可能以 token 重放错误覆盖原始错误。
- 若第一次请求实际成功但响应丢失，前端可能创建重复 challenge/session，具体取决于服务端实现。

**修复建议**

1. 将全局 mutation 默认 retry 设为 false；仅对具有幂等键或明确可安全重放的 mutation 局部开启。
2. 登录和 2FA 明确 retry=false；错误后重新获取 Turnstile token，再由用户显式重试。
3. 测试使用与生产一致的 AppProviders 配置，并断言单次点击只发一个认证请求。

**验证**

- 只读 Query core 探针确认生产等价配置会调用 mutationFn 两次。
- 官方参考：https://tanstack.com/query/latest/docs/framework/react/reference/useMutation
- Turnstile 参考：https://developers.cloudflare.com/turnstile/get-started/server-side-validation/

---

### FAD-P1-04 — 宽松 DTO 隐藏契约漂移，行级资源选项形成 N 次不可取消请求

- **优先级**：P1
- **相对基线**：APC-P1-06 延续并扩展到请求生命周期和重复请求。
- **判定**：
  - **已确认（静态）**：错误 response key/type 被转换为空数组；选项 hooks 直接 useEffect 请求且不传取消信号；充值 action 按行实例化。
  - **运行时假设**：实际重复请求数量取决于当前页行数、权限和 React 挂载时机；默认页长允许放大至约 50 个实例。

**证据**

1. web/src/api/types.ts:25-47 以大量可选字段描述 PageResponse，并广泛使用 ApiRecord，编译期无法约束不同 endpoint 的真实 DTO。
2. web/src/api/adminResources.ts:35-48 在 responseKey 缺失或目标值不是数组时返回 rows=[]，没有抛出契约错误。
3. web/src/admin/resources/AdminResourcePage.tsx:225-258 用手写 useEffect 和 active 布尔量加载资源选项；卸载后只忽略回调，没有取消底层 HTTP。
4. web/src/api/client.ts:30-42 没有统一 timeout 或 AbortSignal 透传策略。
5. web/src/admin/resources/actions/shared.tsx:209-245、248-285 的资产/交易对选项 hooks 同样使用手写 effect，并把错误压成空选项。
6. web/src/admin/resources/actions/users.tsx:85-89 每个 UserRechargeAction 无条件调用 useAssetOptions；该 action 在 web/src/admin/resources/actions/users.tsx:248-263 作为用户行操作挂载。
7. web/src/admin/resources/AdminResourcePage.tsx:65-68、181-184 默认 pageSize=50；在有 50 行且操作可见时可能出现同资源目录的行级请求风暴。
8. scoped tests 中没有 adminResources.ts 的 DTO 契约测试。

**影响**

- 后端字段名或 envelope 漂移会表现为“暂无数据”，而不是可观察的契约故障。
- 资产目录失败会表现为下拉选项为空，值班人员无法判断是“无资产”还是网络/权限/DTO 错误。
- 重复请求增加网关负载和限流风险；组件卸载不取消请求，旧请求仍消耗连接与服务端资源。

**修复建议**

1. 为每个 endpoint 定义窄 DTO，并在边界执行 schema 解析；response key/type 不符时抛出带 endpoint/request-id 的 ContractError。
2. 资产、交易对等目录改为共享 React Query，key 包含身份和过滤条件，queryFn 接收 AbortSignal，并统一错误态。
3. 仅在动作可见且打开表单时加载必要目录；避免每行 eager mount 都发请求。
4. 增加正常 envelope、字段缺失、类型错误、取消、去重和错误 UI 测试。

**验证**

- 静态调用链已确认“每行 hook → 直接请求”的结构；真实请求数需要浏览器/Mock Service Worker 运行时计数。
- Query key 应包含决定返回数据的变量，参考：https://tanstack.com/query/latest/docs/framework/react/guides/query-keys

---

### FAD-P1-05 — 金额展示和表单边界比较仍丢失十进制语义

- **优先级**：P1
- **相对基线**：APC-P1-07 延续；本轮补充了 18 位精度和大整数的直接复现。
- **判定**：
  - **已确认（静态+探针）**：格式化会产生 NaN 或错误数值；边界比较存在 Number 碰撞。
  - **运行时假设**：具体受影响记录取决于生产资产精度和实际金额分布。

**证据**

1. web/src/shared/numberFormat.ts:1-3、50-60 通过 numeral/Number 处理后最多显示 6 位小数。
2. 该 formatter 被 web/src/shared/AmountText.tsx:8-14、web/src/admin/resources/AdminResourcePage.tsx:101、116、web/src/admin/resources/DetailDrawer.tsx:382、423 和 web/src/admin/resources/resourceConfigs.tsx:456-519 使用。
3. 当前依赖探针结果：
   - 0.00000001 → NaN
   - 0.000000000000000001 → NaN
   - 123456789012345678.123456789012345678 → 123,456,789,012,345,664.00
   - 9007199254740993.00 → 9,007,199,254,740,994.00
4. web/src/shared/format.test.tsx:27-53 只覆盖不超过 6 位小数的常规样本。
5. web/src/admin/resources/actions/loan.tsx:145-179 和 web/src/admin/resources/actions/wallet.tsx:158-197 使用 Number 做区间/费率层级边界比较。
6. 探针确认 1.000000000000000000 与 1.000000000000000001 都转换为 Number 1；9007199254740992 与相邻整数也可碰撞。
7. .trellis/spec/backend/wallet-amount-precision.md:12-21 规定精度可达 18 位并采用 DECIMAL(38,18) 语义。

**影响**

- 合法的小额资产可显示为字面量 NaN；大额余额、价格或统计值可能显示成另一个值。
- 合法的费率区间/借贷范围可能因 Number 碰撞被误判为相等或逆序，阻止提交。
- 当前多数请求仍发送原始字符串，因此本文不主张所有写入值都会被前端改写；确认的问题是展示和前置校验。

**修复建议**

1. 使用十进制字符串或任意精度 decimal 实现格式化和比较，任何路径都不经过 Number。
2. 格式化接受资产 precision，保留 0/2/8/18 位等业务精度，并定义尾零策略。
3. 为 10^-18、超过 2^53、38 位边界、负值、科学计数法输入和非法值补充测试。

**验证**

- numeral 与 Number 探针已直接复现 NaN、舍入和比较碰撞。

---

### FAD-P1-06 — 行情实时连接按行创建，缺少重连与 freshness 生命周期

- **优先级**：P1
- **相对基线**：APC-P1-05 延续；现有测试还把“一行一个 socket”固化为预期。
- **判定**：
  - **已确认（静态）**：每次 subscribe 创建 WebSocket，只有 message 和 cleanup；observedAt 未进入 UI。
  - **运行时假设**：半开连接、代理超时和连接上限下的故障表现需真实 WebSocket/网关测试。

**证据**

1. web/src/admin/resources/marketTickerSocket.ts:51-69 每次 subscribe 都 new WebSocket，只注册 message；没有 open/error/close 状态、重连、退避、心跳或 freshness watchdog。
2. web/src/admin/resources/marketTickerSocket.ts:30-45 已解析 observed_at。
3. web/src/admin/resources/resourceConfigs.tsx:506-519 每个行情行订阅一次，但回调只保存价格，丢弃 observedAt，断线后旧价格可无限期保持为看似实时值。
4. 默认资源页可显示 50 行，因此连接数随可见行数线性增长。
5. web/src/admin/resources/resourceConfigs.test.tsx:2636-2725 明确断言两行创建两个 socket，仅覆盖消息更新和卸载 close，未覆盖 close/error/reconnect/stale。

**影响**

- 网关、浏览器或负载均衡器关闭连接后，页面没有离线/陈旧提示，也不会恢复。
- 大表格形成连接扇出；能否协议级多路复用取决于后端，但前端至少缺少统一连接注册、引用计数和状态管理。

**修复建议**

1. 建立共享连接管理器：按订阅键复用并引用计数；若协议支持，则在单连接上多路复用。
2. 增加指数退避+jitter、online/visibility 感知、心跳/超时和卸载清理。
3. 保留 observedAt，在单元格显示实时/重连中/已陈旧，并定义 freshness 阈值。
4. 测试 abnormal close、网络切换、重复订阅、退避上限、卸载和陈旧状态。

**验证**

- 源码和现有测试均确认当前一行一 socket 且无恢复分支。
- WebSocket 生命周期规范参考：https://websockets.spec.whatwg.org/

---

### FAD-P1-07 — 管理员 access/refresh token 均持久化到 localStorage

- **优先级**：P1
- **相对基线**：新增安全姿态项。
- **判定**：
  - **已确认（静态）**：两个 token 可被同源 JavaScript 读取，登出只做本地删除；前端源码/config 未提供 CSP 证据。
  - **运行时假设**：是否存在可利用 XSS、服务端是否支持撤销、部署层是否注入 CSP/安全头均未在本前端范围内验证。因此本项不是“已确认 XSS”或“已确认缺少生产 CSP”的结论。

**证据**

1. web/src/auth/authStore.ts:23-47、57-85 把 access_token 和 refresh_token 序列化到 localStorage。
2. web/src/admin/layout/AdminLayout.tsx:188-194 的退出只清本地 store 并导航；scoped 前端未发现 logout/revoke API 调用。
3. web/index.html:1-13 没有 CSP meta；web/vite.config.ts:1-32 没有生产安全头配置。实际反向代理/CDN 可以在仓库外设置，因此只能记录“前端范围没有证据”。
4. scoped 搜索未发现 dangerouslySetInnerHTML；web/src/shared/QuillRichTextEditor.tsx:291 的 innerHTML 用于清空编辑器，不构成已确认的外部 HTML 注入点。

**影响**

- 一旦同源脚本执行被攻破，长期 refresh token 可被直接读取并外带，管理员账号影响面高于普通会话。
- 只清浏览器状态无法证明服务端会话/refresh token 已撤销；被复制的 token 是否继续有效取决于后端设计。

**修复建议**

1. 优先采用 HttpOnly、Secure、SameSite refresh cookie 或 BFF；access token 尽量只驻留内存并短时有效。
2. 提供服务端 logout/revoke，并在退出时撤销 refresh 会话、清身份缓存和实时连接。
3. 在真实部署验证 CSP、frame-ancestors、Referrer-Policy、Permissions-Policy 等响应头，并纳入发布 smoke test。
4. 持续避免 raw HTML 注入并对富文本输入/输出做明确 sanitize 契约。

**验证**

- 本轮只确认存储与登出调用路径；没有运行 XSS 攻击或检查生产响应头。
- OWASP 参考：https://cheatsheetseries.owasp.org/cheatsheets/HTML5_Security_Cheat_Sheet.html

---

### FAD-P2-01 — 秒合约 Tabs 的 ARIA 关系断裂，重复控件缺少记录级名称

- **优先级**：P2
- **相对基线**：新增识别。
- **判定**：
  - **已确认（静态）**：当前 Semi Tabs 用法会生成指向 panel id 的 aria-controls，但调用处没有 TabPane/tabpanel。
  - **运行时假设**：各屏幕阅读器的具体播报差异需浏览器辅助技术测试。

**证据**

1. web/src/admin/resources/actions/secondsContract.tsx:263-295、374-405 使用 tabList 渲染 Tabs，业务内容放在 Tabs 外部，没有 Tabs.TabPane 或显式 role=tabpanel。
2. 当前安装的 @douyinfe/semi-ui 2.99.2 中，web/node_modules/@douyinfe/semi-ui/lib/es/tabs/TabItem.js:53-66 为 tab 写 aria-controls；TabPane.js:87-95 才创建对应 panel，Tabs 主实现见 index.js:269-287。
3. 因调用处没有 panel，tab 的 aria-controls 指向文档中不存在的元素，不满足 .trellis/spec/admin/ui-system.md:152-162 的 tab/tabpanel 约束。
4. web/src/admin/resources/actions/secondsContract.tsx:187-219 的多条周期配置重复使用“周期秒数”等相同可访问名称，删除按钮也都叫“删除周期”，且数组 key 使用 index。
5. web/src/admin/resources/resourceConfigs.test.tsx:1752-1830、3154-3308 操作秒合约控件但没有断言 tabpanel；同文件其他模块在 :1610-1635、3000-3028 已有 tabpanel 断言，说明测试模式可复用。

**影响**

- 屏幕阅读器无法通过 aria-controls 定位当前标签对应内容。
- 多条重复周期记录中的输入与删除按钮在语音导航中不可区分；index key 还会在删除中间项时增加焦点/节点身份漂移风险。

**修复建议**

1. 使用 Tabs.TabPane，或显式生成稳定 panel id、role=tabpanel 和 aria-labelledby，并只暴露当前 panel。
2. 为每条周期建立稳定客户端 id；标签和删除按钮包含序号或业务值，如“周期 2 秒数”“删除周期 2”。
3. 增加 getByRole(tab)、getByRole(tabpanel)、aria-controls 目标存在、键盘切换和删除后焦点测试。

**验证**

- 通过当前调用代码与已安装 Semi 实现交叉确认 ARIA 目标缺失。
- WAI-ARIA Tabs 参考：https://www.w3.org/WAI/ARIA/apg/patterns/tabs/

---

### FAD-P2-02 — 路由拆分已有改善，但入口与共享资源配置 chunk 仍过大

- **优先级**：P2
- **相对基线**：基线只记录“未测量”；本轮已用不落盘生产构建确认。
- **判定**：
  - **已确认（构建）**：当前构建有大入口 JS/CSS 和单一 resourceConfigs chunk，配置中没有包体预算。
  - **运行时假设**：真实 LCP/INP、缓存命中和低端设备解析耗时需部署后的性能采样。

**证据**

1. 正向变化：web/src/app/router.tsx:9-55 已懒加载 admin/agent shell 和多个页面。
2. web/src/admin/routes.tsx:9-29 的所有通用资源路由仍动态导入同一个 resourceConfigs 模块。
3. web/src/admin/resources/resourceConfigs.tsx:1-34 顶层导入各业务域 action；文件本身约 1,469 行，因而任一资源页会拉入跨域配置/动作集合。
4. scoped 大文件还包括：
   - web/src/admin/resources/resourceConfigs.test.tsx：约 4,556 行
   - web/src/styles.css：约 2,721 行
   - web/src/admin/resources/actions/wallet.tsx：约 1,416 行
   - web/src/admin/support/OnlineSupportWorkbench.tsx：约 954 行
   - web/src/admin/resources/actions/earn.tsx：约 888 行
   - web/src/admin/kyc/KycManagementPage.tsx：约 792 行
5. 当前配置的 Vite 生产模式内存构建（write=false）成功，得到 43 个 chunk、21 个动态 chunk、9 个资产；关键大小：
   - 入口 JS：1,611,596 B raw / 438,288 B gzip / 346,557 B brotli
   - CSS：554,543 B raw / 65,209 B gzip
   - resourceConfigs chunk：219,082 B raw / 49,408 B gzip
   - Quill chunk：205,073 B raw / 60,073 B gzip
6. web/vite.config.ts:1-32 没有 manualChunks 或构建预算；web/package.json:6-12 没有会因 raw/gzip 超阈值而失败的脚本。Vite 默认 chunkSizeWarningLimit 只告警，不是发布门禁。

**影响**

- 即使登录页不直接下载全部业务页，公共入口仍承担较高解析/执行成本。
- 访问任一通用资源页都会下载跨业务 resourceConfigs；大型单测和组件也提高改动冲突与定位成本。

**修复建议**

1. 按资源域拆分 config/action 注册表，让每条资源路由只 import 其域模块。
2. 检查 Semi barrel import 和共享依赖提升；对富文本等重依赖维持真正的交互时加载。
3. 以真实产物建立 raw+gzip 预算，并在 CI 超限时失败；记录基线和每次 delta。
4. 按职责拆分超大页面、action 文件和测试夹具，优先拆出纯 schema、mutation hooks 与视图。

**验证**

- 构建使用 Vite JavaScript API 且 write=false，没有生成或修改 dist；它验证当前模块图和压缩尺寸，不替代部署性能测试。
- Vite 构建选项参考：https://vite.dev/config/build-options.html

---

### FAD-P2-03 — 测试全部通过，但关键生产策略与故障模型未被覆盖

- **优先级**：P2
- **相对基线**：新增测试治理项。
- **判定**：
  - **已确认（静态+测试配置）**：测试配置主动改变生产 mutation retry；缺少覆盖率、a11y、hooks/query lint 和包体门禁。
  - **运行时假设**：缺口是否已经对应线上回归需生产遥测或 E2E 证据。

**证据**

1. 全量 Vitest 当前通过：53 个测试文件、382 个测试。
2. web/package.json:6-12 只有基础 dev/build/lint/test/type-check 脚本，没有 coverage、E2E 或 bundle budget 门禁。
3. web/vite.config.ts:21-30 没有 coverage threshold。
4. web/eslint.config.js:1-20 及当前 devDependencies 未配置 react-hooks、jsx-a11y 或 TanStack Query ESLint 插件。
5. web/src/admin/auth/LoginPage.test.tsx:41-44 将 mutation retry 设为 false，与 web/src/app/providers.tsx:15-24 的生产 retry=1 不同。
6. web/src/admin/auth/RequireAdmin.test.tsx:9-16 每次使用全新 QueryClient，未覆盖身份切换缓存。
7. 充值测试只覆盖成功路径；WebSocket 测试把每行一个连接作为预期；秒合约测试没有 tabpanel 断言；scoped tests 中没有 adminResources DTO 契约测试。
8. web/src/admin/resources/resourceConfigs.test.tsx 约 4,556 行，多个业务域共享大 fixture，增加测试耦合和选择性漏测风险。

**影响**

- “382 tests passed”不能捕获生产 retry 重放、响应丢失、DTO 漂移、身份切换、断线恢复和辅助技术关系等本轮确认问题。
- 测试环境与生产 QueryClient 策略分叉，会给认证路径提供错误安全感。

**修复建议**

1. 提供直接复用 AppProviders 默认配置的 production-policy harness，局部测试再显式覆盖。
2. 对资金、认证、权限和实时路径增加故障注入/状态机测试。
3. 增加 DTO schema、a11y role/关系、coverage branch threshold 和构建包体预算。
4. 拆分 resourceConfigs.test.tsx，按资源域拥有 fixture 与断言，保留少量跨域集成测试。

**验证**

- lint、双 tsconfig type-check、目标测试、全量 Vitest 和内存生产构建均已通过；这些通过结果与上述策略/覆盖缺口可以同时成立。

---

### FAD-P2-04 — 用户可见英文标题与 API 环境变量命名漂移

- **优先级**：P2
- **相对基线**：环境变量 caveat 延续；copy 违规为本轮明确记录。
- **判定**：
  - **已确认（静态）**：标题文案违规；仓库 .env 与运行时代码读取不同变量。
  - **运行时假设**：生产 API 是否连接错误取决于构建环境是否注入 VITE_API_BASE_URL，或部署是否刻意使用同源反向代理。

**证据**

1. web/index.html:7-8 的浏览器标题为 “HIPPO Operations”，与 .trellis/spec/admin/ui-system.md:14-17 的中文用户文案和禁用 “HIPPO OPERATIONS/OPERATIONS” 约束冲突。
2. web/.env:2 定义 VITE_BACKEND_API_DOMAIN；web/src/api/client.ts:16 和 web/src/admin/resources/marketTickerSocket.ts:1 读取 VITE_API_BASE_URL。
3. 当前内存生产构建字符串扫描未包含 .env 中配置的域名，也未发现意外 loopback；API path 仍存在，说明 bundle 当前走代码中的同源 fallback。
4. Vite 的 VITE_* 变量在构建时静态替换，变量名不一致不会在运行时自动兼容。

**影响**

- 浏览器标签、收藏夹和任务切换器中的管理后台文案与中文运营界面不一致。
- 若部署期没有额外注入正确变量或配置同源代理，REST 与 WebSocket origin 可能落到非预期地址；该连接影响尚未在真实部署确认。

**修复建议**

1. 将标题改为项目约定的中文管理后台名称，并为品牌文案增加静态断言。
2. 选择唯一变量名并记录 REST/WS origin 契约；若明确采用同源，删除无效变量并在部署文档中写明。
3. 构建时校验 origin：需要显式地址的环境缺失即失败；允许同源的环境则显式声明模式。

**验证**

- 当前构建完成且字符串扫描确认 VITE_BACKEND_API_DOMAIN 没有进入 bundle；真实代理行为未测。
- Vite 环境变量参考：https://vite.dev/guide/env-and-mode.html

---

### 2. 已验证改善与本轮未发现

以下项目有当前源码/测试支撑，本轮没有形成新的缺陷项：

1. **可调整列宽**：web/src/shared/ResizableTable.tsx:20-28 定义宽度边界，:74-88 处理操作列最小宽度，:230-271 提供可聚焦 separator 和键盘操作，:372-412 清理 pointer 监听，:479-499 保持受控 scroll。web/src/shared/ResizableTable.test.tsx:30、65、108、179、232、264、290、309、339 覆盖主要行为；静态审计未发现当前残留缺陷。
2. **表格空/错/载入态**：web/src/shared/DataTable.tsx:101-125 已提供可识别状态结构。
3. **确认弹窗基础可访问性**：web/src/shared/ConfirmAction.tsx:15-72 已处理 reason trim、submitting 和 aria 关联；不代表所有业务弹窗均完成真实屏幕阅读器测试。
4. **设置 mutation 策略**：web/src/admin/settings/query.ts:11-25 与 web/src/admin/settings/useAdminSettingsEditor.ts:73-110 已局部 retry=false，优于全局默认；认证 mutation 尚未跟进。
5. **支持工作台实时替代方案**：web/src/admin/support/OnlineSupportWorkbench.tsx:247-378 使用有界 REST polling 和旧响应保护，:593-619 在未决写请求中复用幂等键，符合 .trellis/spec/admin/ui-system.md:420-452 的当前约束。
6. **路由级拆包**：web/src/app/router.tsx:9-55 已将主要 shell/page 懒加载；FAD-P2-02 记录的是剩余粗粒度共享 chunk，而非否定该改善。
7. **旧状态文案**：scoped 源码搜索未再发现基线中的 unknown_broadcast；当前使用 manual_review。未连接真实后端，故只确认前端源码现状，不确认历史 payload 的兼容性。

### 3. 验证记录

本轮在停止扩展审计前已经完成以下只读或不落盘验证；收到“不要再运行长测试”后未再启动测试：

| 验证 | 结果 |
| --- | --- |
| npm --prefix web run lint -- --no-cache | 通过 |
| web tsconfig.json：tsc --noEmit --incremental false | 通过 |
| web tsconfig.node.json：tsc --noEmit，tsBuildInfoFile=/dev/null | 通过 |
| 5 个目标测试文件 | 28/28 通过 |
| resourceConfigs 定向测试 | 4 通过，61 skipped |
| 全量 Vitest | 53 files、382 tests 通过，123.23s |
| Vite production 内存构建 | 通过；write=false，未写 dist |
| 十进制格式/比较探针 | 复现 NaN、超过 2^53 舍入和 18 位边界碰撞 |
| 幂等 intent 探针 | 复现 25.50 与 25.5 生成不同 intent |
| TanStack mutation 探针 | retry=1 时 mutationFn 调用 2 次 |
| TanStack access cache 探针 | fresh 固定 key 返回旧管理员，fetches=0 |
| scoped 文件写入检查 | 验证阶段未写 web 文件 |

本地 npm ls --depth=0 显示的关键安装版本：

- React / React DOM 19.2.6
- React Router 7.16.0
- TanStack Query 5.100.14
- Semi UI / Icons 2.99.2
- numeral 2.0.6
- Quill 2.0.3
- Vite 8.0.14
- Vitest 4.1.7
- TypeScript 6.0.3
- ESLint 10.4.1

## Files Found

- web/src/shared/idempotency.ts — 通用幂等 intent 与内存 key manager。
- web/src/shared/idempotency.test.ts — 幂等 helper 的当前窄路径测试。
- web/src/admin/resources/actions/users.tsx — 用户行充值 action 与幂等调用点。
- web/src/api/client.ts — Axios base URL、认证 header、refresh/replay 基础设施。
- web/src/admin/access.tsx — 管理员权限映射和 access query。
- web/src/admin/routes.tsx — 管理后台 route-level lazy 与 read guard。
- web/src/admin/layout/AdminLayout.tsx — 导航、角色显示和本地登出入口。
- web/src/auth/authStore.ts — access/refresh token 的 localStorage 持久化。
- web/src/admin/auth/RequireAdmin.tsx — 未登录路由来源记录。
- web/src/admin/auth/LoginPage.tsx — 登录/2FA mutation 与成功跳转。
- web/src/api/adminAuth.ts — 登录和 2FA POST DTO。
- web/src/app/providers.tsx — 全局 QueryClient retry/stale 策略。
- web/src/api/types.ts — 宽松 PageResponse/ApiRecord 类型。
- web/src/api/adminResources.ts — 通用资源 envelope 解析。
- web/src/admin/resources/AdminResourcePage.tsx — 通用列表、分页和选项加载。
- web/src/admin/resources/actions/shared.tsx — 资产/交易对目录 hooks。
- web/src/shared/numberFormat.ts — numeral 金额格式化。
- web/src/shared/AmountText.tsx — 通用金额显示组件。
- web/src/admin/resources/DetailDrawer.tsx — 详情抽屉金额展示。
- web/src/admin/resources/actions/loan.tsx — 借贷范围表单和 Number 比较。
- web/src/admin/resources/actions/wallet.tsx — 钱包费率/区间表单和 Number 比较。
- web/src/admin/resources/marketTickerSocket.ts — 行情 WebSocket 订阅。
- web/src/admin/resources/resourceConfigs.tsx — 所有通用资源配置和行操作注册。
- web/src/admin/resources/actions/secondsContract.tsx — 秒合约 tabs 和动态周期表单。
- web/src/shared/ResizableTable.tsx — 可调整列宽表格实现。
- web/src/shared/DataTable.tsx — 通用表格状态。
- web/src/shared/ConfirmAction.tsx — 通用确认弹窗。
- web/src/admin/security/SecurityPolicyPage.tsx — 独立安全策略写操作。
- web/src/admin/kyc/KycManagementPage.tsx — 独立 KYC 审核/配置写操作。
- web/src/admin/support/AdminSupportPage.tsx — 动作级权限正向样例。
- web/src/admin/support/OnlineSupportWorkbench.tsx — 轮询、并发保护和幂等写入。
- web/src/admin/settings/query.ts — 设置查询 key 和 mutation defaults。
- web/src/admin/settings/useAdminSettingsEditor.ts — 设置编辑 mutation 生命周期。
- web/src/shared/QuillRichTextEditor.tsx — 富文本依赖和 DOM 处理点。
- web/src/app/router.tsx — 顶层路由拆包。
- web/src/admin/resources/resourceConfigs.test.tsx — 通用资源跨域大测试文件。
- web/src/admin/auth/LoginPage.test.tsx — 登录测试 QueryClient 配置。
- web/src/admin/auth/RequireAdmin.test.tsx — 管理员守卫测试。
- web/src/shared/format.test.tsx — 当前金额格式化测试。
- web/src/shared/ResizableTable.test.tsx — 列宽拖拽与键盘测试。
- web/package.json — 前端命令和依赖声明。
- web/vite.config.ts — Vite/Vitest 配置。
- web/eslint.config.js — ESLint flat config。
- web/index.html — 浏览器标题和 HTML shell。
- web/.env — 当前仓库 API 域变量。

## Code Patterns

1. **身份无关的固定 Query key**：web/src/admin/access.tsx:168-190；与 web/src/admin/layout/AdminLayout.tsx:188-194 的“登出不清缓存”组合后形成跨身份复用窗口。
2. **组件内存承担金融命令身份**：web/src/admin/resources/actions/users.tsx:85-127；组件生命周期被错误地当作业务命令生命周期。
3. **能力并集代替动作级授权**：web/src/admin/access.tsx:158-162 与 web/src/admin/resources/resourceConfigs.tsx:1453-1467。
4. **错误降级为空数据**：web/src/api/adminResources.ts:35-48、web/src/admin/resources/actions/shared.tsx:209-285；契约和网络错误失去可观察性。
5. **行组件 eager data fetch**：web/src/admin/resources/actions/users.tsx:85-89、248-263；共享目录未通过 Query cache 去重。
6. **十进制值转换 Number**：web/src/shared/numberFormat.ts:50-60、web/src/admin/resources/actions/loan.tsx:145-179、wallet.tsx:158-197。
7. **原始 WebSocket 生命周期留给每行组件**：web/src/admin/resources/marketTickerSocket.ts:51-69 与 resourceConfigs.tsx:506-519。
8. **单一跨域配置注册表**：web/src/admin/routes.tsx:9-29 与 web/src/admin/resources/resourceConfigs.tsx:1-34；路由懒加载粒度仍停留在整个资源系统。

## External References

- TanStack Query useMutation retry 语义：https://tanstack.com/query/latest/docs/framework/react/reference/useMutation
- TanStack Query query key 设计：https://tanstack.com/query/latest/docs/framework/react/guides/query-keys
- Cloudflare Turnstile 服务端验证与 token 单次使用：https://developers.cloudflare.com/turnstile/get-started/server-side-validation/
- WAI-ARIA Tabs Pattern：https://www.w3.org/WAI/ARIA/apg/patterns/tabs/
- WHATWG WebSockets：https://websockets.spec.whatwg.org/
- Vite 环境变量：https://vite.dev/guide/env-and-mode.html
- Vite 构建选项与 chunk warning：https://vite.dev/config/build-options.html
- OWASP HTML5 Security Cheat Sheet（Web Storage 风险）：https://cheatsheetseries.owasp.org/cheatsheets/HTML5_Security_Cheat_Sheet.html

## Related Specs

- .trellis/spec/admin/ui-system.md:14-17 — 中文用户文案和禁用英文装饰文案。
- .trellis/spec/admin/ui-system.md:152-162 — tab/tabpanel、重复控件记录级可访问名称。
- .trellis/spec/admin/ui-system.md:420-452 — 支持工作台有界 polling 与 stale response 保护。
- .trellis/spec/admin/ui-system.md:505-522 — 充值幂等 intent 冻结、失败重试复用、参数变化换键、409 反馈及测试要求。
- .trellis/spec/admin/ui-system.md:526-529 — 可调整列宽测试要求。
- .trellis/spec/admin/index.md:29-30 — 用户可见 copy 使用中文。
- .trellis/spec/backend/wallet-amount-precision.md:12-21 — 0..18 位精度与 DECIMAL(38,18)。
- .trellis/spec/backend/auth-sessions.md:47-48 — 客户端 access/refresh 与 401 refresh/replay 契约。
- .trellis/tasks/08-30-p0-release-blockers-remediation/research/p0-02-financial-idempotency.md:23-49 — 服务端按 BigDecimal normalized 指纹、前端失败重试稳定 key 的既有研究。
- .trellis/tasks/08-30-project-code-business-optimization-reaudit/research/admin-pc-cross-layer.md — 2026-08-30 管理端跨层基线。

## Caveats / Not Found

1. 按研究代理约束未执行 git status、git diff、git log 或其他 git 操作；“CURRENT HEAD”在本文中指当前可见源码快照，不能证明工作树清洁，也不能把“新增识别”归因到特定提交。
2. 本轮范围仅为管理后台前端。服务端权限、DTO、幂等指纹、session 撤销和 WebSocket 协议未重新审计；相关服务端语义只引用既有 spec/research。
3. 未连接真实浏览器、生产 API、WebSocket、Turnstile、角色数据或部署 CDN/反向代理；所有依赖这些条件的影响均已标注为运行时假设。
4. 生产构建使用 write=false 的内存输出，没有生成 dist，也没有做部署 smoke、真实 source-map/缓存或 Web Vitals 测量。
5. lint、type-check 和 382 个测试通过不代表故障模型被覆盖；FAD-P2-03 记录了测试策略与生产策略之间的具体差异。
6. npm ls 基于现有 node_modules，未运行 npm ci；其输出另有 3 个 extraneous 间接包（@emnapi/wasi-threads、@floating-ui/core、@floating-ui/utils），本轮没有证据把它们认定为产品缺陷。
7. 除本 research 文件外，未修改生产代码、测试、配置、spec、既有 research 或进度文件。
