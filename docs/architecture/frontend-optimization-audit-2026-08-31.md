# 前端待完善项增量复审（2026-08-31）

> **结论：当前确认 2 组 P0、11 组 P1、6 组 P2；生产发布继续 HOLD。** 两组 P0 分别是 PC 充值地址选择的乱序响应，以及 Admin/PC 资金命令在不确定结果后的幂等意图不可恢复。其余问题按正确性、可用性、交付和体验分级，不以文件大小或测试数量代替风险判断。
>
> **审计范围：** `mobile/`、`pc/`、`web/` 的路由、认证、API/DTO、资金精度、请求与 WebSocket 生命周期、国际化、无障碍、PWA/Tauri、测试与生产构建；未连接生产数据库或执行资金操作。

## 1. 执行摘要

### 1.1 必须先修的 P0

| ID | 前端 | 问题 | 直接影响 | 最低退出条件 |
| --- | --- | --- | --- | --- |
| FE-P0-01 | PC | 充值币种/网络请求没有 generation，旧响应可以覆盖新选择 | 页面可能显示“网络 B”但地址/二维码属于网络 A，形成不可恢复链上转账风险 | 选择快照、Abort/generation、地址/二维码/网络标签原子 view model；A→B 乱序行为测试通过 |
| FE-P0-02 | Admin + PC | 资金命令的客户端 intent 只驻留组件/进程内存，且金额没有按十进制语义归一 | 响应丢失、超时、组件重挂载或刷新后，同一业务意图可能换幂等键并再次动账 | 可恢复 pending intent、十进制规范化、按 key 查询/对账；commit-before-timeout、reload、`25.50/25.5` 测试均只形成一次资金效果 |

### 1.2 建议执行顺序

1. **先关闭 FE-P0-01/02**，冻结 PC 充值相关发布和未验证的资金重试路径。
2. 修 Mobile 会话与行情冷启动竞态，再统一三端 Decimal string 和权威资金 DTO。
3. 修 PC/Admin WebSocket、新旧请求、权限和错误/陈旧态，删除假“实时”状态。
4. 补齐全量测试、PWA/Tauri/PC release gate 和配置 fail-closed。
5. 最后拆分超大页面、样式与 bundle，并完成无障碍和国际化收口。

## 2. P0 详细证据

### FE-P0-01 — PC 充值地址与当前网络可被乱序响应错配

- `pc/src/views/User/Recharge.vue::selectCoin` 先修改 `selectedCoin`，再等待网络列表；返回时不校验请求所属币种，直接覆盖 `availableNetworks` 并自动调用 `selectNetwork`。
- `selectNetwork` 也没有 request generation、AbortSignal 或 `{coin,network}` 当前性复核，旧地址响应可最后写入 `walletData`，异步二维码生成同样没有 identity guard。
- 模板的地址/二维码来自 `walletData`，警告和扫描说明来自当前 `selectedCoin/selectedNetwork`；复制按钮复制 `walletData.address`，因此并非同一原子快照。
- `pc/src/views/User/Withdraw.vue` 的币种/网络选择存在同型竞态，应复用同一 selector controller。

**整改边界：** 以 `{assetSymbol, networkKey, generation}` 作为不可变选择身份；取消 superseded request；只有当前 generation 可以提交网络目录、地址、二维码和 loading/error；请求未完成时禁用复制。行为测试必须让 B 先返回、A 后返回，并断言最终所有可见信息与复制值都属于 B。

### FE-P0-02 — Admin/PC 资金命令幂等只覆盖“同组件、同字面量”的重试

- `web/src/shared/idempotency.ts` 对字符串仅 `trim()`；`25.50` 与 `25.5` 会生成不同 intent。
- Admin 充值在每个 `UserRechargeAction` 行组件里用 `useRef` 创建内存 manager；行卸载、账号切换、刷新页面后 pending key 丢失。
- PC 共用 10 秒请求超时，但杠杆开仓、秒合约、理财、借贷、新币、预测、提现等路径会在每次调用时重新生成 key；仅现货下单与杠杆划转使用 retry-stable helper，而且仍只在内存中。
- 若服务端已经提交、响应随后丢失，用户按相同业务含义重试但携带新 key，后端会把它视为新命令。服务端强制“必须有 key”不能替代客户端“同一意图必须复用 key”。

