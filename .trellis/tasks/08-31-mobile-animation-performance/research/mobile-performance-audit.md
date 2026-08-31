# Research: mobile 静态性能与动画可靠性审计

- Query: 对 `mobile/` 做静态性能与动画可靠性审计；重点覆盖加载旋转动画停住、CSS/JS 动画、主线程长任务、WebView 兼容与生命周期、重复计时器/rAF/Observer、不可见页面实时订阅、图表更新、大列表/深层响应式、重型滤镜与背景、同步存储和启动资源；并补充资产长数字、稳定 GET 去重缓存及 KYC 国家搜索复用建议。
- Scope: mixed（项目内静态代码、既有构建产物与官方外部文档；未修改生产代码）
- Date: 2026-08-31

## Findings

### 1. 结论与优先级

| 优先级 | 发现 | 直接影响 | 建议先做 |
|---|---|---|---|
| P0 | Home/Markets 常驻全屏 `SignalField` 每帧做 Canvas 全量绘制 | 低端 WebView 主线程/绘制线程被占用时，CSS spinner 虽仍在计时但画面可能长时间不更新，看起来“停住” | 做真机 A/B：关闭 SignalField、限制 15/30 FPS、降低 DPR/粒子/网格后比较长任务与帧间隔 |
| P1 | `prefers-reduced-motion` 下 spinner 被明确设为 `animation: none` 或仅运行 1ms/1次 | 开启系统“减少动态效果”时，静态加载图标会被误报为卡死 | 保留文字/进度/骨架等不依赖旋转的加载状态，并加入 reduced-motion 回归 |
| P1 | 页面进入后台后，多条 WebSocket、Support 轮询和 Seconds 1秒时钟仍运行 | 隐藏页继续解析消息、触发响应式更新和网络活动；恢复时可能积压或重复刷新 | 统一 `visibilitychange` + `pagehide/pageshow` + 原生 WebView 生命周期策略，隐藏时暂停 UI 流与轮询，恢复时单次重连/对账 |
| P1 | `LaunchIntro` 首屏同步挂载、GSAP 进入主包、全屏层最多阻塞 3 秒 | 冷启动慢或 WebView 暂停计时器时，首屏交互被遮罩；动画兼容失败时用户感知为“卡住” | 将装饰性启动动画与应用可交互状态解耦；验证超时、后台/前台和异常路径必定释放遮罩 |
| P1 | Markets 首次为全部交易对并发请求 sparkline，ticker 每条消息都写深层响应式数组 | 产品数/推送频率上升后形成网络突发和高频全列表重算 | sparkline 只取可见项并限并发；ticker 合并为每帧至多一次批量提交 |
| P1 | SupportChat 每 5 秒串行请求会话和最多 100 条消息，隐藏页不暂停；消息分组存在重复排序和渐进复制 | 长会话下产生持续请求、O(n²) 分配和大量 DOM | 隐藏时停止轮询；按增量游标拉取；线性分组并对长历史虚拟化 |
| P2 | MarketDetail 的 K 线在父子层重复标准化/均线计算，图表深度 watch 仍扫描整组数组 | 高频 K 线下浪费 CPU；主题切换会全量 `setData` 5 条序列 | 只在一个层级派生数据；实时路径保持 `series.update()`，避免重复全量派生 |
| P2 | 多处大面积 `backdrop-filter`/渐变/背景图，且 WebKit 前缀不一致 | 老 WKWebView 可能丢样式，新设备也可能增加离屏渲染和显存压力 | 真机分层/过绘制测试；为不支持或低性能设备提供无模糊降级 |
| P2 | 路由重新挂载时稳定公共/产品目录 GET 重复请求 | 页面切换产生可避免的 RTT、JSON 解析和加载闪烁 | 对白名单 API 做内存 TTL + in-flight 去重；强一致数据明确不缓存，详见末尾新增研究项 |
| P3 | 少量一次性 `setTimeout` 未保存句柄，重复点击可叠加回调 | 离开页面后仍可能提示或跳转 | 保存句柄并在卸载时清理；行为测试覆盖快速返回 |

### 2. Files found

