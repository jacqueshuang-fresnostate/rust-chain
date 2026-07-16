# PC 端闪兑功能对接

## Goal

让 PC 端 `/swap` 页面使用 Rust 后端真实闪兑接口完成交易对展示、报价、确认兑换和最近订单展示，避免继续使用写死币种和本地行情估算。

## What I Already Know

- 后端用户闪兑接口已存在：`GET /convert/pairs`、`POST /convert/quote`、`POST /convert/confirm`、`GET /convert/orders`。
- 后端单条闪兑交易对支持正向和反向报价；PC adapter 已把一条交易对展开为正反两个方向。
- 当前 `pc/src/api/swap.ts` 已经调用后端报价和确认，但页面仍写死 `ETH/USDT`，并使用 market ticker 本地估算兑换金额。
- 当前 `/swap` 页面没有从后台闪兑配置加载可选币种，也没有展示用户最近闪兑订单。

## Assumptions

- MVP 以后台配置的启用闪兑交易对为唯一可选来源。
- 兑换按钮可以在一次操作中获取最新报价并确认；页面展示的报价用于预览，提交前仍以最新报价为准。
- 最近订单只展示当前用户的后端闪兑订单列表，不新增独立订单详情页。

## Requirements

- PC API 层提供可复用的闪兑交易对、报价、确认、订单读取函数。
- `/swap` 页面加载启用交易对，并按可用方向选择源资产和目标资产。
- 页面显示用户钱包余额，并校验金额、余额、最小/最大兑换金额。
- 后台闪兑交易对可分别配置源资产和目标资产作为支付资产时的最小/最大兑换金额；PC 端正反向切换时使用对应方向的限额。
- 页面使用 `/convert/quote` 获取真实兑换报价，展示预计到账、汇率和报价过期时间。
- 点击兑换时使用最新报价调用 `/convert/confirm`，成功后刷新余额、订单和报价状态。
- 页面展示最近闪兑订单，订单中的资产符号优先通过交易对/钱包数据解析，避免显示裸 ID。

## Acceptance Criteria

- [ ] 页面不再写死 `ETH/USDT` 或用现货行情 ticker 计算兑换金额。
- [ ] 可选资产来自后端 `/convert/pairs`，并支持同一交易对正反向兑换。
- [ ] 兑换操作会调用 `/convert/quote` 和 `/convert/confirm`。
- [ ] 最近订单来自 `/convert/orders`。
- [ ] 同一闪兑交易对正向使用源资产限额，反向使用目标资产限额。
- [ ] PC type-check 通过，相关 adapter/source wiring 测试通过。

## Definition of Done

- Tests added or updated for adapter/API wiring.
- PC type-check passes.
- Touched files pass whitespace checks.
- Progress recorded in `docs/superpowers/PROGRESS.md`.

## Out of Scope

- 不改后端闪兑撮合、钱包结算、代理佣金逻辑。
- 不新增订单详情页或导出功能。
- 不重做 PC 全站设计体系。
- 不接入新的行情源作为闪兑定价来源。

## Technical Notes

- Key files: `pc/src/api/swap.ts`, `pc/src/views/Swap.vue`, `pc/src/api/backendAdapters.ts`, `pc/tests/backendAdapters.test.ts`, `src/modules/convert/routes.rs`。
- Backend quote TTL is 30 seconds; submit should request a fresh quote before confirm to avoid stale preview.