**整改边界：** 建立 session-scoped、可恢复的 `FinancialCommandIntent`；金额全程 Decimal text 规范化；网络失败、timeout、5xx、刷新和组件重挂载都保留 key；成功或用户明确改变业务意图后才轮换；不确定结果先按 key 查询/REST 对账。所有高价值 mutation 都要覆盖 response-drop 和 reload 测试。

## 3. P1 正确性、可用性与交付缺口

| ID | 范围 | 当前缺口 | 建议 |
| --- | --- | --- | --- |
| FE-P1-01 | Mobile 行情 | `market.refresh()` 在 loading 时返回已完成 Promise而不是 join 首请求；路由 A→B 冷启动可留下 REST 快照但不启动共享 ticker 流 | 保存 `refreshPromise`，抽出幂等 `ensureLive()`，由 store 而非页面持有期望 lease；加入 deferred route-switch 测试 |
| FE-P1-02 | 会话/身份 | Mobile refresh 缺 session epoch/CAS，logout 后旧 refresh 可复活 token并重放请求；Admin access query key 不含管理员身份且登出不清缓存；PC 仍有 storage/Pinia 双事实 | 单一 Session owner + generation；refresh compare-and-swap；登出取消旧请求/WS并清身份缓存；跨标签同步 |
| FE-P1-03 | Decimal | Mobile mutation 与 Wallet Ledger、PC 资金 DTO、Admin formatter/比较广泛使用 `number`/`Number`；已复现 `1e-18` 显示 0、超过 `2^53` 余额改变、Admin 小额显示 `NaN` | transport/domain 使用 branded Decimal text；Decimal 库按资产 `precision_scale` 截断；非法/缺失/零值三态分离 |
| FE-P1-04 | PC 权威资金合同 | 提现不获取/提交服务端 `quote_id`；杠杆 cross/risk snapshot 被旧 view model 丢弃；秒合约 settlement price/evidence 被置 0并本地重算收益 | 生成或窄类型 DTO；直接呈现后端 risk、settlement、payout/ledger 字符串；缺证据显示未知而不是 0 |
| FE-P1-05 | 实时连接 | PC token 轮换存在旧 socket `onclose` 覆盖新 socket 的 ABA，已确定性复现；Admin 行情一行一个 socket且无重连/freshness；Mobile 私有 WS 无入站沉默 watchdog | generation/source identity、heartbeat/watchdog、指数退避+jitter、引用计数；UI 暴露 live/stale/offline 与最后消息时间 |
| FE-P1-06 | Admin 认证 | `AppProviders` 为全部 mutation 设 `retry: 1`，登录和 2FA 会自动重放，并复用一次性 Turnstile token | 全局 mutation 默认不重试；只对明确幂等操作局部启用；登录/2FA 单击只发一次请求 |
| FE-P1-07 | Admin 权限 | 通用资源只要拥有任一写能力就展示整组操作；多个独立页面只有读权限路由守卫，保存/审核入口缺动作级门控 | 每个 action 声明精确 permission；建立 read/review/operate/write 角色矩阵测试；后端继续 fail closed |
| FE-P1-08 | API/请求状态 | Admin 宽松 DTO 把错 response key 降为 `[]`，行级目录形成 N 次不可取消请求；PC 错误被吞成空/旧态且存在伪分页；Mobile Orders tab 无 generation；Mobile 还直接显示后端原始 `message` 而忽略稳定 `code` | 窄 DTO/schema validation；共享 query + AbortSignal；统一 `loading/error/stale/lastSuccessfulAt`；错误按 code 本地化，5xx 仅展示安全文案 |
| FE-P1-09 | Mobile 现货批量撤单 | 仍以 N 次单撤模拟后端已存在的 `DELETE /spot/orders`，只抛第一个失败，不能表达部分成功 | 调用批量端点并消费 `orders[]/failures[]`；显示成功/失败数量和剩余风险订单 |
| FE-P1-10 | 发布/配置 | PC 无环境变量时静默指向真实生产域名；Web `.env` 与代码读取的变量名不一致；PC updater pubkey、capability、process plugin、artifact 未闭环；Mobile/PC Tauri CSP 为 null | 构建环境 fail-closed；统一 REST/WS origin；完成 updater 签名/ACL/插件或禁用入口；建立最小 CSP并做 staging smoke |
| FE-P1-11 | 质量门禁 | PC 标准 gate 只跑一个测试文件，全量 97 项有 5 项失败；Mobile 90 个测试中 76 个读取源码；Mobile gate 不构建 PWA/Tauri；三端都缺关键竞态/资金行为与包体门禁 | 单一 release script 执行 type/lint/全测/build；组件/E2E/deferred promise/fake socket/SW 测试；测试 TS 纳入 type-check；关键资金分支设 coverage |