| 文件 | 一句话说明 |
|---|---|
| `mobile/src/components/SignalField.vue` | Home/Markets 全屏 Canvas 背景，每帧绘制网格、4 条波形和 28 个粒子。 |
| `mobile/src/components/LaunchIntro.vue` | 启动 GSAP 时间线、同步 `sessionStorage`、全屏阻塞层和 3 秒兜底。 |
| `mobile/src/App.vue` | 总是挂载 LaunchIntro，并按路由挂载 SignalField；路由组件按 `fullPath` 重新创建。 |
| `mobile/src/styles/base.css` | 全局 reduced-motion 将动画压到 1ms、1 次。 |
| `mobile/src/api/marketTickerStream.ts` | 共享 ticker WebSocket、心跳与重连；消息逐条提交监听者。 |
| `mobile/src/api/marketDetailStream.ts` | 深度/K线按 rAF 合并，成交逐条回调；无隐藏页挂起。 |
| `mobile/src/api/privateUserStream.ts` | 私有事件流心跳、重连与租约清理；无页面可见性策略。 |
| `mobile/src/views/HomeView.vue` | 订阅全市场 ticker，视图只展示部分数据；组件卸载才释放。 |
| `mobile/src/views/MarketsView.vue` | 全市场列表、全部 symbol 的 sparkline 并发加载与排序。 |
| `mobile/src/stores/market.ts` | 每条 ticker 用 `findIndex` 更新深层响应式数组。 |
| `mobile/src/views/MarketDetailView.vue` | 实时详情流和父层 K 线派生。 |
| `mobile/src/components/MobileMarketChart.vue` | 再次标准化点位并计算移动平均线。 |
| `mobile/src/components/LightweightMarketChart.vue` | lightweight-charts 深度 watch、增量 update、全量 setData 与 ResizeObserver。 |
| `mobile/src/core/marketChartRuntime.ts` | 通过扫描数组判断 append/update/replace。 |
| `mobile/src/views/TradeView.vue` | 公私实时流、5秒保证金账户对账及可见性处理。 |
| `mobile/src/views/SecondsView.vue` | 多 symbol ticker、1秒时钟、Canvas sparkline、Resize/MutationObserver。 |
| `mobile/src/core/supportChat.ts` | 5秒单飞轮询、消息合并/排序/分组。 |
| `mobile/src/views/SupportChatView.vue` | 每轮两次请求、历史消息累积和完整 DOM 渲染。 |
| `mobile/src/core/marginAccountReconciliation.ts` | 5秒间隔、单飞与隐藏页跳过逻辑。 |
| `mobile/src/views/WalletLedgerView.vue` | 每页 30 条并持续累积，未虚拟化。 |
| `mobile/src/main.ts` | 首屏挂载与全局样式导入；PWA 初始化在挂载之后。 |
| `mobile/src/i18n/index.ts` | 中英文语言包都被同步导入。 |
| `mobile/vite.config.ts` | PWA Workbox 将 JS/CSS/HTML/图片纳入预缓存。 |
| `mobile/src-tauri/android/MainActivity.kt` | 只有 edge-to-edge/overscroll 设置，没有 WebView 前后台生命周期桥。 |
| `mobile/src/views/AssetsView.vue` | 资产摘要长数字当前使用 ellipsis；亮暗两张 hero 图都在 DOM。 |
| `mobile/src/views/RegisterView.vue` | 已实现可搜索国家弹窗、焦点管理和国际化地区名。 |
| `mobile/src/views/KycView.vue` | 国家仍使用原生 select，且与 KYC 状态并行重复拉取国家目录。 |
| `mobile/src/core/countrySearch.ts` | 可跨文字系统、按 ISO/后端名/本地化名分词匹配的共享过滤器。 |
| `mobile/src/core/modalDialog.ts` | 已有焦点圈闭、Escape 关闭、body 滚动锁和焦点归还。 |
| `mobile/src/api/*.ts` | 公共目录、产品目录、账户与行情 GET 定义，用于划定 TTL 白名单和排除项。 |

### 3. 加载旋转动画为何可能停住

#### P0：spinner 本身未停，但主线程/绘制未及时产帧

- `SignalField` 把 DPR 限到 2，且最大约 220 万像素，仍可能在高分屏手机上创建大画布（`mobile/src/components/SignalField.vue:10-13`、`mobile/src/components/SignalField.vue:40-50`）。
- 每一帧都会清空整张画布、循环绘制网格、绘制 4 条按 `x += 4` 采样的波形、更新 28 个粒子并创建渐变（`mobile/src/components/SignalField.vue:53-141`）。
- 动画使用无帧率上限的连续 `requestAnimationFrame`（`mobile/src/components/SignalField.vue:143-155`）。它有 `document.hidden` 暂停和卸载清理，这是正确的，但前台低端机仍会持续占用绘制预算（`mobile/src/components/SignalField.vue:175-220`）。
- 该背景出现在 Home/Markets 根页面（`mobile/src/App.vue:102-105`；`mobile/src/core/navigation.ts:99-118`），恰好也是常见初始加载与市场加载 spinner 出现的位置。
- 现有 motion 测试只用源码断言固定 28 粒子、4 波形、网格以及 visibility/cleanup 存在，没有真实帧预算或 spinner 时间推进断言（`mobile/tests/motion-parity.test.ts:13-46`）。

判断：transform spinner 通常可在合成线程执行，但全屏 Canvas、渐变、页面过绘制、JS 回调和 WebView 合成共享有限资源。发生长任务或连续昂贵绘制时，动画 `currentTime` 可能推进而画面无法按时呈现，所以“旋转停住”不能只排查 spinner CSS。

建议：

1. SignalField 按设备预算自适应：低端/省电模式禁用，或限 15/30 FPS、减小 DPR/粒子/波形采样；不要让装饰动画和业务 loading 争抢帧预算。
2. 以 `PerformanceObserver({type:'longtask'})`、rAF 帧间隔 p95/p99、连续丢帧最长时长为门槛；在同一加载过程记录 spinner `Animation.currentTime/playState`，区分“动画状态停了”和“未绘制”。
3. 测试 A/B 必须保持请求和数据一致，只切 SignalField，才能确认因果。

#### P1：reduced-motion 下 spinner 确实被静态化

- 全局规则将所有动画缩短为 `0.001ms` 且只执行一次（`mobile/src/styles/base.css:407-423`）。
- 多个加载图标进一步在 `prefers-reduced-motion: reduce` 下设置 `animation: none`，例如 `mobile/src/components/MobileMarketChart.vue:92-103`、`mobile/src/components/OrderBookPanel.vue:994-1004`、`mobile/src/views/DepositDetailView.vue:297-328`、`mobile/src/views/MarketDetailView.vue:1482-1499`。
- 静态审计未发现通用 `animation-play-state: paused` 逻辑；常规 spinner 主要只动画 `transform`。因此系统减少动态效果是最确定的“看起来停住”路径。

建议：尊重 reduced-motion，不强制恢复旋转；改用明确的“加载中”文本、骨架占位、已加载/总数或静态状态图形。测试需覆盖 OS 设置在应用启动前开启、运行中切换、WebView 返回前台三种路径。

#### P1：启动遮罩与生命周期

