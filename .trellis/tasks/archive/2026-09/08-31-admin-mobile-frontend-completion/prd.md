# 完善 Admin 与移动端全部前端问题

## Goal

以 2026-08-31 前端增量复审为基线，集中修复管理后台 `web/` 与手机端 `mobile/` 中已经确认的正确性、资金精度、会话安全、权限、请求/实时生命周期、PWA/Tauri、无障碍、国际化、性能和工程治理问题；保持既有 Pencil 视觉与业务流程，不修改 PC 端 `pc/`。

## What I already know

- 用户明确要求本轮只完善 Admin 和移动端，PC 不在实施范围内。
- 审计证据已落盘：Admin 共 1 个 P0、7 个 P1、4 个 P2；Mobile 共 7 个 P1、5 个 P2，并有若干跨条目问题（错误码本地化、触控目标、PWA 安装提示与首屏资源）。
- 当前工作树包含多个已完成但尚未提交的后端、Admin、Mobile 任务改动；本任务必须增量修改，禁止覆盖、回滚或清理这些成果。
- 资金、订单、权限和会话问题优先于机械拆文件或纯视觉优化；“测试通过”必须包含行为测试而不是只读取源码字符串。
- 后端接口已经存在现货批量撤单；其他需要服务端支持的安全闭环，仅在保持现有客户端兼容时做最小后端调整。

## Requirements

### R1. Admin 资金命令与十进制正确性

- 对管理员充值等高价值命令建立可恢复、会话隔离的幂等意图；`25.50` 与 `25.5` 必须生成同一规范化意图。
- 网络超时、响应丢失、组件重挂载和页面刷新不得自动轮换幂等键；成功、服务端明确失败或管理员明确改变业务意图后才轮换。
- Admin 金额展示、输入校验、边界比较和请求载荷全程使用 Decimal text，正确处理 `1e-18`、超过 `2^53` 的整数和资产精度。
- API client 支持 AbortSignal、超时分类和稳定错误模型；错误响应不得被宽松转换为伪空列表。

### R2. Admin 会话、权限与认证重放

- 管理员 access 查询按 subject/session generation 隔离；登出必须取消旧请求、清理私有缓存/实时连接，并支持跨标签同步。
- refresh 使用 session epoch/CAS，旧会话响应不得复活已登出的身份；受保护页面的 redirect 仅接受站内 path/search/hash。
- 全局 mutation 默认不重试；登录、2FA 与一次性 Turnstile token 单击只发送一次请求。
- 每个通用资源 action、批量操作及独立页面操作声明精确权限；只读角色不显示、不可触发 write/review/operate/settle 操作。
- 管理员令牌从长期 `localStorage` 迁移到更短生命周期的单一 session owner；若后端已有撤销/刷新能力则调用权威注销，同时保留现有部署兼容。

### R3. Admin 数据合同、实时性与交互质量

- Admin DTO 在 API 边界进行窄类型/结构校验；错误 key、缺失必填字段和非法 Decimal 抛出可识别 ContractError。
- 行级目录/选项改为共享、可取消、按需加载的 React Query；不得为表格每一行重复发起同一目录请求。
- 行情连接由共享连接管理器持有，具备 generation、引用计数、重连退避、入站沉默 watchdog、fresh/stale/offline 与最后更新时间。
- 修复秒合约 Tabs 的 ARIA panel 关系、重复字段的记录级 accessible name、浏览器中文标题和 API 环境变量命名漂移。
- 在不改变现有视觉系统的前提下按业务域拆分过大的配置/懒加载边界，并为 Admin bundle 建立可执行预算。

### R4. Mobile 会话、行情与请求生命周期

