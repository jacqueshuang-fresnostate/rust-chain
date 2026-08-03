# Pencil 当前选中但尚未映射的生产页面

## 来源

- 活动设计文件：`mobile/pencil/hippo-mobile-uiux.pen`
- 读取方式：Pencil MCP `get_app_state` + `execute`，未直接解析 `.pen` 文件。
- 基准尺寸：所有目标画板均为 390 × 920。
- 已完成生产映射：Home、Markets、Market Detail、Spot Trading。
- 本切片目标：Assets、Profile Guest/Member、Orders Spot/Leverage、Login、Register、News、News Detail、Swap/Asset Picker、Earn、Loan、New Coins、New Coin Detail 的浅色与深色版本。

## 画板 ID 映射

| 生产页面 | Light | Dark |
| --- | --- | --- |
| Assets | `CUK3y` | `i6YDBr` |
| Profile Guest | `dUqOS` | `duJTW` |
| Profile Member | `S23rM` | `S0Bj8` |
| Orders Spot | `kcP5D` | `A85if` |
| Orders Leverage | `n6oGO` | `t2GTW4` |
| Login | `u99Fpg` | `WNbsc` |
| Register | `MCuqb` | `RGYGj` |
| News | `VGPW0` | `b6EGF` |
| News Detail | `Q50Rgr` | `ASvmq` |
| Swap | `x9T4CL` | `eXdnN` |
| Swap Asset Picker | `sf288` | `xvVss` |
| Earn | `zIzOm` | `tCHZ9` |
| Loan | `kIOBX` | `yrsRy` |
| New Coins | `oOJ0q` | `ZTtvY` |
| New Coin Detail | `nFwYy` | `B6Qh9J` |

## 390px 几何与层级合同

- Assets：38px 安全区后 48px 页面 Header；157px 资产主舞台；80px 四项快捷操作；159px 资产分布诚实空态/真实态；207px 资金工具；84px 五入口 Dock。
- Profile：38px 安全区后 48px 页面 Header；72px 账户主舞台；访客 58px 登录/注册动作或会员 44px 状态行；身份安全与偏好支持使用无冗余大卡片的连续分组；会员态包含独立退出按钮；84px Dock。
- Orders：32px 安全区后 48px Header；45px 现货/杠杆一级标签；34px 当前委托/历史委托/持仓二级标签；64px 紧凑订单行；84px Dock。
- Login：40px 起 62px 品牌行；114px 起 88px 标题区；26px 认证方式；48px 完整字段；56px 薄荷主按钮；切换入口与安全说明。Register 使用同一品牌/标题语言，依次为国家、邮箱、密码、确认密码、邀请码、协议与 56px 主按钮。
- News：36px 起 48px Header；34px 分类标签；243px 首条主新闻；其后为紧凑连续新闻行。News Detail 使用 58px Header、标题/元信息、主视觉语义块、安全富文本和相关推荐。
- Swap：28px 起 58px Header；639px 主体；支付/获得字段、居中方向切换、汇率/费用/有效期、56px 主动作与最近记录。币种选择器为全屏遮罩 + y=280、h=640 的底部面板。
- Earn / Loan：28px 起 60px Header；主体从 y=88 开始。Earn 以收益/期限/起投/风险真实产品为主角；Loan 以可用额度/抵押率前置说明、认证前提和真实产品/诚实空态为主角。
- New Coins：28px 起 60px Header；项目列表主体 y=88。New Coin Detail 使用 60px Header，随后为项目身份、发行事实、三步流程和真实状态驱动的认购表单。

## 生产映射规则

1. Pencil 决定默认态的几何、排版、颜色层级、Lucide 图标与控件形态；Vue 现有 API、WebSocket、DTO、状态机、确认弹窗、路由和权限行为保持不变。
2. Pencil 演示数据不得写入生产代码。列表只显示真实返回项；缺失数据必须显示加载、错误、空态或登录提示。
3. Assets/Profile 不叠加 Root Header 或 SignalField；使用画板内页面 Header，并继续由全局 Shell 提供五入口 Dock。
4. Orders 使用一个真实页面承载现货/杠杆一级标签与当前/历史/持仓二级标签，切换不得丢失已有查询、取消、平仓与刷新行为。
5. 登录注册可重排为画板的一屏字段布局，但必须保留真实登录配置、验证码、邀请码、密码策略、安全重定向和二次验证流程。
6. News Detail 继续使用 `NewsRichText`，禁止不受控 `v-html`。
7. Swap 币种面板从真实可闪兑交易对和钱包余额生成；确认交易继续使用现有 quote/review/submit 接口和可访问对话框。
8. Earn、Loan、New Coins 与详情继续使用后端真实状态，不创建演示产品、额度、项目或收益率。
9. 所有新文案保持 zh-CN/en 键对称；主要触控目标不小于 44px；焦点环覆盖完整控件，不出现内层 input 的第二层矩形边框。