- `LaunchIntro` 在模块顶层同步导入 GSAP，并在 setup 读取/写入 `sessionStorage`（`mobile/src/components/LaunchIntro.vue:2`、`mobile/src/components/LaunchIntro.vue:14-33`）。
- GSAP 时间线包含 `clipPath` 等可能触发重绘/兼容差异的属性，并用 3 秒 `setTimeout` 兜底结束（`mobile/src/components/LaunchIntro.vue:75-153`）。
- 全屏层为 fixed、`pointer-events: auto`、`touch-action: none`，结束前会拦截交互（`mobile/src/components/LaunchIntro.vue:208-216`）；App 无条件挂载它（`mobile/src/App.vue:102`）。
- 组件有 reduced-motion 立即结束、try/catch、超时和卸载清理，属于正向防护（`mobile/src/components/LaunchIntro.vue:64-76`、`mobile/src/components/LaunchIntro.vue:159-163`），但浏览器进入后台时 JS 时间线与 timer 都可能被节流。

建议：业务壳先可交互，启动动画作为可取消装饰层；`visibilitychange/pagehide/pageshow` 后直接完成或按绝对 deadline 结算，不依赖“后台 timer 最终会按时触发”。

### 4. 重复计时器、rAF、ResizeObserver、MutationObserver

#### 已有正确模式

- `marketDetailStream` 将 depth 和 kline 合并为每帧一次，并在停止时取消待执行 frame（`mobile/src/api/marketDetailStream.ts:279-316`、`mobile/src/api/marketDetailStream.ts:325-340`）。
- `SignalField` 只有一个 rAF 链，隐藏时取消、恢复时重启，卸载时清理（`mobile/src/components/SignalField.vue:143-220`）。
- `LightweightMarketChart` 与 Seconds 的 ResizeObserver 均在卸载时 disconnect；Seconds 的 MutationObserver 也有 disconnect（`mobile/src/components/LightweightMarketChart.vue:188-194`、`mobile/src/components/LightweightMarketChart.vue:308-319`、`mobile/src/views/SecondsView.vue:499-508`、`mobile/src/views/SecondsView.vue:952-953`）。
- 邮箱验证码倒计时保存单一句柄并清理，例如 `mobile/src/views/RegisterView.vue:118-124`、`mobile/src/views/RegisterView.vue:196`。

#### P1/P2 问题

- SupportChat controller 每 5 秒 tick，虽有 active + in-flight 单飞保护，但隐藏页不暂停（`mobile/src/core/supportChat.ts:158-193`）。每次 tick 先查当前会话，再取最多 100 条消息，形成两次串行 HTTP（`mobile/src/views/SupportChatView.vue:165-221`），只在卸载时停止（`mobile/src/views/SupportChatView.vue:473-485`）。
- Seconds 用 1 秒 interval 更新当前时间和触发到期对账（`mobile/src/views/SecondsView.vue:924-930`），隐藏页仍唤醒；每次更新会让所有活动订单倒计时/进度派生重算。
- 保证金账户 reconciliation 每 5 秒 interval；隐藏时能跳过网络，是正确保护，但 interval 自身仍唤醒（`mobile/src/core/marginAccountReconciliation.ts:71-80`、`mobile/src/core/marginAccountReconciliation.ts:207-212`）。Trade 每轮还会对 eligible positions 并行取风险（`mobile/src/views/TradeView.vue:590-618`）。
- PWA 初始化维护应用全生命周期事件监听与每小时检查 interval（`mobile/src/pwa/index.ts:167-185`）；开销低，但应确保初始化严格幂等并在测试/HMR 中不重复注册。
- `ReferralsView`、`LanguageView`、`DepositDetailView`、`SecurityView`、`ProfileView` 有未保存的一次性 timer；`ForgotPasswordView` 的延迟导航尤其可能在离页后生效（例如 `mobile/src/views/ForgotPasswordView.vue:67`）。

未发现：重复创建但未 disconnect 的 ResizeObserver/MutationObserver；未发现两个并行 SignalField rAF 链。风险集中在“合法单例仍做太多工作”和“隐藏页不暂停”，而不是典型句柄泄漏。

### 5. 不可见页面实时订阅与 WebView 生命周期

- 共享 ticker 流维护租约、心跳、重连并在最终 lease 释放时关闭，结构正确（`mobile/src/api/marketTickerStream.ts:97-147`、`mobile/src/api/marketTickerStream.ts:287-309`）；但每条消息立即遍历 listeners，且没有 visibility/freeze 策略（`mobile/src/api/marketTickerStream.ts:233-254`）。
- detail 流的成交 `onTrade` 仍逐条回调，depth/kline 才按帧合并（`mobile/src/api/marketDetailStream.ts:279-316`）。
- private stream 有 heartbeat/reconnect/停止清理，但无隐藏页处理（`mobile/src/api/privateUserStream.ts:154-178`、`mobile/src/api/privateUserStream.ts:239-313`）。
- Home、Markets、MarketDetail 均只在组件卸载时释放实时流（`mobile/src/views/HomeView.vue:336-349`、`mobile/src/views/MarketsView.vue:140-153`、`mobile/src/views/MarketDetailView.vue:455-469`）。
- Trade 的 visibility handler 只保护保证金 HTTP 对账；公开详情流、共享 ticker、私有 socket 没有随隐藏暂停（`mobile/src/views/TradeView.vue:1476-1585`）。
- Seconds 同时订阅产品和活动订单 symbol，全部只在 `onBeforeUnmount` 停止，没有 visibility 分支（`mobile/src/views/SecondsView.vue:334-393`、`mobile/src/views/SecondsView.vue:934-953`）。
- Android `MainActivity` 没有 `onPause/onResume` 向 Web 层传递状态，Rust 壳也无生命周期协调（`mobile/src-tauri/android/MainActivity.kt:8-17`、`mobile/src-tauri/src/lib.rs:1-5`）。

