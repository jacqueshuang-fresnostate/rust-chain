# 手机端动画可靠性、低端设备性能与页面请求优化

## Goal

让手机端在低性能 Android 设备、系统开启“减少动态效果”的设备和网络较慢的设备上仍然保持可用、可理解且流畅：功能性加载指示器持续反馈，装饰动画按设备能力降级，首屏减少脚本解析与持续绘制开销；同时修复资产总额长数字省略、KYC 国家不可搜索，以及页面往返时稳定参考数据被重复请求的问题。

## What I Already Know

- 多个页面把功能性 `.spin` 加载图标在 `prefers-reduced-motion: reduce` 下直接设为 `animation: none`，会让真实加载过程看起来“卡住不动”。
- `SignalField.vue` 在首页与行情页面持续以 `requestAnimationFrame` 绘制全屏 Canvas；当前每帧包含粒子、波线和网格，低端设备主线程与 GPU 压力较大。
- `LaunchIntro.vue` 静态导入 GSAP，导致只在首次进入展示的动效库进入主入口包。
- `LightweightMarketChart.vue` 对完整 K 线数组使用深度 watcher，实时更新时会产生不必要的深度遍历。
- 现有 `market` store 已有 20 秒快照新鲜度控制，但国家、注册配置、产品目录等稳定 GET 数据缺少统一的内存 TTL 与并发请求复用。
- 余额、订单、持仓、报价、KYC 状态等强一致或用户私有数据不适合长时间缓存，任何请求优化都不能让这些数据陈旧。
- `AssetsView.vue` 的会员资产总额固定为 34px 且强制 `text-overflow: ellipsis`，长数字会显示为 `...`。
- `RegisterView.vue` 已有可访问、可搜索的国家弹窗与 `filterCountryOptions`，KYC 页面目前仍使用原生 `select`。

## Requirements

### 功能性加载反馈

- 建立单一的功能性加载动画契约，统一 `.spin`、Turnstile 等真实加载状态使用的旋转反馈。
- 系统开启减少动态效果时，装饰动效停止，但功能性加载指示器必须保留低频、分步旋转，不能静止后伪装成卡死。
- 旋转只使用 compositor 友好的 `transform`，避免布局和绘制抖动。

### 低端设备运行策略

- 提供可测试的设备性能档位检测；综合 `saveData`、`deviceMemory`、`hardwareConcurrency`，在 API 缺失时安全回退到标准档。
- 在应用挂载前把性能档位写入 `<html>`，供 Vue、Canvas 与 CSS 使用。
- 低性能档关闭非必要的无限动画、重度毛玻璃与装饰滤镜，同时保持布局、按钮、弹窗、交易反馈和加载反馈完整。
- `SignalField` 在低性能档使用静态背景；标准档降低装饰 Canvas 帧率，并在页面不可见或 Canvas 离开视口时暂停。
- 首屏动画仅在需要展示且设备允许时动态加载 GSAP；减少动态效果或低性能档直接跳过，不阻塞应用进入。
- K 线组件避免对完整 OHLC 数组做深度遍历，保持现有增量更新、视口和 TradingView attribution 行为。

### 请求缓存与去重

- 增加小型、可测试的内存请求缓存，支持 TTL、新鲜命中、过期重取、并发 in-flight 复用、失败不落缓存和显式失效。
- 仅对跨页面复用且变化较慢的公共参考数据启用缓存，例如国家列表、登录/注册公开配置和只读产品/资产目录；TTL 在调用点显式定义。
- 余额、订单、持仓、成交、K 线、盘口、实时报价、一次性 quote、KYC 审核状态以及所有写操作保持网络实时，不进入 TTL 缓存。
- 页面手动刷新或业务写操作后的权威刷新仍可绕过/失效对应缓存。

### 资产数字与 KYC

- `assets-member-summary` 的总资产数字按可用宽度自适应缩小，完整显示，不使用省略号；320px 到 448px 不产生横向溢出。
- 今日收益区域也应在长数字时保持可读，不遮挡总资产或单位。
- KYC 国家选择复用注册页搜索逻辑，以弹窗/底部面板提供搜索、选中、无结果、键盘焦点约束和 Escape/遮罩关闭。
- 国家搜索匹配 ISO 代码、后端名称与本地化名称；原有后端配置限制的国家集合仍是唯一可选范围。