- `market.refresh()` 共享同一个 in-flight Promise；任一路由冷启动完成后都幂等确保 ticker live lease，且暴露 live/connecting/stale/offline/lastFrameAt。
- Mobile refresh 使用 session epoch/CAS；logout 后旧 refresh 不得写回 token、重放旧请求或恢复私有 WS；跨标签/容器状态同步。
- Orders 等可切换页面使用 generation/AbortSignal，旧 tab/旧参数响应不能覆盖当前 loading/error/data。
- Mobile 用户可见 API 错误优先按稳定 `code` 本地化；未知 5xx 使用安全通用文案，原始 message 仅进入诊断信息。
- 非必要参考数据继续使用现有 TTL/single-flight 缓存，并在身份变化时失效私有缓存。

### R5. Mobile 资金、订单与权威 DTO

- 现货、杠杆、秒合约、闪兑、借贷、理财、预测、新币和钱包 mutation 的金额/价格/数量使用 branded Decimal text，不通过 `number`/`Number` 传输或比较。
- 资金账单和收益展示按资产精度格式化，合法微额不得显示为 0，超大余额不得发生舍入改变。
- 现货“全部撤单”调用后端批量端点并消费 `orders[]/failures[]`，向用户准确呈现成功数、失败数和剩余风险订单。
- KYC、邀请、快捷充值、闪兑、理财等业务 enum 使用 typed presentation adapter；未知值保留“未知/原值”语义，不伪装为 pending。

### R6. Mobile 私有实时、PWA/Tauri 与无障碍

- 建立 session-scoped 私有 WS 管理器/topic lease，至少覆盖交易刷新与 `support.refresh`，包含 token generation、重连退避、heartbeat/watchdog 和 REST 权威对账。
- 发布门禁运行 Mobile type-check、全量行为测试、PWA 构建、Tauri 构建及关键制品断言；Tauri 设置满足当前 API/WS/Turnstile/图片资源的最小 CSP。
- PWA 更新流程具有 timeout/error/retry 恢复，busy 不会永久卡住；安装提示具备首会话延迟、频控和价值动作触发策略。
- 根壳只保留一个主 landmark，路由切换更新 title、焦点和 announcement；Seconds listbox 支持方向键与 roving tabindex。
- KYC Blob URL 在替换、提交成功和离页时 revoke；关键按钮/筛选/图标触控目标在 320–448px 视口达到至少 44×44px 或等效命中区域。

### R7. Mobile 性能与结构治理

- 将首屏 1.7MB 位图优化为响应式现代格式或移出预缓存，避免无条件进入首次 PWA 安装资源集合。
- 对 Trade/Seconds/Assets 的新增逻辑按 session、market、dialog、financial intent 或 domain adapter 抽出可测试 composable/service；禁止仅为降低行数做机械切割。
- 建立 raw/gzip 资源预算和 CSS/JS 关键 chunk 断言；低性能设备上的加载反馈持续变化，装饰动画可降级且不阻塞交互。
- 增加真正挂载组件/模拟 deferred Promise、fake socket、service worker 和 reload 的行为测试；测试 TypeScript 纳入类型检查。

## Acceptance Criteria

- [x] Admin 幂等测试覆盖 `25.50 == 25.5`、commit-before-timeout、response-drop、组件重挂载和 reload，服务端效果最多一次。
- [x] Admin 权限矩阵覆盖 read/review/operate/write/settle；无权限 action 不渲染且直接调用仍被后端拒绝。
- [x] Admin 登录/2FA 单次点击仅发出一次请求，Turnstile 失败后可重新获取 token；会话切换不复用旧 access query。
- [x] Admin Decimal 测试覆盖 `1e-18`、`9007199254740993`、不同资产 precision；列表合同错误显示明确错误态而非空列表。
- [x] Admin 行情共享连接在重连、旧 socket 延迟关闭、沉默超时和多行订阅场景下状态正确。
- [x] Mobile deferred A→B 冷启动、logout-vs-refresh、旧 tab 响应、私有 WS 沉默/重连测试全部通过。
- [x] Mobile 所有资金 mutation 的公开类型不再接受 `number`；微额和超大金额展示/比较保持十进制语义。
- [x] Mobile 批量撤单能展示全成功、全失败和部分成功三种权威结果。
- [x] Mobile PWA 更新失败可恢复，Tauri CSP 非空，release gate 同时构建 PWA/Tauri 并检查产物。
- [x] 320px、390px、448px 视口下关键路由无横向溢出；键盘可完成 Seconds 列表选择，路由后焦点/title/announcement 正确。
- [x] KYC Blob URL 无泄漏；未知业务状态不再被错误映射；关键触控目标满足 44px 命中区域。
- [x] Admin lint/typecheck/test/build、Mobile type-check/test/build:pwa/build:tauri、相关 Rust 检查（若改后端）全部通过。
- [x] 独立 `trellis-check` 复核未留下未处理 P0/P1；所有暂缓项必须有可执行原因和跟踪条目，不能用“范围太大”代替修复。
- [x] `docs/superpowers/PROGRESS.md` 按交付切片记录修改与验证。