建议统一状态机：

1. `visible + active route` 才消费 UI 高频流；hidden/pagehide 时停止 UI listener、轮询和 1秒时钟，必要时关闭 socket。
2. resume/pageshow 时只启动一个 lease，做一次权威 REST snapshot/账户对账后再接增量流；用 generation token 丢弃后台旧请求结果。
3. 对 bfcache 使用 `pagehide/pageshow`，对冻结使用 Page Lifecycle；Tauri/WKWebView/Android WebView 增加原生 pause/resume 信号作为补充。
4. 监控指标：隐藏期间请求数、WS frame 数、响应式 commit 数均应为 0（保活若为明确产品需求则单列）；恢复后 socket 数量必须回到 1，不能递增。

### 6. 行情、图表更新频率与深层响应式

#### Markets（P1）

- 首次加载会针对所有市场 symbol 运行 `fetchKlines(symbol, '15m', 24)`，通过 `Promise.allSettled` 同时发出，没有可见项过滤和并发上限（`mobile/src/views/MarketsView.vue:115-138`）。
- 列表每次 ticker 更新都会复制/过滤/排序 tickers（`mobile/src/views/MarketsView.vue:46-56`），模板完整渲染列表而无虚拟化（`mobile/src/views/MarketsView.vue:204`、`mobile/src/views/MarketsView.vue:299`）。
- store 对每条 ticker 做一次 `findIndex` 并写入深层响应式数组（`mobile/src/stores/market.ts:43-54`）；共享流又把每条消息立即分发，缺少每帧批处理。

建议：sparkline 按 viewport/分页加载、并发 4–6、缓存 symbol+interval；ticker 先写普通 Map，再用单一 rAF 批量提交当前帧变化；排序只在节流点执行。

#### Seconds（P1/P2）

- ticker 回调每次用对象展开替换完整 `liveTickerSnapshots` 根对象（`mobile/src/views/SecondsView.vue:379-390`）；任何 symbol 更新都会使依赖根对象的计算重新求值。
- Canvas sparkline 在数据 watch、ResizeObserver 和主题 MutationObserver 路径触发同步重画，并在绘制时读取 computed style/重设 canvas 尺寸（`mobile/src/views/SecondsView.vue:429-508`、`mobile/src/views/SecondsView.vue:919-922`）。
- Observer 都有清理，未确认存在 resize loop；但在老 WebView 必须压测“设置 canvas width/height 是否再次触发 observer”的循环告警。

建议：Map 原地记录后每帧一次生成视图快照；绘制路径统一进一个 dirty-frame scheduler，尺寸/主题/数据只置 dirty 标记；隐藏时不绘制。

#### MarketDetail/lightweight-charts（P2）

- 父层标准化 K 线并计算 MA（`mobile/src/views/MarketDetailView.vue:98-100`），`MobileMarketChart` 又做一次同类标准化/MA（`mobile/src/components/MobileMarketChart.vue:20-24`）。
- lightweight chart 对 points/averages 使用 `{ deep: true }` watch，并调用分类逻辑扫描数组（`mobile/src/components/LightweightMarketChart.vue:274-302`、`mobile/src/core/marketChartRuntime.ts:16-44`）。
- 实时 append/update-last 已使用 `series.update()`，这是正确模式（`mobile/src/components/LightweightMarketChart.vue:139-147`）；全量 replacement/主题变更才对 5 个 series 做 `setData`（`mobile/src/components/LightweightMarketChart.vue:114-137`、`mobile/src/components/LightweightMarketChart.vue:159-185`）。

建议：让单一层拥有标准化和均线结果，父子传稳定浅引用；实时路径不重新构造全部 160 点数组；主题变化尽量只 applyOptions，确认数据颜色设计确需重建后才 setData。

### 7. 大列表与算法复杂度

- Support 消息合并先 Map+sort，分组又 sort，并在同一天每增加一条消息时复制一次当前数组（`mobile/src/core/supportChat.ts:82-124`），长单日会话形成 O(n²) 累计分配。模板渲染所有已加载消息（`mobile/src/views/SupportChatView.vue:625-649`）。
- WalletLedger 每页 30 条并把后续页追加到同一数组，模板完整渲染全部条目（`mobile/src/views/WalletLedgerView.vue:28-45`、`mobile/src/views/WalletLedgerView.vue:72-79`、`mobile/src/views/WalletLedgerView.vue:224-234`）。
- 当前 K 线通常约 160 点、Seconds/产品列表通常有 50 上限，尚不构成“大数据必然卡顿”；风险在高频重建和长期累计，不应仅按数组长度判断。

建议：Support 分组改为一次线性归并，消息按 ID 增量加入；500 条以上虚拟化。Ledger 采用虚拟列表或只保留窗口。大型近只读集合考虑 `shallowRef`，避免把后端大对象全部深度代理。

### 8. 重型滤镜、背景和 WebView 兼容

- 源码约有 256 处 `color-mix()`，并存在多处大面积 `backdrop-filter`；仅少量同时写 `-webkit-backdrop-filter`。
- Assets 多处使用 18px blur（如 `mobile/src/views/AssetsView.vue:965`、`mobile/src/views/AssetsView.vue:1090`、`mobile/src/views/AssetsView.vue:1681-1699`）；Register 全屏国家 picker mask 使用未加前缀的 10px blur（`mobile/src/views/RegisterView.vue:635-645`）；PWA 状态层使用 22px blur+saturate（`mobile/src/components/PwaStatus.vue:304-307`）。
- 一些全局 topbar/bottom nav blur 后续已覆盖为 none，不能把所有声明都当作最终生效（`mobile/src/styles/prototype-base.css:3661-3665`、`mobile/src/styles/prototype-base.css:3974-3984`、`mobile/src/styles/prototype-base.css:7751-7758`）。
- Vite 配置未声明特定 WebView 构建目标/兼容矩阵；Android 系统 WebView 版本由设备环境决定，壳层也未记录实际 WebView package/version。

