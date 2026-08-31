# 继续审查项目代码与业务优化

## Goal

以 2026-08-24 项目级审计及其已完成的 P0 修复为基线，对当前仓库再次进行全域静态审查，识别仍然存在或近期新增的代码结构、业务闭环、数据一致性、实时链路、前后端契约、部署运维与测试治理问题，并形成带源码证据、优先级、依赖关系和验收方式的可执行优化清单。

## What I already know

- 用户明确要求继续审查代码，覆盖“代码”和“业务”，本任务的主要交付是审查报告与实施路线，而不是在审查过程中混入大范围生产代码重构。
- `docs/architecture/project-optimization-audit-2026-08-24.md` 已形成 12 项 P0、21 项 P1、8 组 P2；后续提交已经完成原报告中的 P0 修复，因此本轮必须逐项复核当前代码，禁止原样复制历史结论。
- 仓库同时包含 Rust/Axum 后端、React 管理后台、Vue PC、Vue/PWA/Tauri 手机端、MySQL migrations、MongoDB、Redis、RabbitMQ/outbox/inbox、workers、行情与 WebSocket、Docker/1Panel 和 GitHub Actions。
- 当前工作树包含手机端秒合约历史订单等未提交改动；本轮审查不得覆盖、回滚或混入这些已有改动。
- Trellis 中存在较多历史活动任务，任务状态治理本身也需要作为工程治理证据复核，但不得仅以任务数量推断业务缺陷。

## Assumptions

- 本轮采用“静态源码 + 仓库内测试/配置 + 历史提交与审计文档”的证据基线；生产数据库、云 WAF、RabbitMQ policy、真实多副本拓扑、告警平台和备份系统等仓库外状态标记为“待运行时补证”。
- 优先级口径保持严格：P0 仅用于可直接导致资金、权限、结算、价格时点或不可恢复数据正确性风险；P1 用于业务可用性、跨进程可靠性和显著维护风险；P2 用于体验、性能、一致性和长期治理。
- 对同一根因跨层重复出现的问题只保留一个规范 ID，并在该项内列出所有受影响层。
- 不建议一次性重写；结构优化应提供兼容 façade、expand-contract migration、灰度/回滚和行为保护测试方案。

## Requirements

### R1. 基线与历史项复核

- 核对 2026-08-24 审计中的 P0/P1/P2 与当前 HEAD/工作树，标明“已完成、部分完成、仍存在、证据失效、需运行时补证”。
- 重点验证 P0 修复是否存在旁路、迁移遗漏、客户端契约漂移或缺少发布门禁，而不是只确认文件中出现了新代码。

### R2. 代码结构审查

- 审查 Rust 模块边界、依赖方向、巨型文件/函数、共享状态、错误模型、事务边界、幂等、锁序、时间/金额类型与重复实现。
- 审查 admin、PC、mobile 的 API client、状态管理、实时恢复、组件/样式复用、类型合同、路由与可访问性边界。
- 审查 migrations、worker 生命周期、MQ/outbox/inbox、行情、WebSocket、Docker/CI、日志/指标/readiness、备份恢复和 Secret 管理。

### R3. 业务流程审查

- 按“请求 → 鉴权/权限 → 服务端权威输入 → 事务/锁 → 订单/钱包/流水 → outbox/MQ → worker → REST 读模型/WS 提示 → 前端状态”追踪核心业务。
- 至少覆盖：注册与钱包初始化、充提、现货、杠杆、秒合约、闪兑、借贷、理财、预测、新币、代理返佣、在线客服、后台配置和行情生成/恢复。
- 检查业务状态机、额度/精度/价格新鲜度、失败补偿、人工复核、对账、并发与重试语义。

### R4. 可执行输出

- 每项发现必须包含：唯一 ID、分类、优先级、源码/配置证据、可达影响、建议方案、兼容/迁移策略、验收方式、工作量与依赖。
- 输出跨业务保护矩阵、结构热点清单、30/60/90 天路线图、任务拆分建议和验证矩阵。
- 明确区分“静态证据已成立”和“需要生产/运行时补证”，不把推测写成事实。

### R5. 独立复核

- 由独立检查代理核对报告中的严重级别、重复项、证据路径、结论可达性与遗漏面。
- 对被复核否定或证据不足的条目降级、合并或删除。

## Acceptance Criteria

- [x] 新审查报告覆盖 backend、admin、PC、mobile、data/migrations、MQ/workers/realtime、CI/deploy/security/observability/test governance。
- [x] 历史 12 项 P0 和 21 项 P1 均有当前状态映射，不能只引用旧报告结论。
- [x] 所有新发现均可定位到当前仓库中的文件与符号，或明确标为运行时补证。
- [x] P0/P1/P2 口径统一，同一根因不跨章节重复计数。
- [x] 报告包含核心业务流矩阵、依赖顺序、30/60/90 天实施路线和可拆分 Trellis 任务。
- [x] 报告经独立检查代理复核，严重级别和证据路径无未处理异议。
- [x] `docs/superpowers/PROGRESS.md` 记录本轮交付、文件与验证结果。
- [x] 不修改或回滚当前工作树中与本审查无关的手机端未提交改动。

## Definition of Done

- 审查研究产物已持久化至本任务 `research/`。
- 项目级复审报告已写入 `docs/architecture/`。
- 对报告执行路径存在性、编号唯一性、历史项覆盖率和 Markdown 结构检查。
- 独立检查意见已合并；仍需运行时验证的事项被明确列出。
- 未经用户明确要求，不提交、不推送、不实施报告中的大范围生产改动。

## Out of Scope

- 本轮不直接完成报告中的全部 P0/P1/P2 优化。
- 不连接或修改生产数据库、生产 RabbitMQ、云 WAF、Secret、备份与告警平台。
- 不对当前未提交的手机端设计改动做清理、提交或回滚。
- 不以新框架或微服务重写替代增量审查和兼容迁移建议。

## Research References

- `docs/architecture/project-optimization-audit-2026-08-24.md`：上一轮全域静态审计基线。
- `research/baseline-and-p0-verification.md`：当前基线、CI 限制与历史 P0 状态。
- `research/historical-p1-p2-status.md`：历史 P1/P2 当前映射与去重策略。
- `research/backend-core-finance.md`、`research/backend-product-business.md`：后端核心金融流与产品业务证据。
- `research/data-async-realtime.md`：数据、异步、worker、行情、实时与灾备证据。
- `research/admin-pc-cross-layer.md`、`research/mobile-cross-layer.md`：三端跨层合同、恢复、结构与交付证据。
- `research/delivery-security-quality.md`：供应链、CI、容器、Secret、可观测性与治理证据。
- `docs/architecture/project-optimization-reaudit-2026-08-30.md`：经独立复核的最终报告。

## Technical Notes

- 审查结果以符号名为主、行号为辅，避免后续代码位移导致证据失效。
- 研究代理只写本任务 `research/*.md`，不得修改生产代码、spec、进度文件或现有任务。
- 报告优先提出可被自动测试、发布门禁、对账或运行时指标验证的整改方案。