## 4. P2 体验、性能与维护缺口

### FE-P2-01 — Bundle 与首屏资源缺预算

- Mobile PWA 构建 precache 为 144 项、约 3.9 MB；`signal-theatre.png` 单文件约 1.7 MB并进入 precache。Ego Browser 390px 冷启动资源传输约 2.8 MB。
- Admin 入口 JS 约 1.61 MB raw / 438 KB gzip，CSS 约 555 KB raw，`resourceConfigs` 约 219 KB raw；任一通用资源页会拉取跨业务配置。
- PC 主入口和两个图表 chunk 分别约 407 KB、376 KB、170 KB raw，且没有 raw/gzip/brotli budget。
- 应把大型位图转为响应式 WebP/AVIF，按业务域拆 resource config/style/图表依赖，并让 CI 对包体 delta 失败。

### FE-P2-02 — 巨型生命周期 owner 仍在增长

- Mobile `TradeView.vue` 6,125 行、`SecondsView.vue` 3,395 行、`AssetsView.vue` 2,086 行；三个共享样式文件合计 12,609 行。
- PC `backendAdapters.ts` 2,101 行、i18n 2,222 行；Admin `resourceConfigs.tsx` 1,469 行、`styles.css` 2,721 行。
- 应先建立行为测试，再按 session、market、financial intent、dialog、domain adapter 和 CSS layer 小步拆分，避免机械切文件。

### FE-P2-03 — 无障碍与触控目标不完整

- Mobile 根壳与 39 个页面重复使用 `<main>`；路由切换没有 title/focus/announcement；Seconds `listbox` 缺方向键/roving tabindex。
- Ego Browser 实测首页部分分类按钮宽度仅 22–33px，新闻分类约 26×26px，交易/秒合约部分按钮高度 30–32px，低于 44px 移动触控建议值。
- PC 多个可点击 `span/div` 和资金弹窗缺 dialog/focus/Escape 语义；Admin 秒合约 Tabs 的 `aria-controls` 指向不存在的 panel。
- 应统一 Button/Dialog/Tabs/Listbox primitive，并用 axe + keyboard-only E2E 覆盖登录、下单、撤单、平仓和审核。

### FE-P2-04 — 国际化与状态真实性仍不完整

- Mobile 双语 key 已对称，但邀请、快捷充值、闪兑、理财等页面会直接显示后端英文 enum；KYC 未知状态被错误映射为 `pending`。
- PC 英文缺 `market.high/low/turnover`，仍有硬编码英文、宿主 locale 格式化和固定 `<html lang="en">`。
- Admin 浏览器标题仍为 `HIPPO Operations`，且 API 环境变量命名漂移。
- 应建立 typed enum presentation adapter、未知值保留策略、locale parity 与用户文案静态门禁。

### FE-P2-05 — 资源与 PWA 状态生命周期

- Mobile KYC 的证件预览 `URL.createObjectURL` 从不 revoke；替换和离页会保留敏感 Blob 与内存。
- PWA 更新发送 `SKIP_WAITING` 后只等待 `controllerchange`，没有 timeout/error 恢复，可能永久保持 busy。
- 首次进入完成启动动画后立即出现安装卡片，建议增加首会话延迟、频控和“用户完成首个价值动作后提示”的策略。

### FE-P2-06 — PC 首页存在生产感知误导

