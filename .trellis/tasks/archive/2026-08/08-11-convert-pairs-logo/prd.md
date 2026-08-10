# 修复闪兑交易对接口缺少 Logo

## Goal

让公开接口 `GET /api/v1/convert/pairs` 在返回闪兑交易对及双方资产符号时，同时返回数据库中对应资产的 Logo，供手机端、PC 端等调用方直接展示真实后台上传图片。

## What I Already Know

- 当前 `ConvertPairResponse` 只有 `from_asset_symbol` 与 `to_asset_symbol`，没有 Logo 字段。
- 查询已经分别关联 `assets from_assets` 和 `assets to_assets`，可以直接读取双方资产的 `logo_url`，无需新增查询、迁移或外部图片服务。
- `assets.logo_url` 是可空字段；项目既有 Logo 合同要求保留数据库原值，缺失时返回 `null`，禁止根据 symbol 拼接图片路径。
- `/convert/pairs` 是公开接口，本次只扩展响应字段，不改变鉴权、筛选、排序、限额和交易规则。

## Requirements

- `GET /api/v1/convert/pairs` 每条交易对新增 `from_asset_logo_url` 与 `to_asset_logo_url`。
- 两个字段分别来自 `from_assets.logo_url` 与 `to_assets.logo_url`。
- 字段类型为可空字符串；数据库未配置图片时返回 JSON `null`。
- 保留现有响应字段和语义，不修改闪兑报价、确认、订单或后台管理接口。
- 增加无需数据库即可运行的序列化合同测试，并增强真实 MySQL 路由测试验证 Logo 传播。

## Acceptance Criteria

- [x] 配置双方资产 Logo 后，`/api/v1/convert/pairs` 返回对应的 `from_asset_logo_url` 与 `to_asset_logo_url`。
- [x] 未配置 Logo 时字段仍存在且值为 `null`。
- [x] Logo 值原样来自数据库，不做 symbol 推导、默认图片替换或外部调用。
- [x] 现有字段、公开访问和分页限制行为保持兼容。
- [x] Rust 格式检查、编译检查和 convert 定向测试通过。

## Definition of Done

- 响应 DTO、SQL 查询和回归测试完成。
- 最贴近改动的 Rust 质量门通过。
- `docs/superpowers/PROGRESS.md` 更新。

## Technical Approach

在 `ConvertPairResponse` 增加两个 `Option<String>` 字段，并在现有 `list_convert_pairs` JOIN 查询中为双方 `assets.logo_url` 设置同名别名。通过纯序列化单测锁定字段存在和 null 语义；数据库路由测试写入不同 Logo URL，验证完整 HTTP JSON 响应。

## Decision (ADR-lite)

**Decision**: 使用方向语义明确的 `from_asset_logo_url` / `to_asset_logo_url`，与现有 `from_asset_symbol` / `to_asset_symbol` 一一对应。

**Consequences**: 这是向后兼容的响应扩展；调用方可以直接消费后台资产图片，同时继续自行处理 `null` 的视觉降级。

## Out of Scope

- 不新增或修改数据库迁移。
- 不修改资产 Logo 上传流程。
- 不改动手机端或 PC 端具体展示逻辑。
- 不为缺失 Logo 生成默认 URL。

## Technical Notes

- 主要文件：`src/modules/convert/presentation.rs`、`src/modules/convert/infrastructure.rs`、`tests/unit_src/src_modules_convert_mod_tests.rs`、`tests/convert_routes.rs`。
- 参考合同：`.trellis/spec/backend/market-favorites.md`。
