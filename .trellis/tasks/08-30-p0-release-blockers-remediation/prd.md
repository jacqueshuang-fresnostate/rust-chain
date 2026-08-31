# P0 发布阻断项修复

## Goal

关闭 2026-08-30 复审确认的三个严格 P0：清除 PC 构建期异常加载器并在任何依赖安装或构建前建立静态源码完整性门禁；为人工充值、现货下单和杠杆资金划转建立稳定客户端幂等协议；让合成行情进入受代际保护的事件时间价格历史，并使秒合约在缺少合法结算证据时确定性地 fail closed、进入可审计终态。

## What I already know

- 当前可信基线为 `main@fac1def`，工作树中已有 Mobile/Pencil 等未提交改动，本任务不得覆盖、回滚或夹带这些改动。
- `pc/postcss.config.js` 的第 8 行包含 tracked 顶层混淆 IIFE；当前根 Docker workflow 不执行 PC Vite build，但历史本地、预览、原生或其他 CI 执行情况未知。
- 人工充值无请求键，现货下单与杠杆划转允许缺键；服务端临时 UUID 不能跨网络重试提供幂等保证。
- 外部行情会写入 `market_price_ticks`，合成行情当前只更新 Redis/触发器/WebSocket；秒合约结算只读取 MySQL 事件时间快照。
- 历史缺失价格不得通过当前 Redis 最新价或伪造 K 线补写。

## Assumptions

- P0 修复采用 fail-closed：旧客户端缺少幂等键时返回明确 4xx，不再自动生成服务端键后动账。
- 幂等身份按“主体 + 操作 + 客户端键”作用域隔离；同键同参返回首次结果，同键异参返回 409。
- 合成 ticker 只有在当前 strategy lease/generation/version 合法且事件时间快照可持久化后，才能驱动金融触发器和客户端实时广播。
- 代码仓库可以完成静态处置、门禁、数据迁移、测试和运维 runbook；主机取证、凭据轮换、旧制品/cache 失效及生产数据对账由部署环境 owner 按 runbook 留存运行证据。

## Requirements

### P0-01：PC 构建供应链

1. 将 `pc/postcss.config.js` 恢复为最小、可审查、无顶层副作用的 PostCSS 配置。
2. 新增不加载 Node/Vite/PostCSS 配置的静态源码完整性扫描器，至少阻断：
   - 构建配置中的长行/高熵混淆可执行代码；
   - 网络访问与动态求值/子进程组合；
   - 本次已知 IOC hash 或等价恶意片段重新出现。
3. 扫描器必须在 GitHub Actions checkout 后、依赖安装和任何构建前运行，同时保留在本地 `p0-release-gate` 的第一道检查。
4. 增加门禁回归测试和事件处置 runbook，明确冻结、取证、轮换、cache/artifact 失效、可信 clean build 与回滚步骤。
5. 清理后才允许执行 PC 构建验证；验证过程不得访问未知外部地址或执行历史 payload。

### P0-02：高价值资金命令强幂等

1. 人工充值、现货下单、杠杆划转的 API 请求必须携带稳定客户端幂等键，并由 Admin/PC/Mobile 调用方生成和复用。
2. 后端以规范化请求指纹验证重放；同键同参只能产生一次业务记录、一次余额变化和一组流水，并返回首次成功结果。
3. 同主体、同操作、同键但不同参数必须返回 HTTP 409；不同主体可以安全复用相同字符串键。
4. 命令收据与业务事务必须具备原子/可恢复语义；提交后断连、并发请求和进程重试不得重复动账。
5. migration 对存量记录保持只读兼容，不伪造历史请求身份；删除所有“缺键时服务端生成 UUID 并继续动账”的分支。
6. 为三条命令补齐单元/数据库集成/客户端合同测试，覆盖缺键、同参重放、异参冲突、20 并发及提交后响应丢失语义。

### P0-03：合成行情与秒合约结算闭环

1. 合成 ticker 在通过 strategy lease/generation/version fence 后，以 append-only 方式写入 `market_price_ticks`，保存 pair、price、事件时间、source、generation/version 等可审计身份。
2. 重复 ticker 写入必须幂等；旧 generation、过期 lease、倒退/非法事件时间不得污染结算历史。
3. 秒合约产品保存与开仓必须验证交易对具备事件时间归档能力；无能力时在扣款前 fail closed。
4. 合成行情的金融触发器和 WebSocket 广播不得早于权威结算快照持久化成功。
5. 无合法事件时间快照的订单不得使用当前价猜测结算；达到最大待结算年龄后进入确定性的 `manual_review` 或退款终态，并以幂等事务记录钱包流水和审计证据。
6. 覆盖 strategy 开仓→合成 tick 归档→事件窗口结算，以及 Redis/MySQL/Mongo 任一步失败、重复 tick、旧 generation、worker 重启、结算与超时处置竞争。

## Acceptance Criteria

- [x] `pc/postcss.config.js` 只保留最小配置，已知 IOC hash 与同类静态特征在 tracked 源码中为零。
- [x] 源码完整性门禁在依赖安装前运行；恶意 fixture 会使 workflow/local gate 非零退出，正常配置通过。
- [x] Admin recharge、Spot create、Margin transfer 缺幂等键均在动账前拒绝。
- [x] 三条命令同键同参并发/重放均只动账一次，同键异参返回 409，不同主体互不冲突。
- [x] 合成 ticker 只有在 generation/version fence 与 MySQL 归档成功后才发布并参与结算。
- [x] strategy/internal 秒合约可按事件时间快照完成结算；无归档能力的 pair 不能开仓。
- [x] 超龄且无合法快照的订单具有可审计、幂等、资金守恒的人工复核或退款状态，不会永久 `opened`。
- [x] 最贴近改动的 Rust、Web/Admin、PC、Mobile 测试通过；`cargo fmt -- --check`、Clippy `-D warnings` 和发布门禁通过。
- [x] `docs/superpowers/PROGRESS.md`、相关 Trellis spec、P0 复审报告状态和运维 runbook 已更新，并明确区分代码证据与待补生产证据。

## Definition of Done

- migration、后端、客户端、CI、测试与文档形成闭环，新增 schema 可 fresh install 与 upgrade。
- 所有资金路径在失败、重试、并发和异参重放下保持资金守恒。
- 独立 `trellis-check` 审查完成，发现的问题已修复或被明确记录为部署环境阻塞项。
- 不覆盖当前工作树中与本任务无关的 Mobile/Pencil 改动。

## Out of Scope

- 不处理复审报告中的 P1/P2 项，除非它们是本次 P0 修复不可分割的前置条件。
- 不连接或修改生产数据库、Cloudflare、GitHub Secrets、GHCR、开发机 EDR 或远程 runner。
- 不伪造“凭据已轮换、主机已取证、历史重复账已清理”等生产运行证据；只提供可执行 runbook、查询与门禁。
- 不重写与 P0 无关的现有 Mobile/Pencil UI 工作。

## Technical Notes

- 审计来源：`docs/architecture/project-optimization-reaudit-2026-08-30.md`。
- 历史证据：`.trellis/tasks/08-30-project-code-business-optimization-reaudit/research/`。
- 先完成 P0-01 静态清理与门禁，再运行任何 PC Vite/PostCSS 构建。
- 三条 P0 可按互斥写入范围并行实施，统一 migration 编号和共享文档由主会话协调。
