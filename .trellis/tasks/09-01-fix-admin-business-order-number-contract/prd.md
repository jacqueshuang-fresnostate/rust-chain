# 修复 Admin 业务订单号列表兼容

## Goal

修复 Admin 秒合约订单与理财申购列表因前端把展示层合成的 `order_no` 误判为后端必填字段而加载失败的问题，并一次性覆盖所有复用相同订单号列工厂的后台业务列表。

## What I Already Know

- `AdminResourcePage` 当前把每一个表格列键都放入 `rowContract.requiredFields`，并由 `listAdminResource` 对接口每一行执行严格字段校验。
- `orderNoColumn()` 使用 `order_no` 作为表格列键，但渲染函数 `formatBusinessOrderNo()` 本来就支持在接口没有 `order_no` 时根据业务前缀、时间和记录 ID 生成稳定展示值。
- 秒合约 `SecondsContractOrderResponse` 与理财 `EarnSubscriptionResponse` 的既有后端 DTO 都没有 `order_no`；这不是数据库漏查字段，而是前端派生列与传输字段的边界建模错误。
- 相同列工厂还用于借贷、预测、现货、新币申购、新币购买和闪兑列表，因此只针对两个接口写例外会留下同类故障。
- 后端真实返回 `order_no` 的业务仍应优先显示该值；其他真实 API 列继续保持严格必填校验。

## Requirements

- 为 Admin 表格列显式区分 API 原始字段与前端派生字段，默认仍按 API 字段处理。
- `orderNoColumn()` 生成的列必须声明为派生列。
- 构建响应行契约时，派生列不得进入 `requiredFields`，也不得削弱其他真实列的必填与 Decimal 字符串校验。
- 所有复用 `orderNoColumn()` 的业务页面同时生效，包括秒合约订单和理财申购。
- 接口已返回非空 `order_no` 时继续优先显示后端值；未返回时继续使用现有稳定合成规则。
- 不新增数据库字段、不修改现有后端 DTO、不针对 endpoint 写硬编码白名单。

## Acceptance Criteria

- [x] `/admin/api/v1/seconds-contracts/orders` 的记录没有 `order_no` 时列表可以加载并显示合成订单号。
- [x] `/admin/api/v1/earn/subscriptions` 的记录没有 `order_no` 时列表可以加载并显示合成订单号。
- [x] 借贷、预测、现货、新币、闪兑等复用订单号列的页面不会因缺少展示层 `order_no` 而失败。
- [x] 普通 API 列缺失时仍由严格行契约报错，金额列仍执行 Decimal 字符串校验。
- [x] 后端提供 `order_no` 时显示该真实订单号，不被合成值覆盖。
- [x] Admin 类型检查、Lint、测试、生产策略、覆盖率、构建和预算检查通过。

## Definition of Done

- 完成列元数据、行契约构建和订单号列配置修复。
- 增加针对派生列边界、秒合约和理财配置的回归测试。
- 将派生列与 API 响应契约规则补充到 Admin 规范。
- 更新 `docs/superpowers/PROGRESS.md` 并完成 Trellis 检查与归档。

## Technical Approach

- 在 `AdminResourceColumn` 增加语义明确的来源元数据，例如 `source: 'api' | 'derived'`，未声明时保持 `api` 默认行为。
- 提取或集中行契约构建逻辑，只把非派生列加入 `requiredFields`，并保持金额列的 Decimal 校验逻辑。
- 由 `orderNoColumn()` 统一声明 `source: 'derived'`，避免逐个 endpoint 特判。
- 使用纯函数测试验证契约构建，再用资源配置测试验证所有订单号列都带有派生标记并保留既有渲染回退。

## Decision (ADR-lite)

**Context**：表格列既可能映射后端 DTO，也可能只是展示层计算值；当前代码把二者都当作 wire contract，导致严格校验与既有订单号回退逻辑互相冲突。

**Decision**：在列定义层显式标记派生来源，并只从 API 来源列推导响应行契约。

**Consequences**：严格校验继续 fail-closed，同时派生展示列可以安全复用；新增派生列时需要显式声明来源，避免再次误入传输契约。

## Out of Scope

- 不为各业务表新增持久化 `order_no` 字段。
- 不改变秒合约、理财或其他业务接口的查询、分页和排序语义。
- 不调整订单号合成格式。
- 不修改 Admin 页面视觉样式。

## Technical Notes

- 关键前端文件：`web/src/admin/resources/AdminResourcePage.tsx`、`web/src/admin/resources/resourceConfigs.tsx`、`web/src/api/adminResources.ts`、`web/src/shared/orderNo.ts`。
- 后端核对范围：秒合约和理财 presentation/application/infrastructure DTO 与查询。
- 相关规范：`.trellis/spec/admin/index.md`、`.trellis/spec/admin/ui-system.md`。
