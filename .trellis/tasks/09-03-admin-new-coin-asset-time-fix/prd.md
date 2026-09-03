# 修复新币项目资产与时间配置

## Goal

修复后台创建新币项目时缺少后端必填 `quote_asset_id` 导致的 JSON 反序列化失败；明确并锁定“充提开关不影响新币项目资产资格”的业务合同；将后台新币创建与生命周期动作中的上市时间、固定解禁时间改为日期时间选择框，并在提交时转换为后端要求的 Unix 毫秒。

## What I already know

- `CreateNewCoinProjectRequest.quote_asset_id` 是非可选 `u64`，当前 Admin 创建表单没有该字段和 payload 映射，因此请求在进入业务层前就反序列化失败。
- 后端新币资产校验只要求资产 `status = active`，不会读取 `deposit_enabled` 或 `withdraw_enabled`；后台资产列表也未按两个充提开关过滤。
- 当前创建表单及“新币生命周期动作”页仍要求管理员输入裸 Unix 毫秒，项目已有 `AdminTextInput type="datetime-local"` 和本地日期时间转 Unix 毫秒的成熟模式。
- 项目资产与计价资产必须不同；启用解禁费时，解禁费资产必须与计价资产一致。现有后端规则保持不变。

## Assumptions

- “关闭提现充值”指资产仍为 `active`，但 `deposit_enabled = false` 和/或 `withdraw_enabled = false`；本任务不会允许 `status = disabled` 的资产参与新币发行。
- 日期时间选择框按浏览器本地时区输入，提交为绝对 Unix 毫秒，后端继续按 UTC `DateTime` 存储。

## Requirements

1. 新币创建表单新增“计价资产”下拉，复用 active 资产数据源，并排除当前项目资产。
2. 项目资产变化后如果与已选计价资产冲突，立即清空计价资产；缺少计价资产或两者相同不得提交。
3. `POST /admin/api/v1/new-coins` 必须发送正整数 `quote_asset_id`。
4. active 资产即使充值和提现开关均关闭，也必须出现在项目资产/计价资产候选中并可创建新币项目。
5. 创建表单中的“上市时间”“固定解禁时间”使用 `datetime-local`；按解禁类型维持现有条件显示。
6. 生命周期流转及解禁规则中的“上市时间”“固定解禁时间”使用 `datetime-local`，不再向管理员暴露“时间戳”输入文案。
7. 必填日期时间缺失或非法时给出中文校验错误；可选空值继续省略；有效值转换为 Unix 毫秒发送。
8. 不改变后端路由、数据库结构、active 状态要求、精度规则、解禁规则及审计事务。

## Acceptance Criteria

- [x] 创建新币项目选择项目资产和不同计价资产后，请求体含正确的 `quote_asset_id`，不再出现 missing field 反序列化错误。
- [x] 项目资产和计价资产不能选择同一资产，缺少计价资产时提交按钮不可用；切换项目资产形成冲突会立即清空计价资产。
- [x] 前端回归通过真实下拉选择与最终请求证明 `deposit_enabled=false` 且 `withdraw_enabled=false` 的 active 资产仍可使用，不再以 fixture 自断言替代 UI 证据。
- [x] 后端路由回归测试体覆盖 active 且关闭充提时创建成功，并覆盖 `status=disabled` 的计价资产仍被拒绝；本机已编译该测试，实库断言执行情况见下方说明。
- [x] 创建页、生命周期流转和解禁规则中的绝对时间字段均为 `datetime-local`。
- [x] 日期时间输入按本地时区正确转换为 Unix 毫秒，空的可选时间不进入请求体或序列化为 `undefined`。
- [x] Admin 聚焦测试、类型检查、Lint、全量测试、生产策略/覆盖率、生产构建和 bundle 门禁通过；Rust 格式、全目标检查及聚焦测试命令通过。
- [x] `git diff --check` 通过，全部无关脏改动保持原样。

> 本机未配置 `DATABASE_URL`。`cargo test --test admin_routes admin_new_coin_project_create_lists_events_and_audits -- --nocapture` 已完成编译且命令返回成功，但输出明确为 `skipping MySQL admin route test because DATABASE_URL is not set`；因此 active/关闭充提创建与 disabled 拒绝的 MySQL 实库断言本轮均未执行，不将其记作实库验证。

## Final Reviewer Closeout

- 将新币创建页及本任务相关动作页的复合状态写入统一改为函数式更新；计价资产回调也会依据最新项目资产拒绝竞态下的同资产值。
- 将本地日期时间转换改为按数字日历分量构造浏览器本地时间，再逐分量回读校验，避免依赖字符串解析宽容行为；测试以数字本地时间构造器独立计算期望毫秒。
- 前端回归实际选择关闭充提的 active 资产、切换项目资产制造冲突并观察计价下拉清空，最后检查精确 POST；后端同一聚焦用例增加 disabled 计价资产拒绝分支。
- 全仓搜索确认，新币创建测试不再断言缺少 `quote_asset_id`；剩余 `quote_asset_id` 省略断言属于现货交易对“安全字段编辑”PATCH 合同。裸时间戳文案只保留在“不应出现”的负向 DOM 断言中，生产 UI 已无该文案。

## Definition of Done

- 前端创建/动作页实现与 payload 合同修复。
- 前端和后端回归测试覆盖三个用户反馈。
- 相关 Admin/Backend 规范与项目进度记录同步。
- 不提交或推送，除非用户后续明确要求。

## Out of Scope

- 不修改 Mobile/PC 新币页面。
- 不允许 `status = disabled` 的资产用于新币项目。
- 不新增迁移或修改新币数据库字段。
- 不重做新币生命周期、申购、派发、购买或解禁业务流程。
- 不增加与本次表单修复无关的视觉设计。

## Technical Notes

- Admin 创建表单：`web/src/admin/resources/actions/newCoins.tsx`。
- Admin 生命周期动作：`web/src/admin/actions/NewCoinActions.tsx`。
- Admin 资产选项：`web/src/admin/resources/actions/shared.tsx`。
- 后端 DTO/校验/创建：`src/modules/admin/{presentation,service,application,infrastructure}/new_coin.rs`。
- 重点测试：`web/src/admin/resources/resourceConfigs.test.tsx`、`web/src/admin/actions/NewCoinActions.test.tsx`、`tests/admin_routes.rs`。
- 代码调查见 [`research/repo-findings.md`](research/repo-findings.md)。
- 最终复核记录见 [`review.md`](review.md)。