## Definition of Done

- Admin 与 Mobile 审计清单逐项关闭，并在任务研究/检查记录中给出代码和测试证据。
- 行为测试先覆盖风险路径，生产代码随后修复；源码字符串断言只作为补充合同，不作为唯一证据。
- 所有改动通过相应 release gate、生产构建和浏览器回归。
- 新形成的会话、Decimal、实时连接、错误合同和无障碍约束更新到 `.trellis/spec/`。
- 不覆盖当前工作树已有成果；不修改 `pc/**`。

## Technical Approach

1. 先关闭 Admin P0 与两端会话/Decimal/请求竞态，建立可复用 primitives。
2. 再替换页面调用与权限/DTO/批量操作，保证服务端权威结果直接进入 UI。
3. 再收口实时连接、PWA/Tauri、无障碍、资源预算和结构拆分。
4. 每个切片执行定向测试与进度记录；最后执行两端全量 gate、Ego Browser 320/390/448px 与 Admin 桌面回归。

## Decision (ADR-lite)

**Context**：审计项跨越正确性、安全、体验和工程治理，且工作树已有大量并行成果。

**Decision**：采用“风险优先、共享 primitive、兼容迁移、分切片验证”的增量修复；Admin 与 Mobile 分开写入，公共后端仅做必要兼容扩展；PC 明确冻结。

**Consequences**：任务会产生多个可独立验证的切片，并可能需要多轮 implement/check；不以一次性重写或纯 CSS 扫描替代行为修复。

## Out of Scope

- 不修改、重构、测试修复或打包 `pc/**`；审计中的 PC P0/P1/P2 保留给独立任务。
- 不连接或修改生产数据库、生产 RabbitMQ、Cloudflare、1Panel 或真实用户资产。
- 不重新设计已经通过 Pencil 1:1 验收的页面；仅为正确性、无障碍、性能和一致性做必要调整。
- 不引入与现有 Vue/React/Rust 技术栈无关的大型框架重写。

## Research References

- `../08-30-project-code-business-optimization-reaudit/research/frontend-admin-delta-2026-08-31.md`：Admin FAD-P0/P1/P2 完整证据与文件索引。
- `../08-30-project-code-business-optimization-reaudit/research/frontend-mobile-delta-2026-08-31.md`：Mobile FMD-P1/P2 完整证据与文件索引。
- `docs/architecture/frontend-optimization-audit-2026-08-31.md`：跨端去重后的优先级、执行顺序和发布结论。
- `research/remediation-scope.md`：本任务范围、切片与禁止冲突区域。

## Technical Notes

- 涉及资金的 transport/domain 类型优先为 string-backed Decimal；展示层按资产精度做确定性截断/格式化。
- refresh、请求、WebSocket、缓存都必须绑定 session generation，旧 generation 只能丢弃结果，不能写回当前状态。
- 每个 action 的前端 permission 仅负责隐藏/禁用，后端仍保持 fail-closed。
- 生产代码改动由 `trellis-implement` 代理完成，独立 `trellis-check` 复核；主会话负责集成、冲突处理、完整门禁和交付记录。
