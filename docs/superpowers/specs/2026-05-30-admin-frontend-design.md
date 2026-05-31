# Admin 后台前端设计

## 1. 范围

本设计用于区块链交易所后台页面首期实现，前端放在当前仓库 `web/`。

首期只实现 Admin 后台：

- Admin 登录
- `/admin/*` 路由
- Admin 菜单与布局
- Admin 业务页面
- Admin API 对接

Agent 后台暂不实现。后续用户确认后再补充 Agent 菜单、页面和 `/agent/api/v1/*` 对接。首期只保留不造成返工的角色扩展边界。

## 2. 技术栈

- Vite
- React
- TypeScript
- Semi Design
- React Router
- TanStack React Query
- Vitest

界面语言使用中文。视觉风格为专业金融后台：高密度表格、卡片统计、明确状态色、适配交易所运营场景。

## 3. 目录结构

```text
web/src/
  app/
    router.tsx
    providers.tsx
  auth/
    LoginPage.tsx
    authStore.ts
    RequireAdmin.tsx
  layouts/
    AdminLayout.tsx
    PageHeader.tsx
  admin/
    dashboard/
    users/
    agents/
    wallet/
    spot/
    newCoins/
    market/
    convert/
    secondsContract/
    margin/
    earn/
    risk/
    audit/
  api/
    client.ts
    adminAuth.ts
    adminNewCoins.ts
    adminMarket.ts
    adminConvert.ts
    adminAgents.ts
    adminSpot.ts
    adminSecondsContract.ts
    adminMargin.ts
    adminEarn.ts
    adminAudit.ts
    types.ts
  shared/
    DataTable.tsx
    FilterBar.tsx
    StatusTag.tsx
    AmountText.tsx
    TimestampText.tsx
    ConfirmAction.tsx
    JsonDrawer.tsx
```

## 4. 路由与权限

首期路由：

```text
/login
/admin/dashboard
/admin/users
/admin/agents
/admin/agent-commissions
/admin/wallet/accounts
/admin/wallet/ledger
/admin/spot/orders
/admin/spot/trades
/admin/new-coins/projects
/admin/new-coins/subscriptions
/admin/new-coins/distributions
/admin/new-coins/purchases
/admin/new-coins/lock-positions
/admin/new-coins/unlocks
/admin/market/pairs
/admin/market/strategies
/admin/convert/pairs
/admin/convert/rules
/admin/convert/orders
/admin/seconds-contract/products
/admin/seconds-contract/orders
/admin/margin/products
/admin/margin/positions
/admin/margin/liquidations
/admin/margin/interest
/admin/earn/products
/admin/earn/subscriptions
/admin/risk
/admin/audit-logs
/403
/404
```

登录页首期启用 Admin 登录。Agent 身份入口可保留为不可用状态，文案为“暂未开放”。

Admin 登录调用：

```text
POST /admin/api/v1/auth/login
```

登录成功后保存：

- access token
- refresh token
- scope
- 账号标识

`RequireAdmin` 只允许 `scope = admin` 访问 `/admin/*`。未登录跳转 `/login`，非 admin scope 跳转 `/403`。

## 5. 请求层

`api/client.ts` 统一处理：

- `VITE_API_BASE_URL`
- `Authorization: Bearer <token>`
- JSON 请求与响应
- 401 清理会话并跳转登录
- 403 跳转无权限页
- 5xx 转换为页面错误状态
- 请求取消与 loading 状态

所有外部时间字段按 Unix milliseconds number 处理，不解析本地化时间字符串。金额字段按后端 Decimal 字符串展示，前端不使用浮点数做资产计算。

## 6. Admin 菜单

```text
Admin 后台
├─ 仪表盘
├─ 用户与代理
│  ├─ 用户管理
│  ├─ 代理管理
│  ├─ 代理返佣
├─ 资产与交易
│  ├─ 钱包账户
│  ├─ 资产流水
│  ├─ 现货订单
│  └─ 现货成交
├─ 新币管理
│  ├─ 新币项目
│  ├─ 申购记录
│  ├─ 派发记录
│  ├─ 上市后认购
│  ├─ 锁定仓位
│  └─ 解禁记录
├─ 行情管理
│  ├─ 交易对
│  └─ 行情策略
├─ 闪兑管理
│  ├─ 闪兑币对
│  ├─ 新币汇率规则
│  └─ 闪兑订单
├─ 秒合约
│  ├─ 产品配置
│  └─ 订单记录
├─ 杠杆
│  ├─ 产品配置
│  ├─ 仓位记录
│  ├─ 强平记录
│  └─ 利息记录
├─ 理财 Earn
│  ├─ 产品配置
│  └─ 申购记录
├─ 风控与审计
│  ├─ 风控概览
│  └─ 审计日志
└─ 系统
   └─ 当前账号
```

## 7. 页面设计

### 7.1 仪表盘

展示运营入口和基础状态卡片：

- 今日注册用户
- 今日交易订单数
- 今日闪兑订单数
- 待解禁数量
- 风险事件数
- Worker / RabbitMQ 状态空态卡片

如果没有聚合接口，页面展示空态或从已有列表接口做基础汇总，不生成假数据。

