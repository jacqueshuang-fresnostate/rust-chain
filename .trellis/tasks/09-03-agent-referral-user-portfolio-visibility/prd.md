# 统一代理邀请码并补齐下级用户资产交易视图

## Goal

修复代理门户生成的邀请码与代理本人在手机端“邀请好友”页面看到的邀请码不一致的问题，并让代理在服务端权威子树范围内查看下级用户的现货/杠杆资产、杠杆仓位和全部秒合约订单（明确包含进行中的订单）。

## What I already know

- 代理邀请码与用户邀请码都存放在 `invite_codes`，注册接口已经能消费两类邀请码。
- 当前差异来自“代理门户创建 agent-owned code，但代理关联的普通用户账号仍读取 user-owned code”，且两边生成格式也不同。
- 现有 `AgentAccessScope` 和 `agents.path` 已实现三级代理子树授权，父代理可见后代代理归属用户，子代理不可见父级、兄弟或其他根团队。
- 代理门户当前只有团队用户身份列表，没有资产、杠杆仓位或秒合约订单入口。
- 钱包分为 `wallet_accounts`（现货）与 `margin_wallet_accounts`（杠杆）；两者不能无标识地合并。
- 秒合约订单至少包含 `opened`、`settled`、`manual_review`，默认列表必须覆盖全部状态。

## Requirements

### 1. 邀请码统一

- 新创建的代理邀请码与用户邀请码共用六位大写字母/数字安全随机生成规则和全局唯一命名空间。
- 六位码发生唯一键冲突时须在有限次数内重新生成，不把数据库冲突直接暴露为随机创建失败。
- `/api/v1/referral/my-code` 对普通用户保持现有 user-owned code 语义；若当前用户是某个代理的关联用户，则返回该代理最新的 active agent-owned code。
- 活跃代理尚无可用 code 时，由服务端创建一枚 agent-owned code 并返回；并发首次读取不得产生错误或返回其他代理的码。
- 代理门户新建的 active code 立即成为该代理关联用户在手机端读取到的有效邀请码。
- 历史长格式代理码继续可用于注册和绑定，既有邀请码与邀请关系不迁移、不失效。

### 2. 下级用户金融数据 API

- 新增代理只读接口，按团队用户 ID 分别分页返回：
  - 现货与杠杆账户资产，明确 `account_type`，包括资产符号、Logo、available/frozen/locked。
  - 杠杆仓位，包括交易对、方向、保证金模式、杠杆、金额/价格/PnL、状态与时间。
  - 秒合约订单，包括交易对、方向、本金、周期、赔率、开仓/结算价、状态、输赢与时间。
- 列表响应必须包含与筛选条件一致的 `total`；页大小和 offset 使用项目统一边界。
- 杠杆仓位支持状态筛选；秒合约支持 `opened | settled | manual_review` 状态筛选，缺省表示全部。
- 目标用户必须属于当前登录代理的服务端子树。代理 ID 不允许由路径、查询或请求体指定。
- 父代理可读取后代代理归属用户；父级、兄弟、无归属和其他根代理用户必须不可见，并以不泄露记录存在性的结果返回。
- 所有金融行查询自身重复子树谓词；目标用户在检查与读取之间被改派时不得泄露原数据。
- 查询严格只读，不触发结算、强平、行情回填、钱包变更或流水写入。

### 3. 代理门户

- 团队用户表新增“资产与订单”操作，进入该用户的独立详情页。
- 详情页用“资产 / 杠杆仓位 / 秒合约订单”标签组织，显示当前用户邮箱或用户 ID。
- 每个标签使用服务端分页；杠杆仓位与秒合约订单提供中文状态筛选。
- 秒合约默认“全部”必须直接显示 `opened` 进行中订单，不能只展示历史终态。
- 金额继续使用共享 `AmountText`/Decimal 文本显示，状态使用中文可辨识标签，所有表格继续支持项目级列宽拖动与容器内横向滚动。
- 标签按需加载，切换标签不重复请求已经成功加载且查询条件未变化的数据；筛选或分页变化时只刷新对应标签。
- 邀请码页明确提示“最新启用邀请码会与代理关联用户的手机端邀请页同步”，最新 active code 按时间倒序显示。

### 4. Contracts and documentation

