# N07 独立复审

## 范围

- 已读取 AGENTS、PROGRESS 尾部、PRD/check.jsonl、相关规范及 N01/N03/N05/N06 报告。
- 审查共享 numeric/time/config/auth 调用边界和 Web 字符串金额入口；保留既有 dirty work。
- 未修改后端、任务元数据、PROGRESS 或索引；未启动服务、提交、部署或操作生产。

## 已修复发现

### P2：杠杆档位预填把已丢精度的 JSON Number 重新认证为金额字符串

- 文件：`web/src/admin/resources/actions/margin.tsx:184`。
- 原路径：数组同时接受 string/number，再 `String(level)`；
  `JSON.parse('2.000000000000000001')` 已变成 `2`，后续精度校验无法恢复原值。
- 修复：非字符串/空档位保留为显式无效草稿，阻止下一步和保存，不过滤掉该项。
- 证据：`numericSafety.test.tsx:145` 使用真实编辑组件，修复前下一步错误启用，
  修复后禁用且 mutation 请求为零。

### P2：提现梯度预填既可静默舍入，也可静默删除缺失金额的限制

- 文件：`web/src/admin/resources/actions/wallet.tsx:174`、`:213`。
- 原路径：`formValueString` 接受 Number；缺少 `min_amount` 或
  `fee_rate_percent` 的梯度随后被过滤，保存可能覆盖为更少/空梯度。
- 修复：只接受十进制字符串；无效项保留并阻止保存。仅显式
  `max_amount=null` 表示无上限，不把无效数字当成清空。
- 证据：`numericSafety.test.tsx:161` 三组真实组件用例覆盖
  `0.123456789123456789` 数字费率、`1.000000000000000001` 数字上限、
  null 必填最小额，修复前全部失败，修复后通过。
- 规范同步：`.trellis/spec/admin/resource-response-contract.md:198`。

### P2：行情策略详情与预设把数字节点/生成器参数转换为可提交字符串

- 文件：`web/src/admin/resources/actions/marketStrategy/model.ts:118`、
  `:123`、`:145`、`:305`、`:341`。
- 按主会话的明确追加范围验证此模型预填链，没有扩展其他模块。
- 修复前四条实际函数链复现均失败于精确 payload 断言，且此前的
  `isMarketStrategySubmittable(..., true)` 均返回 true：
  详情及预设各自把 `JSON.parse('1.000000000000000001')` 的节点目标
  输出为 `"1"`，把 `JSON.parse('0.123456789123456789')` 的生成器
  mean_reversion_strength 输出为 `"0.12345678912345678"`。
- 修复：模型内 `decimalDraft` 只接收字符串；显式 Number/null/boolean
  保留为无效草稿，随后验证与 payload 创建拒绝。仅 undefined 使用原有
  默认值；可选节点成交量的 null/undefined 仍表示未配置，字符串 `"0"`
  仍是精确零。详情顶层金额也使用同一适配，未改变 ID/日期/进度数字。
- 回归：`marketStrategy/model.test.ts:32`、`:45` 覆盖详情/预设两入口，
  五个节点金额字段和三个生成器参数；`:57`、`:74`、`:97` 验证合法
  18 位字符串、尾随零、零/null、缺省默认、显式无效值及详情顶层金额。
- 首轮复现命令：
  `npm --prefix web run test -- src/admin/resources/actions/marketStrategy/model.test.ts -t high-precision`；
  4/4 精确值断言失败，实际丢失值如上。修复后验证见追加门禁。

## 后端复审

- `src/numeric.rs:14` 用系数/指数和 i128 校验容量，不重缩放；
  `:52` 在 BigDecimal 构造前限制文本/指数。
- `src/numeric.rs:104` 新列表适配器逐项复用相同解析，保留 null/空数组；
  `tests/numeric_contract.rs` 递归识别 Option/Vec 形状，未知包装失败。
- `src/time.rs:10`、`:23` 分离 TIMESTAMP 存储界限和生成到期时间；
  通用 Unix 毫秒编解码故意不受数据库范围限制，不应一并收紧。
- `src/config.rs:128` 在 from_env 返回前验证定时配置；
  `src/modules/auth/service.rs:358`、`:426` 在会话创建和刷新记录写入前
  校验到期时间。没有将可选数据库测试的早退当作实库验证。
- 行情 provider、synthetic snapshot 与 K 线恢复入口复用受限解析；
  恢复槽位改为整数对齐和 checked 日期运算。本轮未发现新的确定性后端回归。

## 验证

- 修复前四条新增回归均失败。首次运行的两个提现用例因缺少 jsdom
  matchMedia 先遇到环境错误；补齐该浏览器 API 后重新确认四条行为断言失败。
- `npm --prefix web run typecheck`：通过。
- `npm --prefix web run lint`：通过。
- `npm --prefix web run test -- src/admin/resources/actions/numericSafety.test.tsx src/admin/resources/resourceConfigs.test.tsx src/shared/decimal.test.ts src/api/adminResources.test.ts src/api/agent.test.ts`：
  5 文件、158/158 通过。
- `npm --prefix web run build`：通过；原有 lottie eval/大 chunk 提示仍在。
- `npm --prefix web run budget`：通过；首包 JS gzip 461386 字节，
  总 JS gzip 749455 字节。
- 定向 `git diff --check`：通过。
- 首轮 `npm --prefix web run test`：96 文件、891/891 通过，0 失败；
  使用仓库默认并发，226.69 秒。本轮新增 4 项，原 N03 为 887 项。
- 没有运行 Cargo 套件或数据库测试，避免与主会话最终门禁及专属实库回归争用。
  其他会话的 Rust/实库结果以其交付记录为准。

### 追加模型复核门禁

- 追加修改仅 `marketStrategy/model.ts`、`marketStrategy/model.test.ts`
  及本报告；新增 8 项回归。没有修改后端、其他业务模块或布局。
- `npm --prefix web run test -- src/admin/resources/actions/marketStrategy/model.test.ts src/admin/resources/actions/marketStrategy/MarketStrategyPreviewAction.test.tsx src/admin/resources/resourceConfigs.test.tsx`：
  3 文件、109/109 通过。
- `npm --prefix web run typecheck`、`npm --prefix web run lint`：均重新通过。
- `npm --prefix web run test:production-policy`：4 文件、15/15 通过。
- `npm --prefix web run test:coverage`：4 文件、23/23 通过；
  Statements 85.61%、Branches 81.78%、Functions 85.33%、Lines 92.87%。
  这是仓库指定基础模块门禁，不冒充本次模型覆盖率。
- `npm --prefix web run build`、`npm --prefix web run budget`：均重新通过；
  首包 JS gzip 461381 字节、总 JS gzip 749492 字节。
- `npm --prefix web run test`：96 文件、899/899 通过，0 失败，
  默认并发、191.33 秒；本轮追加 8 项，累计比 N03 新增 12 项。
- 定向 `git diff --check`：通过；所有已启动检查均已结束。

## 复审结论与限制

- 已确认发现均已修复；所检查的共享后端边界没有新增确认问题。
- 追加的行情策略模型预填链已复现并修复，除此没有扩大审查。
  本报告不声称所有动态 JSON 和表单预填路径均已穷尽审计。
- 本轮没有重新运行浏览器；真实组件验证在 jsdom 内使用本地 mock，
  不表示真实 API 曾返回这些损坏数据，也不证明线上曾发生数值损失。
