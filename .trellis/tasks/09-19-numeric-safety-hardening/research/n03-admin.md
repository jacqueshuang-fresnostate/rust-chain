# N03 Admin / Agent 数值安全交付

日期：2026-09-19（HKT）。范围仅 `web/` 与 Admin 专用规范。保留既有
dirty work；未修改其他端、PROGRESS、任务元数据、spec index，未提交或部署。
本文记录本次修改，不把 `git diff` 内其他会话的已有改动算成本次工作。

## 数值用途库存

| 类别 / 入口 | 本次判断与处理 |
| --- | --- |
| 通用金额入口 `shared/decimal.ts` | `decimalFitsStorage` 只接受 string，默认38,18；源文本最长256字符、指数绝对值不超过256，拒绝超有效小数位/整数位，不舍入。`requiredDecimalText` 返回 trim 后原文，尾随零保留。 |
| JS Number 金额 | 明确拒绝 `JSON.parse('0.123456789123456789')` 等已丢原文的 number 进入存储校验；不能通过 String(number) 恢复精度。旧 `canonicalDecimalText` / `formatDecimalText` 的有限小数 Number 兼容仅用于显示，不是权威金额入口。 |
| 产品表单 | 秒合约、闪兑、杠杆、理财、贷款、新币、竞猜、市场交易对/策略、用户人工充值、手工成交：实际提交函数/表单检查金额38,18、已有产品区间，实际数据库 rate/leverage18,8。保留业务规则与源文本。 |
| 资产手续费 | 资产精度限0..18；充值/提现费用、提现阶梯金额检查资产精度及存储容量。阶梯 fee_rate_percent 是JSON百分比配置，不强套rate18,8或新增100上限。 |
| 快速充值 | 按真实36,18容量；金额控件改为文本，`12.50` 不再自动变为`12.5`，测试金额同样校验。 |
| 额度 / 资金预算 | 秒合约赔付额度、闪兑资金库存、理财本金/负债容量、贷款本金容量保持 exact decimal，库存精度使用目标资产元数据；不把预算当作已核验余额。 |
| 人工结算派生值 | 现货成交与新币分配的乘积用现有精确乘法，只检查整数溢出；不在前端擅自决定后端资产量化。源价格/数量单独拒绝超精度。 |
| 佣金 / Agent统计 | 新增规则比例检查0..1及8位精度；非法批量ID整批拒绝。列表按 payout_asset_id 精确合计并标明“已加载”，缺失币种不参与合计。Dashboard使用后端新增commission_assets，不显示旧跨币种总和。闪兑总额没有按资产语义，因此隐藏金额并说明缺少分币种统计。 |
| Admin列表 / 明细 / CSV | 金额列响应必须string，读取允许65,18聚合容量；实际写入仍用字段窄边界。现有格式化使用十进制算法，小额非零显示阈值，CSV保留原文且标注“当前已加载数据（非全量）”，未为此改写导出模块。 |
| Agent财务DTO | 钱包、现货、杠杆、秒合约及旧佣金接口检查金额string，普通金额38,18，合计65,18；类型取消money string/number union。缺失值不伪装0。 |
| 通用整数 / ID | 新增shared/integer：先十进制整数词法再安全范围；拒绝unsafe、微小小数、指数、hex、NaN/Infinity。资源操作、代理ID、赠币ID、SMTP测试ID、KYC等级、上传字节数、Agent路由/邀请码应用边界。订单号fallback对数字字符串使用BigInt基36，不经Number舍入；它仍只是显示号。 |
| JSON Number响应 | api/client拒绝非有限数和不安全整数Number，不能替代领域字段类型验证，也不能恢复已经舍入的原始JSON。Admin/Agent金额字段另行强制string；未知非金额有限小数不被全局粗暴改成整数。 |
| 分页 / 计数 | Admin资源及Agent财务limit/offset在请求前检查u32；响应total须非负安全整数；邀请使用上限i32。Dashboard单项或合计不安全则显示缺失而非舍入。列表分页仍是后台/本地原有模式，不假装全量。 |
| 提现策略整数表单 | InputNumber改为文本保留原文；非空非法值进入无效draft并禁用保存，绝不当作null解除限制。冷静期/滚动窗口u32、KYC i32、复核人数1..20，保存再次验证。 |
| 时间 / 版本 | shared TimestampText及AgentDTO要求安全Unix毫秒和Date范围；已有日期解析拆解四位年份/月日/毫秒并回验，保留Number。库存/审计配置版本原有词法+safe验证保留。API超时为1..2147483647毫秒。 |
| 有限数UI / 图表（允许） | `MarketStrategyPreviewChart`只把OHLC文本映射为有限、正且OHLC一致的SVG坐标；tooltip/原始表仍保留文本，不用于下单。`ResizableTable`宽度parseFloat、像素和拖动差值、DataTable几何保留。 |
| 其他允许Number | 市场预设0..100进度映射到显式分钟网格；datetime部件；已验证范围的深度档数/秒数；WebSocket有限退避抖动；KYC文件MB输入显式转字节并Math.round且检查safe integer。这些不是财务金额。 |
| 已有治理页 | 对账、重试、快照已有逐字段金额string、安全ID/count、时间及有界分页合同；继续保持原文和差异范围，不重写其业务逻辑。源码扫描发现的Number主要在已校验计数/时间上，未泛化替换。 |