- 更新 Agent OpenAPI 路由、参数、响应 schema 和 tag 说明。
- 扩充代理层级规范，固化“代理关联用户的有效邀请码”和“金融子树只读”边界。
- 风险敏感 SQL、范围判断、并发邀请码生成和只读副作用边界使用详细中文注释。

## Acceptance Criteria

- [x] 代理门户创建的新邀请码为六位大写字母/数字，并可直接用于手机端注册/绑定。
- [x] 代理关联用户调用 `/referral/my-code` 得到门户最新 active code；普通用户行为不变；旧长码仍有效。
- [x] 根代理能读取直属及后代代理归属用户的两类钱包资产、杠杆仓位与秒合约全部状态订单。
- [x] 子代理无法读取父级、兄弟、其他根、未分配用户的数据，且不能通过传代理 ID 扩大范围。
- [x] 秒合约结果包含 `opened` 订单，状态筛选和分页 total 正确。
- [x] 代理团队用户页可进入详情页，三个标签、筛选、分页、空态/错误态和 Decimal 展示可用。
- [x] OpenAPI、后端路由测试、前端 API/页面/路由/布局测试同步完成。
- [x] Rust fmt/check/clippy/目标测试与 Admin Web typecheck/lint/test/build/budget 通过；`git diff --check` 通过。

## Definition of Done

- 后端 domain/repository/application/infrastructure/presentation/routes、OpenAPI、Web agent portal 和测试完整闭环。
- 不依赖 Admin 全局接口，不放宽现有用户自服务或客服精确归属边界。
- 更新 `.trellis/spec/backend/agent-hierarchy.md` 与 `docs/superpowers/PROGRESS.md`。

## Decision (ADR-lite)

**Decision**: 以“代理关联用户读取代理最新 active code”为唯一有效展示规则；金融数据采用用户详情子路由和三个分页只读接口，而不是一次性返回整棵团队的所有资金明细。

**Why**: 前者消除 agent-owned/user-owned 双码混淆且保留普通用户邀请链；后者使授权对象明确、分页稳定、页面可按需加载，也避免代理门户增加三个缺少用户上下文的全局菜单。

**Consequences**: 代理每次新建 active code 会改变手机端显示的主邀请码，但旧码仍保持可用；金融详情有短暂并发快照差异，但重复 scope 谓词保证不会跨树泄露。

## Out of Scope

- 不允许代理修改下级用户资产、仓位或订单。
- 不新增代理代客下单、平仓、结算、充值或人工调账。
- 不改变代理返佣计算、客服精确归属例外或 Admin 全局权限。
- 不改 PC 客户端，也不重做代理门户整体视觉系统。

## Research References

- [`research/current-agent-referral-and-team-data.md`](research/current-agent-referral-and-team-data.md) — 当前双码根因、子树授权与缺失金融视图审计。

## Technical Notes

- 适用规范：`.trellis/spec/backend/{directory-structure,quality-guidelines,agent-hierarchy,wallet-amount-precision,seconds-contracts}.md`、`.trellis/spec/admin/{ui-system,resource-response-contract}.md` 和 `.trellis/spec/guides/{cross-layer-thinking-guide,code-reuse-thinking-guide}.md`。
- 主要实现区域：`src/modules/{agent,user}`、`src/openapi/agent_portal.rs`、`tests/{agent_routes,user_routes,openapi_routes}.rs`、`web/src/{api/agent,agent,layouts/AgentLayout}`。
- 所有 Decimal 字段保持字符串/`BigDecimal`，不得在后端或 API 适配层转为浮点数。

## Independent Check Fixes

- 邀请码启停改为按 `invite code -> agent` 锁序执行的单事务，重复状态请求幂等；停用最新码后，关联用户立即回退到下一枚最新 active 码。
- 六位安全随机生成改用拒绝采样消除 256 对 36 取模偏差；继续由全局唯一键与有界重试处理碰撞。
- 金融响应在后端与 Web 边界共同拒绝超出 `0..=18` 的资产精度，并严格校验必填 nullable 字段、Decimal 文本、枚举、安全整数和 Unix 毫秒字段；OpenAPI 同步 required/nullable 与数值边界。
- 资产表补显真实 Logo；秒合约 `opened` 在该业务上下文显示为“进行中”，不再误用杠杆仓位的“持仓中”。
- MySQL 集成测试扩充三级后代、自身、父级、兄弟、其他根、未归属、不存在、伪造 `agent_id`、双钱包只读快照、脏精度、最新 active 回退与同值状态幂等边界。
