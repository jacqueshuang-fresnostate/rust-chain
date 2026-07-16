# 后台配置第三方账号绑定开关

## 背景

用户需要后台可配置 Coinbase 钱包绑定和 TG 账号绑定能力：

- 后台开关开启后，PC 用户安全页可以绑定对应账号。
- 后台开关关闭后，PC 不支持绑定，并且后端接口必须拒绝绑定请求。

现有项目已有 `security_policy_configs` 和 `UserSecurityPolicy`，后台“安全策略”页已经可以配置登录 2FA、注册邀请码、资金动作校验，本需求沿用该安全策略模型。

## 目标

1. 安全策略增加第三方绑定开关：
   - Coinbase 钱包绑定开关
   - TG 账号绑定开关
2. 用户端接口返回当前绑定策略和已绑定状态。
3. 用户端绑定接口根据后台开关强制校验。
4. PC 安全页展示 Coinbase 钱包和 TG 账号绑定入口；未开启时显示不支持绑定。
5. 补充必要的数据库表、测试和 i18n 文案。

## 非目标

- 不实现 Coinbase OAuth、钱包签名登录或链上校验。
- 不实现 Telegram Bot OAuth 或 Telegram Login Widget。
- 不增加后台逐用户手动绑定功能。

## 数据与接口

- `UserSecurityPolicy` 新增 `third_party_bindings`，默认两个开关均为关闭。
- 新增用户第三方绑定表，按 `user_id + provider` 保证每个用户每种提供方只有一个当前绑定。
- 新增用户端接口：
  - `GET /api/v1/user/third-party-bindings`
  - `POST /api/v1/user/third-party-bindings`
- `GET /api/v1/user/2fa` 同步返回第三方绑定策略，便于 PC 安全页初始化。

## 验收标准

- 后台安全策略页可以保存和刷新两个第三方绑定开关。
- 开关关闭时，用户提交对应绑定返回明确错误。
- 开关开启时，用户可以保存 Coinbase 钱包标识或 TG 账号标识，并能在状态接口中读回。
- PC 安全页可以根据策略显示可绑定或不支持绑定状态，绑定成功后显示已绑定。
- 相关 Rust、web、pc 类型检查与目标测试通过。
