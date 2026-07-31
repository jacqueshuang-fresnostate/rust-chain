# 手机端远程接口接入与导航修复

## 背景

手机端目前依赖环境变量或本地代理选择后端，发布后的 PWA/Tauri 包需要默认连接
`https://hipoex.cllbmz.kdns.fr`。现有部分页面的返回目标和入口语义不合理，尤其是首页
品牌、交易相关页面、秒合约、充值详情和认证流程。

## 目标

1. 手机端发布包和本地开发代理默认连接远程服务，同时保留环境变量覆盖能力。
2. HTTP API 统一使用 `https://hipoex.cllbmz.kdns.fr/api/v1`。
3. 公共行情 WebSocket 统一使用
   `wss://hipoex.cllbmz.kdns.fr/api/v1/ws/public`。
4. 修复不符合页面层级或丢失来源上下文的跳转与回退。
5. 保持现有七个底部根栏目、`push`/`replace` 语义、秒合约二级动效层级及 PWA
   能力不变。

## 功能要求

### 远程接口

- 提供产品默认后端域名常量
  `https://hipoex.cllbmz.kdns.fr`。
- `VITE_BACKEND_API_DOMAIN` 非空时仍优先使用显式配置。
- API 前缀默认保持 `/api/v1`。
- 本地 Vite 开发代理默认代理到远程服务，仍允许
  `VITE_BACKEND_DEV_PROXY_TARGET` 覆盖为本地后端。
- PWA 和 Tauri 构建均能生成正确的 HTTPS API 与 WSS 行情地址。
- 客户端启动不得依赖 `/health` 成功；该地址当前可能被 Cloudflare Managed
  Challenge 拦截，业务 API 不受影响。
- 更新 `.env.example`、README 和自动化测试中的默认配置说明。

### 导航修复

- 首页 Header 品牌 Logo 点击后回到首页，不再进入个人中心。
- 兑换和订单页面返回时优先回到导航状态中最近一次交易路径，而不是固定
  `BTC_USDT`。
- 通过底部中心入口或直接地址进入秒合约后，返回的安全兜底为首页；底部
  `replace` 入口必须写入显式来源状态，使页面返回不受旧
  `history.state.back` 影响；从产品中心 `push` 进入时，浏览器历史仍应自然
  返回产品中心。
- 直接打开充值地址详情时，返回到当前资产的网络选择页，而不是越级返回资产选择页。
- 登录页跳转注册、忘记密码和登录 2FA 时使用 `replace` 并保留安全的站内来源
  上下文，认证完成后不得在历史栈中残留登录页；语言页仍使用可自然返回的
  `push`。
- 注册成功后使用经过清洗的 `redirect` 返回原业务页；注册返回登录时继续保留
  `redirect`，并明确替换回登录页而不是使用旧根页面历史。
- 忘记密码完成后返回登录页时继续保留 `redirect`。
- 登录 2FA 重置、挑战失效和头部返回登录时继续保留经过清洗的 `redirect`；
  正常验证或设置完成仍安全替换到该业务页。
- 语言页支持安全的 `back` 参数，以便从认证页进入或页面刷新后仍回到合理来源。
- 所有回跳参数必须继续通过站内重定向清洗逻辑，拒绝绝对外链和协议相对地址。

## 非功能要求

- 不修改远程后端、Cloudflare 或 Nginx 配置。
- 不改动 PC 端现有工作区文件。
- 不新增 API 响应缓存。
- 不重构当前视觉设计和业务接口模型。
- 所有新行为需要源代码级或组件级自动化测试覆盖。

## 验收标准

- `npm --prefix mobile run type-check` 通过。
- `npm --prefix mobile test` 通过。
- `npm --prefix mobile run build:pwa` 通过。
- `npm --prefix mobile run build:tauri` 通过。
- 远程 `/api/v1/markets`、登录配置、Tauri Origin CORS 和公共 WebSocket
  连通性验证通过。
- 页面跳转满足上述导航修复要求，并且根栏目仍使用 `replace`、详情页面仍使用
  `push`。
- 自动化测试使用真实 Vue Router 内存历史覆盖 Seconds 的底部/产品中心来源、
  登录到注册/忘记密码/2FA 的历史替换，以及 2FA 重置/失效的安全回跳。
- `pc/src/config/app.ts` 不进入本任务提交。

## 范围外

- 后端业务接口开发。
- PC 管理端修改。
- Cloudflare `/health` 挑战规则修改。
- 手机端全局 UI 重设计。
