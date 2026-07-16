# 快速充值接入 Epusdt/GMPay

## 背景

平台已有普通充值地址池，用于给用户分配链上充值地址。现在需要新增“快速充值”，通过私有化部署的 Epusdt / GM Pay 创建托管收银台订单，用户在 PC 端输入金额后跳转到 GMPay 支付链接，支付成功后由 Epusdt 回调平台，平台验签并给用户钱包入账。

## 外部接口依据

- 官方仓库：<https://github.com/GMWalletApp/epusdt>
- 官方文档：<https://epusdt.com/api/payment>
- 推荐创建订单接口：`POST /payments/gmpay/v1/order/create-transaction`
- 创建订单必填字段：`pid`、`order_id`、`currency`、`token`、`network`、`amount`、`notify_url`、`signature`
- 回调方式：普通 GMPay 使用 `POST` JSON 回调到 `notify_url`
- 回调成功返回：纯文本 `ok`
- 签名规则：排除 `signature`，非空参数按 ASCII 字典序拼接为 `key=value`，以 `&` 连接后直接追加 `secret_key`，计算小写 MD5。

## 范围

- 后台可以配置 GMPay 快速充值参数：
  - 是否启用
  - API Base URL
  - 商户 PID
  - 商户 Secret Key
  - 默认法币 currency
  - 默认 token
  - 默认 network
  - 最小/最大充值金额
  - 同步跳转 URL（可选）
- 用户 PC 充值页增加快速充值入口，输入金额后创建快速充值订单并打开/跳转 `payment_url`。
- 后端保存快速充值订单，包含平台订单号、Epusdt trade_id、金额、币种、网络、支付地址、支付 URL、状态。
- Epusdt 回调后，后端验签、校验订单号和金额，幂等地给用户钱包入账并记录 `wallet_ledger`。
- 后台可以查看快速充值订单列表和状态。

## 数据流

后台配置 GMPay -> PC 用户输入充值金额 -> 后端创建本地订单 -> 后端签名调用 GMPay 创建交易 -> 保存 Epusdt `trade_id/payment_url` -> PC 打开支付链接 -> Epusdt 支付成功回调 -> 后端验签并更新订单 -> 钱包入账并写流水。

## API 合约

用户端：

- `GET /api/v1/wallet/quick-recharge/config`：获取快速充值启用状态和默认币种/网络/金额限制。
- `POST /api/v1/wallet/quick-recharge/orders`：创建快速充值订单。
- `GET /api/v1/wallet/quick-recharge/orders`：查询当前用户快速充值订单。

公开回调：

- `POST /api/v1/payments/gmpay/notify`：Epusdt GMPay 异步回调。

后台：

- `GET /admin/api/v1/quick-recharge/config`：查询 GMPay 配置（secret 脱敏）。
- `PATCH /admin/api/v1/quick-recharge/config`：保存 GMPay 配置。
- `GET /admin/api/v1/quick-recharge/orders`：查询快速充值订单。

## 规则

- 本地订单号长度不超过 32 字符，使用 `QR` + 时间/随机后缀。
- 未启用或配置不完整时，用户端创建订单返回校验错误。
- 回调必须使用配置的 `secret_key` 验签，签名不匹配返回 401。
- 回调状态只有 `2` 才进行入账。
- 回调必须校验 `order_id`、`trade_id`、`pid`，并要求订单处于可更新状态。
- 入账必须在数据库事务内完成，且同一订单只入账一次。
- 钱包流水 `ref_type` 使用 `quick_recharge`，`ref_id` 使用本地订单号。
- 支付金额计入用户充值资产的可用余额；充值资产默认用配置 `token` 匹配资产符号。

## UI 要求

- PC 充值页在普通地址充值之外增加“快速充值”区域。
- 快速充值显示金额输入、默认 token/network、最小/最大金额提示、提交按钮和支付链接。
- 后台系统配置增加“快速充值配置”页面，使用 Semi 表单组件保存配置。
- 后台钱包资产分组增加“快速充值订单”列表。

## 验收

- 后台能配置 GMPay API Base URL、PID、Secret Key 和默认支付参数。
- PC 用户能创建快速充值订单并得到 `payment_url`。
- 创建订单请求签名符合 Epusdt GMPay 规则。
- GMPay 回调验签成功后能幂等入账，重复回调不重复加余额。
- 后台能查看快速充值订单。
- 添加后端路由测试、签名单元测试、后台/PC 前端测试。
