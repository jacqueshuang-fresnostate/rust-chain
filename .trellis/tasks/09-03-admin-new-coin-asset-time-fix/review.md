# 后台新币资产与时间配置最终复核

## Findings（已修复）

- 文件：`web/src/admin/resources/actions/newCoins.tsx`、`web/src/admin/actions/NewCoinActions.tsx`
  - 问题：除资产冲突联动外，相关表单仍混用捕获旧 render 快照的对象展开写法；计价资产回调也未在提交状态更新时再次核对最新项目资产，存在批处理/竞态下恢复旧字段或写入同资产值的风险。
  - 修复：本任务涉及的项目、生命周期及解禁规则字段改用函数式 state updater；计价资产写入依据 `current.assetId` 二次拒绝冲突。
- 文件：`web/src/admin/newCoinDateTime.ts`、`web/src/admin/actions/NewCoinActions.test.tsx`
  - 问题：本地日期时间依赖 `new Date(string)` 解析，测试又用同一字符串解析计算期望值，未独立锁定“浏览器本地日历分量”语义。
  - 修复：按年/月/日/时/分/秒/毫秒数字分量构造本地 `Date` 并逐项回读，拒绝日期滚动及 DST 空洞；测试用数字分量构造独立期望值，并覆盖小数秒、空值及非法日期中文错误。
- 文件：`web/src/admin/resources/resourceConfigs.test.tsx`
  - 问题：关闭充提资产的新增断言有一段只验证测试 fixture 自身，且未真实验证“项目资产切换为当前计价资产后清空计价资产”。多个残留 Semi portal 的同名选项还会让全局文本选项选择产生歧义。
  - 修复：删除 fixture 自断言；通过真实下拉选择两项关闭充提的 active 资产、制造冲突并断言计价下拉恢复空值，再检查最终 POST。新增按当前控件 `aria-controls` 精确定位 option list 的任务内测试辅助方法。
- 文件：`tests/admin_routes.rs`
  - 问题：聚焦路由测试只覆盖 active 且关闭充提时成功，没有锁定 `status=disabled` 仍被拒绝。
  - 修复：同一新币创建用例先将计价资产临时设为 disabled 并断言 `VALIDATION_ERROR`，恢复 active 后再以两个充提开关均关闭的资产执行原成功、事件及审计断言；生产后端逻辑未改。
- 文件：`.trellis/spec/admin/ui-system.md`
  - 问题：缺少本次跨层请求与本地时间转换的可执行 Admin 合同。
  - 修复：新增包含 Scope、Signatures、Contracts、Validation Matrix、Good/Base/Bad、Tests Required、Wrong vs Correct 的七节合同。

## Findings（未修复）

- 本机没有 `DATABASE_URL`。Rust 聚焦测试完成编译并返回成功，但测试函数在进入 MySQL 分支前按既有逻辑跳过；active/关闭充提成功与 disabled 拒绝的实库断言本轮未执行。该项需要可用 MySQL 环境，不是代码缺陷。

## 合同复核

- 新币 POST 的 `quote_asset_id` 为必填正整数，且创建按钮在缺失、非整数、非正数或与项目资产相同时禁用。
- 项目资产与项目符号两条联动路径都会基于最新状态清除冲突计价资产；计价下拉同时排除当前项目资产。
- active 资产资格只依赖 `status`；充提开关不参与前端 active 查询或后端精度锁定。disabled 规则保持不变。
- 创建页、生命周期和解禁规则全部使用 `datetime-local`；创建 payload 只发送当前解禁类型对应字段，动作页空时间键由 JSON 省略。
- 全仓搜索确认：新币创建测试已无“请求不含 `quote_asset_id`”旧合同；现存同名省略断言属于现货交易对安全配置 PATCH。生产 UI 已无裸时间戳文案，旧文案仅在负向 DOM 回归断言中出现。
- 后端 active 状态、资产精度、解禁字段互斥、解禁费资产等于计价资产及审计事务规则均未修改。

## Verification

- Web 聚焦测试：通过，2 个文件、70 项测试。
- TypeCheck：`npm --prefix web run typecheck` 通过。
- Lint：`npm --prefix web run lint` 通过。
- 生产策略：`npm --prefix web run test:production-policy` 通过，4 个文件、15 项测试。
- 覆盖率门禁：`npm --prefix web run test:coverage` 通过，4 个文件、23 项测试；Statements 85.61%、Branches 81.78%、Functions 85.33%、Lines 92.87%。
- Web 全量测试：`npm --prefix web run test` 通过，63 个文件、449 项测试。
- Build：`npm --prefix web run build` 通过，3772 modules；仅有既存依赖 `lottie-web` direct-eval 与大 chunk 警告。
- Budget：`npm --prefix web run budget` 通过；initial gzip 445038 bytes，largest async gzip 60075 bytes。
- Rust 格式：`cargo fmt --all -- --check` 通过。
- Rust TypeCheck：`cargo check --all-targets` 通过。
- Rust 聚焦测试：命令结果 1 passed / 0 failed / 92 filtered；输出明确提示 `DATABASE_URL` 缺失并跳过 MySQL 实库分支，未执行实库断言。
- Diff：`git diff --check` 通过。
- Trellis：`python3 ./.trellis/scripts/task.py validate 09-03-admin-new-coin-asset-time-fix` 通过，implement 4 条、check 5 条。