- Ego Browser 实测 PC 首页仍显示固定 `$1,245,678,901`、`+12.5%`、旧英文新闻、`Stable Connection` 和固定 footer volume。
- 这些内容没有 freshness/source，断线或 API 失败时仍像真实数据。应改为真实公共 API、明确 demo 标签，或在数据不可用时显示 skeleton/error/stale，禁止伪装为 live。

## 5. 本轮验证结果

| 前端 | 验证 | 结果 |
| --- | --- | --- |
| Mobile | `npm run type-check` | 通过 |
| Mobile | `npm test` | 538/538 通过；但 76/90 测试文件以源码读取为主 |
| Mobile | `npm run build:pwa` | 通过，2095 modules；precache 约 3.9 MB |
| PC | `npm run type-check` | 通过 |
| PC | `npm run build` | 通过 |
| PC | `node --test --experimental-strip-types tests/*.test.ts` | **失败：92/97 通过，5 项失败**；标准 gate 未运行这些失败项 |
| Admin Web | lint / typecheck / Vitest | 通过；53 files、382 tests |
| Admin Web | production build | 通过；存在 direct-eval 与大 chunk 警告 |
| Ego Mobile | 390×844、320×568 关键路由 | 文档宽度无横向溢出、无破图、按钮均有可访问名称；发现小触控目标和首屏资源偏大 |
| Ego PC | 1366×768 首页 | 无横向溢出；确认固定“实时”数据和不可访问的 footer 更新入口 |
| Ego Admin | 1440×900 登录页 | 无横向溢出，输入可访问名称完整；Turnstile 正常渲染 |

## 6. 已验证的正向项

1. Mobile 路由使用 lazy import、catch-all 和安全内部 redirect；本轮没有发现新的功能型跳转断裂。
2. Mobile 公开 ticker/detail transport 已有 socket identity 和入站沉默 watchdog；问题位于 store 启动编排，不是否定 transport。
3. Mobile PWA 没有 runtime cache API/WS/资金响应，navigation fallback 也排除了 API/WS/health/download。
4. Admin 表格统一走 `ResizableTable`，列宽拖动、键盘 separator 和操作列最小宽度已有测试。
5. Admin 设置编辑器已局部关闭 mutation retry，并保留 revision/reason/冲突处理。
6. 三端类型检查和生产构建都可以完成；当前主要问题是正确性/故障行为和发布门禁覆盖，而非普通编译失败。

## 7. 推荐任务拆分

| 顺序 | 建议任务 | 主要范围 |
| ---: | --- | --- |
| 1 | `pc-deposit-selector-generation-safety` | FE-P0-01 |
| 2 | `frontend-financial-intent-recovery` | FE-P0-02，Admin + PC 高价值 mutation |
| 3 | `mobile-session-market-lifecycle` | FE-P1-01/02 |
| 4 | `frontend-decimal-contracts` | FE-P1-03/04 |
| 5 | `frontend-realtime-freshness` | FE-P1-05 和 PC 假 live 内容 |
| 6 | `admin-auth-permission-query-policy` | FE-P1-06/07/08 |
| 7 | `client-release-gates` | FE-P1-10/11，PWA/PC/Tauri/Web |
| 8 | `frontend-accessibility-i18n-performance` | FE-P2-01..06 |

## 8. 详细研究证据

- Mobile：`.trellis/tasks/08-30-project-code-business-optimization-reaudit/research/frontend-mobile-delta-2026-08-31.md`
- PC：`.trellis/tasks/08-30-project-code-business-optimization-reaudit/research/frontend-pc-delta-2026-08-31.md`
- Admin Web：`.trellis/tasks/08-30-project-code-business-optimization-reaudit/research/frontend-admin-delta-2026-08-31.md`

## 9. 限制

- 未连接生产 API、数据库、真实用户资产或真实多实例 WebSocket；涉及实际发生率和历史损失的结论仍需运行证据。
- 未打包/安装本轮 PC Tauri、Mobile Android/iOS/desktop 原生制品，也未验证生产 updater、CSP、安全响应头或反向代理。
- Ego Browser 运行时检查使用本地 production preview；浏览器扩展产生的日志已排除，没有归因到应用。
- P0 是根据可达控制流和直接资金后果判定；上线前仍需用 commit-before-timeout、response-drop、乱序响应与真实后端 fixture 关闭运行证据。