扫描使用 `rg` 覆盖 Admin、Agent、API、shared 的
`Number/parseInt/parseFloat/InputNumber/toFixed/Math`、金额序列化、CSV与统计路径；
不声称对未来新增字段、任意未声明JSON或数据库历史值完成验证。

## 修改文件

- 公共边界：`web/src/shared/{decimal.ts,decimal.test.ts,integer.ts,TimestampText.tsx,orderNo.ts}`。
- API：`web/src/api/{client.ts,client.test.ts,adminResources.ts,adminResources.test.ts,agent.ts,agent.test.ts}`。
- 产品/资源动作：`web/src/admin/resources/actions/{shared,market,secondsContract,convert,margin,loan,earn,wallet,agents,users,newCoins,spotFill}.tsx`，
  `marketStrategy/model.ts`、`defaultMarket/{model.ts,model.test.ts}`。
- 实际表单回归：`web/src/admin/resources/actions/numericSafety.test.tsx`，
  `web/src/admin/resources/resourceConfigs.test.tsx`（仅清空资产默认精度后输入，保留已有改动）。
- 配置/新币：`web/src/admin/actions/{AgentManagementPage,KycManagementPage,NewCoinManualDistribution,PredictionMarketRowActions,QuickRechargeConfigPage,UploadConfigPage,WithdrawalPolicyPage}.tsx`；
  `QuickRechargeConfigPage.test.tsx`、`WithdrawalPolicyPage.test.tsx`、`withdrawalPolicy.ts`、`withdrawalPolicy.test.ts`；
  `prediction/model.ts`、`smtp/{model.ts,useSmtpConfigWorkspace.ts}`；
  `web/src/admin/new-coins/{NewCoinProjectSettings,NewCoinGrant}.tsx`。
- Agent / Dashboard：`web/src/agent/{UserPortfolioPage.tsx,pages.tsx,pages.test.tsx}`，
  `web/src/admin/dashboard/DashboardPage.tsx`。
- 文档：本文件及 `.trellis/spec/admin/{ui-system,resource-response-contract}.md`。

以上部分文件在本任务前已修改或未跟踪，未覆盖其既有功能；access/routes/navigation、
治理页、portfolio CSS等工作区改动不归本次新增。

## 测试与门禁

以下为停止业务源码修改后的真实门禁结果（2026-09-19 HKT）：

| 命令 | 结果 / 证据 |
| --- | --- |
| `npm --prefix web run typecheck` | exit 0；最终build中的tsc也再次通过。 |
| `npm --prefix web run lint` | exit 0。 |
| `npm --prefix web run test -- --reporter=json --outputFile=/tmp/n03-final-tests-stable.json` | exit 0；96文件、887通过、0失败、0跳过。使用仓库默认maxWorkers配置。 |
| `npm --prefix web run test:production-policy -- --reporter=json --outputFile=/tmp/n03-production-policy.json` | exit 0；4文件、15/15。 |
| `npm --prefix web run test:coverage` | exit 0；4文件、23/23；Statements85.61%、Branches81.78%、Functions85.33%、Lines92.87%。这是仓库指定的4个基础模块覆盖率，不代表全部改动达到同样覆盖率。 |
| `npm --prefix web run build` | exit 0；3815模块；依赖lottie-web直接eval及大chunk警告仍存在。 |
| `npm --prefix web run budget` | exit 0；初始JS gzip461378字节，最大异步Quill60075字节，CSS73435字节，总JS749441字节；action registry懒加载、Quill不进首包。 |
| `git diff --check -- web .trellis/spec/admin .trellis/tasks/09-19-numeric-safety-hardening/research/n03-admin.md` | exit 0。 |

