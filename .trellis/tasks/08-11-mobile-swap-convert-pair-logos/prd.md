# 手机闪兑改用交易对接口 Logo

## Goal

让手机端闪兑页面直接消费 `GET /api/v1/convert/pairs` 返回的 `from_asset_logo_url` 与 `to_asset_logo_url`，不再依赖登录用户的钱包账户响应提供币种图片。

## What I Already Know

- 后端交易对接口已返回可空的 `from_asset_logo_url` 与 `to_asset_logo_url`，值直接来自双方 `assets.logo_url`。
- 当前 `mobile/src/api/swap.ts` 未适配这两个字段，`SwapView` 仍通过 `wallet_accounts.logo_url` 按 symbol 查找图片。
- 闪兑主卡片、收款卡片和资产选择弹层都使用 `AssetMark`；传入空图片时组件已有 symbol 回退，不需要生成默认 URL。
- 钱包账户请求仍用于可用余额、持仓筛选和订单操作，本次只变更图片来源。

## Requirements

- `BackendConvertPair` 接收可空 `from_asset_logo_url` 与 `to_asset_logo_url`。
- `ConvertPair` 暴露可选 `fromAssetLogoUrl` 与 `toAssetLogoUrl`，适配时 trim 空白，`null`、缺失和空字符串统一为 `undefined`。
- 支付资产主卡片使用当前交易对的 `fromAssetLogoUrl`，获得资产主卡片使用 `toAssetLogoUrl`。
- 资产选择弹层根据当前选择方向，从对应交易对方向字段构建 symbol 与 Logo；同一 symbol 重复出现时保留首个非空 API Logo。
- 钱包账户元数据继续负责余额和“持有”筛选，但不再作为闪兑图片来源。
- API 未返回 Logo 或图片加载失败时继续使用 `AssetMark` 现有字母回退。

## Acceptance Criteria

- [x] API 适配器完整映射双方 Logo，并正确归一化 null/空白值。
- [x] 主卡片两侧只使用当前 `ConvertPair` 对应方向的 API Logo。
- [x] 选择器图片来自 `convert/pairs`，余额仍来自 `wallet/accounts`。
- [x] 页面中不再存在按钱包账户 symbol 获取闪兑 Logo 的逻辑。
- [x] Logo 缺失时不生成默认 URL，`AssetMark` 可继续降级。
- [x] Mobile 聚焦测试、全量测试、type-check 与 PWA build 通过。

## Definition of Done

- Mobile API DTO、适配器、Swap 页面和回归测试完成。
- 最贴近改动的 Mobile 质量门通过。
- `.trellis/spec/mobile/backend-integration.md` 与 `docs/superpowers/PROGRESS.md` 更新。

## Technical Approach

在 `ConvertPair` 映射边界保存双方 Logo；`SwapView` 主卡直接读取当前 pair 字段，选择器按 from/to 方向遍历 pair 构造去重资产列表并保留 API 中首个有效 Logo。钱包账户 Map 只保留余额查询职责。

## Decision (ADR-lite)

**Decision**: `convert/pairs` 是闪兑资产视觉元数据的权威来源；钱包账户是用户余额元数据来源，两者不交叉代替。

**Consequences**: 未登录或尚未创建钱包账户的资产也能获得后台配置图片；图片与可交易 pair 保持一致；后端返回 null 时仍由本地组件安全降级。

## Out of Scope

- 不修改后端接口或数据库。
- 不修改钱包账户 Logo 合同及其他资产页面。
- 不引入第三方币种图片服务或本地 symbol-to-URL 表。
- 不调整闪兑报价、余额、历史订单或页面视觉布局。

## Technical Notes

- 主要文件：`mobile/src/api/swap.ts`、`mobile/src/views/SwapView.vue`、`mobile/tests/swap-asset-logos.test.ts`。
- 后端字段：`from_asset_logo_url: string | null`、`to_asset_logo_url: string | null`。