## Acceptance Criteria

- [x] 在 `prefers-reduced-motion: reduce` 下，真实 `.spin` 加载状态仍有低频分步旋转，装饰动画停止。
- [x] 设备性能档位检测有单元测试，应用挂载前设置 `data-performance-tier`。
- [x] `SignalField` 标准档限帧并支持 visibility/intersection 暂停，低性能档不启动持续 rAF。
- [x] GSAP 从主入口静态依赖中移出，只在允许播放首屏动画时动态加载。
- [x] Lightweight Charts 更新 watcher 不再深度遍历完整 points 数组。
- [x] 内存缓存测试覆盖 TTL、并发去重、错误、失效与按调用点隔离；只应用于稳定参考数据。
- [x] 页面返回后稳定参考数据在 TTL 内不再次发起同一请求，资金和交易数据仍实时请求。
- [x] 资产会员摘要的长数字在 320/390/448px 均完整可见且不出现省略号。
- [x] KYC 国家可以按代码、名称和本地化名称搜索，选择后正确提交原有 `country` 值。
- [x] 手机端 type-check、相关 Node tests、完整 mobile tests 与 PWA build 通过。
- [x] 在浏览器低端设备模拟与 reduced-motion 模拟下完成交互和性能回归验证。

## Definition of Done

- 只修改 `mobile/` 内与动画性能、请求生命周期、资产摘要及 KYC 国家选择直接相关的生产代码与测试。
- 不改变金融数据的实时性、API 协议、Pencil 页面业务结构或服务端接口。
- 更新移动端规格与 `docs/superpowers/PROGRESS.md`。
- 保留工作区中所有既有未提交改动，不回滚、不覆盖其他任务文件。

## Technical Approach

1. 新建纯函数设备性能策略，启动时写入 HTML dataset。
2. 在全局样式集中定义功能性 spinner 和低性能档覆盖，清理会把真实 spinner 停掉的局部规则。
3. 为 `SignalField` 加入 30fps 时间门、IntersectionObserver/visibility 生命周期和 constrained 静态模式。
4. 将 GSAP 改为条件动态 import，并为异步卸载竞态加保护。
5. 将 K 线 watcher 改为浅层数据边界。
6. 新建通用内存 TTL/in-flight 请求缓存，仅在稳定公共/目录 API 适配器中显式接入。
7. 资产总额使用 CSS `clamp()` 与容器查询/字符长度分级的稳定自适应方式，移除省略号。
8. KYC 国家选择复用现有 country search 与 modal dialog 基础能力，并补齐中英文文案和契约测试。

## Decision (ADR-lite)

**Context**: 卡顿来自持续装饰渲染、首屏脚本体积、深度响应式遍历和重复请求；加载“卡死”则主要是辅助功能媒体查询误伤了业务反馈。

**Decision**: 将“功能性运动”和“装饰性运动”分开治理；运行时使用保守的两档性能策略；请求缓存采用显式白名单而不是 Axios 全局 GET 缓存。

**Consequences**: 低端设备会减少视觉装饰但保留完整功能与状态反馈；稳定目录请求显著减少，金融实时数据不受缓存影响；新增策略均可通过纯函数测试防止回退。

## Out of Scope

- 不缓存 Service Worker 中的 API、金融响应或 WebSocket 数据。
- 不调整后端接口、数据库和行情推送协议。
- 不重做现有 Pencil 页面整体布局。
- 不以隐藏真实加载状态、缩短超时或伪造完成状态来掩盖网络慢。

## Technical Notes

- 重点文件预计包括 `mobile/src/main.ts`、`App.vue`、`LaunchIntro.vue`、`SignalField.vue`、`LightweightMarketChart.vue`、全局样式、`AssetsView.vue`、`KycView.vue`、公共 API 适配器与 `mobile/tests/`。
- 性能基线已在 390×844、4× CPU throttling、受限网络环境记录；后续对比时优先关注应用入口包、long task、Canvas 活跃状态和功能性 spinner 的 computed style。
