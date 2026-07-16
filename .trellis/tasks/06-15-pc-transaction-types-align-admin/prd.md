# PC端交易记录类型对齐后台

## Goal

PC 端交易记录的“类型”必须和后台钱包流水的“变动类型”保持同一套业务枚举与中文含义，避免用户端显示旧数字类型或按来源类型误判。

## What I Know

- 后端用户钱包流水接口 `/wallet/ledger` 返回 `change_type`、`ref_type`、`amount`、`fee` 等字段。
- 后台钱包流水已经使用 `walletLedgerChangeTypeLabels` 将 `change_type` 翻译为中文。
- PC 端当前在 `normalizeWalletLedgerPage` 中用 `ref_type` 猜测旧数字枚举，导致快速充值、现货、闪兑、理财等类型和后台不一致。

## Requirements

- PC 交易记录接口模型保留后台 `change_type` 字符串作为记录类型。
- PC 交易记录筛选项按后台钱包流水变动类型提供。
- PC 中文/英文 i18n 覆盖后台已有的所有钱包流水变动类型。
- 交易金额正负以后端 `amount` 为准，不再按旧 `credit/debit` 或旧数字类型推断。
- 未识别的新类型展示兜底文案，不阻断列表渲染。

## Acceptance Criteria

- PC 交易记录列表不再显示或筛选旧的 `RECHARGE/WITHDRAW/OTC_*` 数字枚举。
- `quick_recharge` 在 PC 和后台都显示为“快速充值”。
- `spot_trade_settlement`、`convert_settlement`、`earn_subscribe` 等后台流水类型在 PC 有对应文案。
- PC adapter 测试断言 `normalizeWalletLedgerPage` 输出后台 `change_type` 字符串。
- PC 类型检查和相关测试通过。

## Out of Scope

- 不修改后端钱包流水接口。
- 不调整后台资源配置。
- 不改变 PC 交易记录页面整体样式。
