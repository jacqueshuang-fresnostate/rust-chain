# 手机端资产页接入今日收益

## Goal

让手机端资产页会员 Hero 的“今日收益”消费现有受保护接口
`GET /wallet/today-return`，替换当前写死的 `-- / 暂无数据`，并与首页保持同一
UTC 当日已实现收益口径、严格数据适配和会话竞态边界。

## What I already know

- 后端接口、DTO 严格适配器和请求生命周期已由首页今日收益任务实现。
- `AssetsView.vue` 当前已登录 Hero 的今日收益金额仍固定为 `--`。
- 资产页已有独立余额显隐开关、会员/访客分支、钱包加载状态和真实资产估值。
- 本任务不需要新增第二个后端接口，也不改变收益计算口径。

## Assumptions

- 资产页直接复用 `fetchTodayReturn`、`createTodayReturnRequestLifecycle`、
  `isCompleteTodayReturn` 和 `TodayReturn`。
- 今日收益请求与钱包/行情请求互相独立；任一失败不覆盖另一块真实状态。
- 金额显示使用现有资产页 `pencil-numeric` 视觉，详情显示收益率或已有本地化状态。

## Requirements

1. 仅登录用户请求 `GET /wallet/today-return`，访客分支不发起受保护请求。
2. 资产页只在响应为严格有效的 `complete` 状态时显示真实金额、报告资产和收益率。
3. `loading`、`partial`、`error` 与无会话状态不得显示部分收益金额；使用现有本地化状态文案。
4. 余额隐藏时今日收益金额和详情同步遮蔽，不泄露数值或缺价信息。
5. 正收益、负收益和零收益分别使用正向、负向和中性语义色。
6. token 切换、退出登录、重复请求和组件卸载必须隔离迟到响应；仅最新会话可更新页面。
7. 今日收益加载失败不得影响总资产、持仓、划转和资金入口；钱包加载失败也不得覆盖已独立返回的今日收益。
8. 保持现有 Pencil 资产页布局、访客页面、双主题、触控与安全区行为不变。

## Acceptance Criteria

- [x] 资产页不再包含写死的今日收益 `--` 合同。
- [x] 已登录且接口返回 `complete` 时展示带符号金额、报告资产和真实收益率。
- [x] 隐私关闭、加载、部分数据、失败、访客状态均不泄露真实或部分收益数值。
- [x] 退出登录、换号登录和卸载后的迟到响应不能回写。
- [x] 资产页相关测试、Mobile 全量测试、类型检查、PWA/Tauri 构建及 diff 检查通过。

## Definition of Done

- 测试覆盖完整/部分/错误/隐私/会话竞态状态。
- Mobile 类型检查、测试及双构建通过。
- 进度记录更新；如形成新可复用约束则同步移动端规范。

## Out of Scope

- 修改后端今日收益聚合公式、数据库查询或 Redis 行情估值。
- 增加未实现收益、总资产快照收益或本地模拟收益。
- 重构资产页其他持仓、划转或资金路由。

## Technical Notes

- 目标页面：`mobile/src/views/AssetsView.vue`
- 复用边界：`mobile/src/api/wallet.ts`、`mobile/src/core/todayReturn.ts`
- 现有参考：`mobile/src/views/HomeView.vue`、`mobile/tests/today-return.test.ts`
- 接口研究合同：`../08-09-mobile-home-today-return/research/today-return-contract.md`

## Independent Review Amendments

- [x] 将精确会话、latest-request-wins 和卸载失效抽为通用请求生命周期；
  今日收益与资产页钱包/杠杆钱包读取分别持有独立实例。
- [x] 资产页钱包读取从布尔登录态改为精确 token 监听；换号、退出和卸载会清空
  前一会话状态并阻止迟到响应回写，进行中的划转也不会提交到新会话页面状态。
- [x] 今日收益适配器把十进制负零归一化为正零，避免中性零收益显示为
  `-0 USDT`。
- [x] 隐私关闭时金额、比例、partial 缺价详情、状态属性和 busy 状态均由隐藏态
  优先覆盖。
- [x] 提取可执行的今日收益展示模型测试，直接断言 complete 正/负/零、partial、
  loading、error、idle 和隐私输出；源码合同只负责确认资产页实际接线。
- [x] 今日收益文本被约束在现有 Pencil Hero 网格列内，长金额或状态文案不会造成
  水平溢出。

## Independent Review Verification

- [x] 聚焦测试
- [x] Mobile 全量测试
- [x] Mobile type-check 与 lint（项目无 lint 脚本，`--if-present` 正常退出）
- [x] PWA 与 Tauri 构建
- [x] Trellis task validate 与 `git diff --check`