建议：

- CSS 用 `@supports`/基础纯色作为基线，模糊只作为增强；老 WKWebView 同时考虑 `-webkit-backdrop-filter`，但前缀只解决支持性，不解决性能。
- 真机检查 GPU overdraw、layer 数量、显存、滚动时 frame gap；对低性能档禁用大面积 blur、复杂渐变和 fixed 背景。
- 启动时记录 WebView engine/package 版本、OS、reduced-motion、DPR 和关键 `CSS.supports()`，便于把“spinner 卡死”聚类到具体运行时。

### 9. 同步存储与启动资源

- 同步存储的最突出首屏路径是 LaunchIntro 的 `sessionStorage`；单个键成本通常小，但发生在启动关键路径，且存储异常只能依赖 catch（`mobile/src/components/LaunchIntro.vue:14-33`）。
- `main.ts` 同步导入多份全局 CSS并立即 mount，PWA 初始化在 mount 后（`mobile/src/main.ts:1-15`）。
- i18n 同时静态导入中英文大语言包（`mobile/src/i18n/index.ts:1-3`、`mobile/src/i18n/index.ts:32-40`）；源文件约 80 KB/语言，增加主包解析。
- App stage art 总是存在，Home 与 Assets 的亮/暗 hero 都以 `<img>`/`v-show` 同时进入 DOM，CSS 隐藏不等于不下载（`mobile/src/App.vue:75-80`、`mobile/src/views/HomeView.vue:387-388`、`mobile/src/views/AssetsView.vue:557-558`）。
- Workbox glob 包含 png 等图片，因此大图还进入预缓存（`mobile/vite.config.ts:86-101`）。现有 `mobile/dist` 快照中主 JS 约 485 KB（gzip 约 170 KB）、全局 CSS 约 227 KB（gzip 约 38 KB），`signal-theatre` 图片约 1.8 MB；该快照未在本次审计中重建，只能作趋势证据。

建议：语言包按 locale 异步加载；每个主题只渲染一张响应式图片；装饰大图从首屏/PWA precache 白名单移出或提供 AVIF/WebP 多尺寸；GSAP/LaunchIntro 延后或独立 chunk。建立冷启动预算：主 JS gzip、CSS gzip、首屏图片字节、FCP/LCP/TBT、可交互时间和 Service Worker 首装下载量。

### 10. 建议测试矩阵

1. **动画推进**：Android System WebView、iOS WKWebView、Tauri Android 真机；低端机或 4×/6× CPU throttling；light/dark；reduced-motion 开/关。加载 2 秒内每 250ms 采集 spinner `currentTime/playState` 和截图差异。
2. **长任务与帧预算**：SignalField on/off、60/30/15 FPS、DPR 1/2；统计 >50ms long task、p95/p99 frame gap、最长连续无新帧时长。
3. **生命周期**：分别从 Home、Markets、Trade、Seconds、SupportChat 进入后台 30 秒；断言隐藏期 HTTP=0、UI WS frame/commit=0、1秒计时器不更新；恢复后只有一次 snapshot/重连且 socket/interval 数不增长。
4. **高频行情**：50/200 symbols，ticker 10/50/100Hz；断言响应式提交每帧最多一次、排序频率受控、交互 INP 不恶化。
5. **Markets 首装**：200 symbols 下验证 sparkline 仅请求可见行、并发峰值≤6、滚动后渐进加载且离页可取消。
6. **图表**：160 点、20/60Hz update-last/append；断言实时路径不全量 `setData`、不重复 MA/normalize、缩放窗口不丢失。
7. **长列表**：Support 100/500/2000 条同日消息；Ledger 30×50 页；记录 JS heap、分组耗时、DOM 节点数和滚动帧率。
8. **启动**：首次安装、SW 更新、离线、弱网、后台启动再恢复；验证启动遮罩总能释放、业务壳可操作，并记录资源/预缓存字节。
9. **WebView 兼容**：覆盖当前支持范围内最老/最新 engine；检查 `ResizeObserver`、rAF、`color-mix`、backdrop-filter、`prefers-reduced-motion`、页面冻结/恢复。

### 11. External references

