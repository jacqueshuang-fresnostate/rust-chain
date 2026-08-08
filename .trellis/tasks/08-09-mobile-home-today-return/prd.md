# 手机端首页今日收益后端接口

## Goal

为手机端已登录首页补充后端权威的“今日收益”接口，替换当前固定显示的 `--`，让金额、收益率、统计周期和数据完整性都来自真实账户与行情数据，而不是前端会话内估算或演示值。

## What I already know

- `mobile/src/views/HomeView.vue` 的总资产由现货钱包、保证金钱包和移动端实时 ticker 临时估值；“今日收益”金额和比例目前固定为 `--`。
- 用户端现有 `/wallet/accounts` 与 `/margin/wallets` 只返回当前账户余额，没有日初基线或今日收益字段。
- MySQL 已记录 `wallet_ledger` 与 `margin_wallet_ledger`，Redis 有当前 ticker，MongoDB 有 K 线；尚未发现用户资产日快照表或现成的组合收益接口。
- 充值、提现、现货、闪兑、秒合约、预测、杠杆、理财等业务的余额变化语义不同，不能直接把当日全部钱包流水求和当作收益。

## Assumptions

- 统计币种先与首页保持一致，使用 `USDT`。
- “今日”使用后端统一的 UTC 自然日，并在响应中返回统计起点和截止时间，避免设备时区改变统计口径。
- 数据不足时接口必须返回明确的完整性状态；前端显示不可用状态，不得把缺失行情或缺失日初基线当成 0 收益。

## Decision (ADR-lite)

- **Context**：当前没有资产组合日快照和现货用户成本账本，直接声称“自然日组合盈亏”会把资金流或活动本金误算为收益；近 24 小时持仓涨跌也不等同自然日账户收益。
- **Decision**：本期提供 UTC 自然日“已实现业务收益”，聚合秒合约、预测、已平仓杠杆和理财赎回的可审计净收益；按当前 USDT 行情换算并显式返回 `scope=realized` 与完整性状态。
- **Consequences**：结果不包含现货成本收益和未实现盈亏；未来如引入资产日快照/成本账本，可新增 `portfolio` 口径而不破坏当前响应语义。

## Requirements (evolving)

- 新增受用户登录态保护的后端接口，且只能读取当前用户数据。
- 响应至少包含 `scope=realized`、统计币种、今日收益金额、成本基础、收益率、周期开始/截止时间与数据状态。
- 聚合秒合约、预测订单、已平仓杠杆仓位和理财赎回四类可审计收益，并按 USDT 换算。
- 充值、提现和现货/保证金内部划转不得被计为收益。
- 手机端首页只消费该接口，不在前端自行伪造今日收益。
- 请求失败、数据不完整、访客和真实零收益必须有不同状态语义。
- 保留现有首页总资产、行情、公告、导航和主题行为。

## Acceptance Criteria (evolving)

- [x] 已登录用户可通过受保护 API 获取自己的今日收益，其他用户数据不可见。
- [x] 无业务活动时返回真实 `0`，不是缺失值。
- [x] 正收益、负收益和零收益均有确定金额与百分比。
- [x] 充值、提现和内部划转不会改变今日收益。
- [x] 缺失行情或日初数据时响应明确标记不完整，前端不显示伪造数值。
- [x] 杠杆人工平仓与强平都扣除利息；预测退款成本基础、理财流水精确连接和重复流水去重符合研究口径。
- [x] Redis 缺失、畸形、错配、非正数或过期行情都标记不完整，不冒充当前估值。
- [x] 换号登录、退出和卸载会隔离迟到请求；隐私状态不泄露缺价资产，partial 不展示部分数值。
- [x] 首页不再把“今日收益”固定写死为 `--`，并按正负使用既有语义色。
- [x] Rust 路由测试、Mobile 适配器/页面测试、类型检查、构建和 `git diff --check` 通过。

## Definition of Done

- 后端路由、应用编排、查询/估值逻辑和响应 DTO 完成。
- Mobile API 适配器和首页状态接入完成。
- 单元/集成/源码合同测试覆盖权限、边界、正负零与失败状态。
- 相关 Trellis 规格和 `docs/superpowers/PROGRESS.md` 更新。

## Out of Scope

- 不修改首页其余 UI 信息架构。
- 不把现有交易、结算或钱包账务逻辑改成另一套账本。
- 不使用前端本地存储或会话内曲线作为后端收益依据。
- 不扩展 PC 或后台页面，除非共享 API 合同测试需要。
- 不把现货成交的成本收益、持仓未实现盈亏或完整资产组合盈亏纳入本期口径。

## Technical Notes

- 移动端入口：`mobile/src/views/HomeView.vue`、`mobile/src/api/wallet.ts`。
- 后端入口：`src/modules/wallet/{routes,application,infrastructure,presentation}.rs`。
- 可用数据：`wallet_accounts`、`wallet_ledger`、`margin_wallet_accounts`、`margin_wallet_ledger`、业务订单/持仓表、Redis ticker、MongoDB K 线。
- 主要规格：`.trellis/spec/backend/`、`.trellis/spec/mobile/backend-integration.md`、`.trellis/spec/mobile/pwa-and-shell.md`。
- 研究结论：`research/today-return-contract.md`。