- 早期聚焦8文件97项通过；最后一轮金额/提现策略/真实表单/响应边界聚焦4文件73项通过。
  最后一轮聚焦JSON：`/tmp/n03-final-focused.json`。
- 早期全量出现8项失败，修复了新增约束后的错误提示顺序、尾随零预期、
  资产默认精度重复输入；两个资源页失败用例单独重跑2/2通过。
- 中途全量877通过、4失败：测试运行期间编辑了共享decimal边界，旧模块缓存与新测试混用。
  不把此轮计为通过；上表稳定全量已覆盖并通过这些新增用例。
- 当前无剩余Web测试失败。未新增依赖、提高timeout、跳过断言或修改门禁阈值。

## 浏览器证据

Ego TaskSpace 31，Vite `127.0.0.1:3037`，临时脚本
`/tmp/n03-browser-mock.js` 注入假会话与全量API fetch mock，未知API返回404、
非同源非API请求被拒绝。所有变更请求只进入内存记录，没有真实后端写入。
Vite首次监听受sandbox限制，已按批准流程重跑。编辑源码触发热重载后曾丢失
CDP脚本注入，重新注入后继续，不把该中途权限网络错误算作数值测试通过。

- 1440x1000 提现策略：输入`9007199254740993`原文保留，
  保存disabled=true，mutation请求0，document horizontal overflow=0。
  截图 `/tmp/n03-policy-desktop.png`。
- 390x844 提现策略：输入`1.000000000000000001`仍为完整原文，
  保存disabled=true，mutation请求0，document horizontal overflow=0。
  截图 `/tmp/n03-policy-mobile.png`。
- 390x844 快速充值：逐一输入`1e-19`、`1000000000000000000`（36,18溢出）、
  `1e9999999`、`NaN`、`Infinity`，每次测试按钮disabled=true，mutation请求0。
- 同一窄屏实际操作确认窗后，mock捕获完整POST如下：

```json
{"amount":"9007199254740993.000000000000000001","reason":"N03 local mock precision check"}
```

  响应面板DOM保留同一充值金额，实际支付数量为`0.000000000000000001 USDT`，
  没显示成零。截图 `/tmp/n03-recharge-mobile.png`；document horizontal overflow=0。
- 改为1440x1000，再经确认窗提交，第二次mock POST如下：

```json
{"amount":"12.50","reason":"N03 trailing zero mock check"}
```

  结果面板保留`12.50 CNY`与`0.000000000000000001 USDT`。
  截图 `/tmp/n03-recharge-desktop.png`；document horizontal overflow=0。
- 四张截图均实际打开检查。390px默认展开侧栏占用184px，导致正文窄、顶部标题
  逐字换行，长金额输入需横向滚动查看；这是现有shell布局，未在数值任务中做无关CSS重构。
  额外尝试收起侧栏时按钮不可见，截图步骤失败，没有作为通过证据。
- 浏览器仅创建并复用TaskSpace31。截图和JSON是本机`/tmp`证据，本文保留关键原始
  值与结果，避免临时目录清理后丢失验证结论。

## 边界与后续

- 本轮不修改结算/量化，也不把前端校验当作后端保护。缺少精度元数据的产品/
  新币DTO只能在前端执行存储容量检查；真实资产精度由后端权威验证，不能猜测。
- 通用JSON检查无法恢复合法JS范围内已被解析器舍入的小数。金额wire必须是字符串；
  ID/count仍沿用number协议并逐字段safe检查，若将来支持>2^53身份需端到端迁移string协议。
- 旧Agent接口没有所有必填字段的完整schema；本轮加强数值字段并保留已有兼容，
  不将缺币种/缺统计结果显示成0。闪兑按币种汇总仍需后端新接口。
- 浏览器是本地模拟，不代表真实数据库持久化、并发冲突、生产配置或历史数据审计；
  多类真实表单由Vitest覆盖，浏览器只抽查本轮高风险输入控件。390px默认shell的
  可读性改善属于独立UI后续，不影响本轮原文保留/禁用/精确提交断言。