- [Chrome Page Lifecycle API](https://developer.chrome.com/docs/web-platform/page-lifecycle-api)：hidden/frozen 阶段应停止 UI 更新、计时器/轮询，并处理 WebSocket 等可冻结资源。
- [web.dev — Animations overview](https://web.dev/articles/animations-overview)：transform/opacity 通常更适合合成，layout/paint 属性会占主线程/绘制预算。
- [web.dev — Smoothness](https://web.dev/articles/smoothness)：长任务和未按帧完成的渲染会造成视觉卡顿。
- [Vue Performance Best Practices](https://vuejs.org/guide/best-practices/performance)：大列表虚拟化、减少深层响应式开销和稳定 props。
- [Lightweight Charts 5.0 documentation](https://tradingview.github.io/lightweight-charts/docs/5.0)：实时单点更新应使用 `update()`，避免持续 `setData()`。
- [Resize Observer specification](https://www.w3.org/TR/resize-observer/)：回调处于渲染流程中，尺寸写回可能触发后续观察轮次。
- [Android Jetpack Webkit overview](https://developer.android.com/develop/ui/views/layout/webapps/jetpack-webkit-overview) 与 [Managing WebView objects](https://developer.android.com/develop/ui/views/layout/webapps/managing-webview)：系统 WebView 能力/版本和 Activity/渲染进程生命周期需显式管理与检测。
- [WebKit — Introducing Backdrop Filters](https://webkit.org/blog/3632/introducing-backdrop-filters/)：backdrop filter 需要额外渲染通道，应控制使用范围。
- [WebKit Features in Safari 18.0](https://webkit.org/blog/15865/webkit-features-in-safari-18-0/)：较新 Safari 才提供无前缀 backdrop-filter，老 WKWebView 兼容需真机验证。

### 12. Related specs

- `.trellis/spec/mobile/index.md:178-184`：图表时间戳、ResizeObserver 零尺寸跳过、实时流会话隔离的移动端契约。
- `.trellis/spec/mobile/backend-integration.md:165-230`：WebSocket lease、最终关闭、depth/kline 按 rAF 合并和停止时清理待执行 frame。
- `.trellis/spec/mobile/pwa-and-shell.md`：PWA 壳、路由动效、私有保证金对账生命周期及启动行为约束。
- `.trellis/spec/mobile/navigation-and-localization.md`：路由生命周期、导航返回和本地化呈现约束。

## Caveats / Not Found

- 本报告是静态审计；没有修改或构建生产代码，也没有连接真机采集 trace。因此 SignalField 与 spinner 卡顿是有强代码证据的高概率因果假设，不是已完成性能剖析的定论。
- 既有 `mobile/dist` 资源大小是审计时工作区快照，可能不对应当前源码最新提交；报告只将其用于资源级别排序。
- 未发现通用 spinner 被 JS 设置为 paused，也未发现未清理的重复 ResizeObserver/MutationObserver 或双 rAF 链。
- WebView 最低版本、实际设备分布和后端推送频率在仓库中未找到；优先级需结合线上 telemetry 调整。
- 本次未核验后端 Cache-Control/ETag、目录接口是否因租户/地区/登录态返回不同内容；TTL 白名单上线前需要接口契约确认。

---

## 新增研究项（按 2026-08-31 用户要求追加）

### A. AssetsView `assets-member-summary` 长数字字体自适应且禁止省略号（P1）

#### 证据

- 总资产数值位于 `.assets-member-summary__value > strong`（`mobile/src/views/AssetsView.vue:562-580`）。当前规则固定 34px，并显式设置 `overflow: hidden; text-overflow: ellipsis; white-space: nowrap`（`mobile/src/views/AssetsView.vue:1035-1051`）。窄屏仅把字号固定改为 27px，仍不随实际字符宽度适配（`mobile/src/views/AssetsView.vue:2000-2007`）。
- 今日收益的 strong/small 同样设置 ellipsis（`mobile/src/views/AssetsView.vue:1061-1079`）。这会隐藏金融数字的最低位、符号或百分比信息；对资产摘要而言，省略号会改变用户能读取到的数值语义。
- 布局右列在常规宽度最少 84px，窄屏固定 78px，左列虽为 `minmax(0, 1fr)`，但超长数字没有可靠的可用宽度协商（`mobile/src/views/AssetsView.vue:987-993`、`mobile/src/views/AssetsView.vue:2000-2003`）。

#### 建议交互/实现约束

1. **禁止数值 ellipsis**：总资产、今日收益金额和比例移除 `text-overflow: ellipsis`，测试中直接断言 computed style 不为 ellipsis。完整数值必须在视觉和可访问文本中一致。
2. **按容器和字符宽度自适应，而非仅按 viewport**：推荐限定 34px→20px 的字号区间；在数值或容器宽度变化时测量一次文本宽度，用二分/分档选择最大可容纳字号。避免每帧测量；复用单个 ResizeObserver，并把读写拆到同一调度帧。
3. **简单方案可先用确定性分档**：按格式化后字符数（含符号、小数点、分组符）设置 size tier，再用 `clamp()`/容器查询结合 34/30/27/23/20px。该方案性能稳定，但必须用极端字体与本地化格式验证，字符数并不完全等于像素宽度。
4. 数值使用 `font-variant-numeric: tabular-nums`，单位允许独立缩小或移到下一行；当 20px 仍容纳不下时，优先让摘要两列改为上下布局或让单位换行，仍不得截断数字。
5. 隐藏余额占位（如 `****`）、负收益、正号、千分位、8–18 位整数、8 位小数、中文/英文与 320/340/390/430px 宽度都要进入视觉回归。

#### 建议测试

- 数据集：`0`、`-0.00000001`、`999,999,999,999.99999999`、18位整数+8位小数、`****`；断言完整 `textContent` 可见、无横向溢出、与右侧收益不重叠。
- 字号下限 20px、上限 34px；容器扩大后字号可恢复，主题/余额显隐切换不产生 ResizeObserver loop。
- VoiceOver/TalkBack 读取完整数值与单位；视觉缩放不能通过 `transform: scaleX()` 压扁文本。

### B. 页面切换时稳定 GET 的 TTL + in-flight 去重（P1）

#### 重复请求证据

- App 的路由组件以 `currentRoute.fullPath` 为 key，切换后返回会重新挂载并执行各 View 的 `onMounted(load)`（`mobile/src/App.vue:118-124`）。路由本身是异步分包，但没有 KeepAlive 或共享查询缓存（`mobile/src/router/index.ts:3-40`）。
- Register 和 KYC 都调用 `fetchCountries()`；KYC 每次进入还把国家目录与强一致 KYC 状态一起重拉（`mobile/src/views/RegisterView.vue:180-194`、`mobile/src/views/KycView.vue:101-108`）。
- Orders 和 Trade 都重复拉 `/margin/products`，Orders 的 tab 切换还会再次获取产品/市场目录（`mobile/src/views/OrdersView.vue:206-237`、`mobile/src/views/TradeView.vue:501-519`）。
- 充值链路 Asset → Network → Detail 连续三页都重新拉 `/wallet/deposit-assets`（`mobile/src/views/DepositAssetView.vue:28-39`、`mobile/src/views/DepositNetworkView.vue:25-39`、`mobile/src/views/DepositDetailView.vue:23-31`）。
- NewCoins 与 NewCoinRecords 都拉项目目录（`mobile/src/views/NewCoinsView.vue:36-48`、`mobile/src/views/NewCoinRecordsView.vue:69-87`）。
- 静态搜索未找到通用 TTL 查询缓存；现有 in-flight 模式只用于 Turnstile/PWA 初始化、收藏状态或轮询单飞，不覆盖 API 目录请求（`mobile/src/core/turnstile.ts:19-26`、`mobile/src/pwa/index.ts:36`、`mobile/src/core/marketFavoritesState.ts:20`）。

#### 推荐白名单与 TTL 起点

TTL 从**成功响应完成时**计算；下面是起始值，需要后端变更频率/Cache-Control 佐证后再定稿。

| 等级 | API / 证据 | 建议 TTL | 缓存键/失效注意 |
|---|---|---:|---|
| A 稳定公共配置 | `/countries`（`mobile/src/api/auth.ts:149-156`） | 30 min | `baseURL + locale/region(若后端区分)`；Register/KYC 共享 |
| A 稳定登录配置 | `/auth/login/config`、`/auth/register/config`（`mobile/src/api/auth.ts:39-59`） | 5 min | 配置开关可能运维变更；手动重试可 bypass |
| A 市场/兑换目录 | `/markets` pairs、`/convert/pairs`（`mobile/src/api/market.ts:46-56`、`mobile/src/api/swap.ts:35-37`） | 2–5 min | 只缓存 pair 元数据；不包含 ticker |
| A 产品目录 | `/margin/products`、`/seconds-contracts/products`、`/earn/products`、`/loan/products`（`mobile/src/api/trading.ts:233-264`、`mobile/src/api/seconds.ts:25-47`、`mobile/src/api/earn.ts:34-49`、`mobile/src/api/loan.ts:37-51`） | 1–5 min | key 包含规范化 limit；下单/申购前仍以后端校验为准 |
| A 配置目录 | `/prediction/config`（`mobile/src/api/prediction.ts:53-60`） | 2–5 min | allowed assets 只用于目录展示 |
| B 变化较快目录 | `/prediction/markets`、`/new-coins`（`mobile/src/api/prediction.ts:62-84`、`mobile/src/api/newCoin.ts:78-84`） | 30–60 s | 生命周期/结算状态会变；不可使用长 TTL |
| B 钱包操作目录 | `/wallet/deposit-assets`、`/wallet/withdraw-assets`、`/wallet/deposit-networks?asset_symbol=`（`mobile/src/api/wallet.ts:228-279`） | 30–60 s | 仅复用选择页目录；生成地址、取提现 quote、提交前必须重新由后端验证；若响应因账号/地区而异，key 加 session generation |
| B 本地化内容 | `/news?limit=&locale=`（`mobile/src/api/news.ts:23-34`） | 30–60 s | key 必须含 locale 和 limit；详情可独立短 TTL |

#### 明确排除：不得纳入此 TTL 缓存

- **余额/收益/流水**：`fetchWalletAccounts`、`fetchTodayReturn`、`fetchReturnHistory`、WalletLedger、充值/提现记录。
- **订单/申购/持仓**：Spot/Convert/Seconds/Prediction/Loan orders，Earn subscriptions，NewCoin subscriptions/distributions/purchases/unlocks，Margin positions/wallets/risk/settings。
- **报价和可执行条件**：convert/prediction/withdrawal quote、余额校验、费率 quote、可用额度；quote 已有 expiresAt 时也不能被通用 TTL 再延长。
- **实时行情**：tickers、klines、order book、recent trades、mark price 和 WebSocket snapshot。
- **用户/合规状态**：KYC status、profile、2FA/bindings、referrals、support conversation/messages。

这些数据即使 GET 也具有账号隔离、强一致或高时效语义。in-flight 去重只应在**同一身份、同一参数、同一 generation**内合并真正同时发出的相同读；不应把已经完成的强一致结果跨页面 TTL 复用。

#### 缓存器行为契约

1. 内存级 query registry，key = `API base + path + 排序后的规范化 params + locale + 必要的 auth scope`；不要用 localStorage 持久化目录响应，避免跨账号/版本长期陈旧。
2. fresh 命中直接返回；同 key 请求进行中时共享同一 Promise；Promise 在 `finally` 从 in-flight map 移除；失败与取消结果不写 TTL cache。
3. 缓存值只读或返回浅拷贝，避免某个 View 修改共享数组污染其他页面。
4. mutation 成功、登录态/租户/地区/locale/API base 变化时清相关 scope；手动刷新提供 `force: true`。
5. 请求版本/generation 防护仍需保留，防止旧 Promise 在路由离开后写回新页面。
6. 优先尊重后端 ETag/Cache-Control；TTL 是客户端避免重复解析/闪烁的上限，不替代服务端一致性约束。

#### 建议测试

- Register 与 KYC 同时/连续进入：`/countries` 并发只发 1 次，30分钟内返回不重复；过期或 force 后恰好再发 1 次。
- Orders tab 与 Trade 来回切换：目录请求按 key 复用，但 positions/orders/wallets 每次按当前业务规则重取。
- 充值三页：deposit assets 在 TTL 内只取一次；create address 每次按流程执行；提现 quote 永不从目录 cache 返回。
- 两个相同请求并发失败：两方收到同一错误，cache 不留失败值，下一次会重新请求。
- 切 locale、退出/换账号、切 API host：旧 scope 不命中；不同 limit/asset symbol 不串键。

### C. KycView 国家选择改为可搜索并复用 Register 交互（P1）

#### 现状与复用证据

- KYC 当前把后端配置国家映射为 `{ value, label }`，但 UI 仍是原生 `<select>`（`mobile/src/views/KycView.vue:45-56`、`mobile/src/views/KycView.vue:254-260`）。国家较多时缺少搜索，移动端原生 picker 也无法统一搜索/无结果/焦点体验。
- Register 已有完整可复用模式：`countrySearch`、`filteredCountries`、打开时清空搜索、选择后关闭（`mobile/src/views/RegisterView.vue:40-65`、`mobile/src/views/RegisterView.vue:93-111`）；Teleport modal、search input、无结果态、选中态（`mobile/src/views/RegisterView.vue:297-358`）。
- `filterCountryOptions` 会 NFKD 规范化、去组合符、保留所有书写系统，并让每个 token 同时匹配 ISO code、后端英文名和本地化名（`mobile/src/core/countrySearch.ts:6-38`）。
- `useModalDialog` 已处理初始焦点、Tab 圈闭、Escape、body 滚动锁、关闭后焦点归还和卸载恢复（`mobile/src/core/modalDialog.ts:12-87`）。
- KYC 的可选项不总是 ISO code：后端 `allowedCountries`/document rules 可能提供名称，当前 `matchesCountry` 同时按 code/name 匹配，并且 `form.country` 必须保留配置原始 value 以命中 document rule（`mobile/src/views/KycView.vue:45-57`、`mobile/src/views/KycView.vue:73-83`）。这是复用 Register 时必须保留的差异。

#### 建议结构

1. 优先抽取/复用 Register 的 country picker 展示与交互，不复制一套新的 focus trap/搜索算法。KYC 传入适配后的 option：`value` 保留 KYC 配置原值，另带 `code/name/localizedLabel` 给过滤器。
2. KYC 增加 `countrySearch`、`countryPickerOpen`、dialog/trigger refs，直接用 `filterCountryOptions` 与 `useModalDialog(..., '[data-country-search]')`。
3. 触发器显示当前 label + 展开状态，使用 `aria-haspopup="dialog"`、`aria-expanded`、`aria-controls`；弹窗复用 Register 的 Teleport、44px 关闭按钮、搜索框、滚动列表、选中标记和无结果状态。
4. 选择时写回 option 的原始 `value`，不能一律改写为 ISO code，否则 `selectedRule` 的字符串比较可能失配。选择国家后继续让现有 `watch(documentTypes)` 修正不再支持的证件类型（`mobile/src/views/KycView.vue:205-210`）。

## 2026-08-31 实施与验证结论

- 功能性 spinner 已与装饰动效分离；reduced-motion 与 constrained 档保留低频 `steps(8)` 反馈。
- 性能档在 Vue 挂载前写入根节点；SignalField 限制为 30fps，并在隐藏、离开视口、低动态或受限档停止持续绘制。
- GSAP 改为按需动态加载，Lightweight Charts 移除完整 points 深度监听；PWA 主入口约 `396.34 kB / 144.64 kB gzip`，GSAP 独立 chunk 约 `70.83 kB / 27.95 kB gzip`。
- 稳定参考 GET 使用显式白名单内存 TTL 与 single-flight；浏览器实测注册页离开再返回后 countries/register config 在 TTL 内仍各请求一次，强一致资金与交易请求保持原链路。
- Assets 仅渲染当前主题 Hero，长数值使用 34px 至 20px 的确定性分档；浏览器在 320/390/448px 的极长数值场景均无横向溢出或省略号。
- KYC 国家面板支持 ISO、后端名称、本地化名称和去重音符搜索，且始终提交后端配置原始值；认证状态和国家目录改为独立结算，目录失败不遮蔽 KYC 状态。
- Ego Browser 验证了 `China -> 中国/CN`、`cote ivoire -> 科特迪瓦/CI`、`United States -> 美国/US`，以及焦点归还、Escape、滚动锁、明暗主题和 320/390/448px 零溢出。reduced-motion 与 constrained 下遮罩 blur 均为 `none`；并新增编译级测试防止 scoped `:global` 再次退化。
- 完整 Mobile 测试、PWA 构建和 Tauri 构建均通过；任务未改变金融 API 协议或强一致数据实时性。
5. 国际化优先复用 `auth.countrySearchLabel/countrySearchPlaceholder/countryNoResults/countryPickerClose`；标题若需 KYC 语义再补 KYC key，不硬编码。
6. 国家目录与 KYC 状态错误应分开处理：`fetchKycStatus()` 保持强一致且不进 TTL；`fetchCountries()` 可命中上一节公共目录 cache。国家目录失败时不应遮蔽已成功的 KYC 状态。

#### 建议测试

- 搜索 ISO（`CN`）、英文名、带重音本地化名、中文名、多 token；空查询恢复全部配置允许项。
- KYC 配置同时覆盖 code 和 name 形式；选择后 `form.country` 保持原始值，证件规则与 handheld 要求正确切换。
- 打开自动聚焦搜索；Tab/Shift+Tab 圈闭；Escape/遮罩/关闭按钮关闭并把焦点还给触发器；关闭再开搜索被清空。
- 软键盘、320px 宽、82dvh、safe-area、长国家名、无结果、reduced-motion、TalkBack/VoiceOver。
- Register 与 KYC 共用行为测试，避免未来一处修复另一处漂移；如果抽组件，分别测试 option adapter 与通用 dialog。