### 7.2 用户与代理

用户管理读取用户列表相关接口。代理管理覆盖：

- 创建代理
- 启用/禁用代理
- 查看代理团队用户
- 调整用户代理归属
- 查看代理返佣
- 更新返佣状态

高风险操作必须使用二次确认。

### 7.3 资产与交易

资产页展示钱包账户、资产流水。现货页展示后台订单和成交：

```text
GET /admin/api/v1/spot/orders
GET /admin/api/v1/spot/trades
POST /admin/api/v1/spot/fills
```

成交填充属于高风险操作，必须二次确认并展示价格、数量、买卖双方订单。

### 7.4 新币管理

覆盖：

```text
GET /admin/api/v1/new-coins
POST /admin/api/v1/new-coins
PATCH /admin/api/v1/new-coins/{id}/lifecycle
POST /admin/api/v1/new-coins/{id}/distribute
PATCH /admin/api/v1/new-coins/{id}/unlock-rule
PATCH /admin/api/v1/new-coins/{id}/post-listing-purchase
PATCH /admin/api/v1/new-coins/{id}/unlock-fee-rule
GET /admin/api/v1/new-coins/{id}/subscriptions
GET /admin/api/v1/new-coins/{id}/distributions
GET /admin/api/v1/new-coins/purchases
GET /admin/api/v1/new-coins/lock-positions
GET /admin/api/v1/new-coins/unlocks
```

页面按项目详情组织生命周期、解禁规则、认购配置、矿工费配置和派发操作。固定时间解禁与相对周期解禁必须用不同说明展示，避免运营误解。

### 7.5 行情管理

交易对页展示市场列表。行情策略页覆盖：

```text
GET /admin/api/v1/market-strategies
POST /admin/api/v1/market-strategies
PATCH /admin/api/v1/market-strategies/{id}/status
```

页面必须提示：只有 internal / strategy 市场可由后台策略控制，external 外部行情不可人工控制。策略状态变更必须二次确认。

### 7.6 闪兑管理

覆盖：

```text
GET /admin/api/v1/convert/pairs
POST /admin/api/v1/convert/pairs
PATCH /admin/api/v1/convert/pairs/{id}
POST /admin/api/v1/convert/new-coin-rules
GET /admin/api/v1/convert/orders
```

配置页展示手续费、限额、状态。订单页按用户、状态、时间展示。

### 7.7 秒合约

覆盖产品配置和订单记录：

- 产品创建
- 产品启用/禁用
- 订单查询
- 管理员单笔结算

产品状态和单笔结算必须二次确认。

### 7.8 杠杆

覆盖：

- 产品配置
- 仓位记录
- 强平记录
- 利息记录
- 利息汇总

风险字段使用红/橙状态色突出展示。

### 7.9 理财 Earn

覆盖：

- 产品创建
- 产品启用/禁用
- 申购记录

产品状态变更必须二次确认。

### 7.10 审计日志

覆盖：

```text
GET /admin/api/v1/audit-logs
```

筛选项：

- admin_id
- action
- target_type
- target_id
- limit

表格字段：

- 操作人
- 操作类型
- 目标类型
- 目标 ID
- 操作原因
- IP
- 创建时间

`before_json` 和 `after_json` 使用抽屉展示。

## 8. 通用交互规范

所有列表页具备：

- 筛选区
- 表格
- loading 状态
- error 状态
- empty 状态
- 刷新按钮

高风险操作统一使用 `ConfirmAction`：

- 操作摘要
- 风险提示
- reason 输入框
- 二次确认按钮

状态标签统一由 `StatusTag` 映射，禁止页面自行硬编码颜色。

## 9. 错误处理

- 401：清理 token，跳转 `/login`
- 403：跳转 `/403`
- 400 / 409：表单或操作级错误提示
- 500：页面级 Banner
- 网络失败：保留当前页面，显示重试按钮

## 10. 测试与验证

首期验证命令：

```bash
npm run typecheck
npm run lint
npm run test
npm run build
```

最小测试覆盖：

- auth store 保存、读取、清理 token
- RequireAdmin 权限守卫
- Unix milliseconds 时间格式化
- Decimal 字符串金额展示
- StatusTag 状态映射
- API client 401 / 403 / 5xx 行为
- LoginPage 表单提交
- AdminLayout 菜单渲染
- 审计日志 JSON 抽屉

## 11. Agent 延后边界

首期不实现 Agent 页面、菜单和 API。为了避免返工：

- auth store 保留 `scope` 字段
- 登录页身份选择保留 Agent 禁用态
- route guard 独立封装，后续可增加 `RequireAgent`
- API client 不绑定 Admin 前缀，具体模块自行决定 `/admin/api/v1` 或未来 `/agent/api/v1`

## 12. 验收标准

- `/web` 可独立安装、启动、构建。
- Admin 可登录并进入后台布局。
- Admin 菜单覆盖本设计列出的模块。
- 主要列表页使用真实后端 API，不使用假数据冒充真实结果。
- 所有时间展示来自 Unix milliseconds。
- 所有资产金额按 Decimal 字符串展示，不做浮点资产计算。
- Agent 后台不会在首期暴露。
