# HIPPO Mobile Pencil Screen Inventory

## Foundations

| Artboard | Purpose |
| --- | --- |
| `00 / Design System` | Tokens, typography, controls, fields, status roles, grid and navigation language |
| `01 / Home / Light · Guest` | Guest editorial Hero in light mode |
| `02 / Home / Dark · Guest` | Guest editorial Hero in dark mode |
| `03 / Home / Light · Member` | Authenticated live-asset semantics in light mode |
| `04 / Home / Dark · Member` | Authenticated live-asset semantics in dark mode |

## Discovery and trading

| Artboard | Production route | Primary intent |
| --- | --- | --- |
| `03 / Markets · Light` | `/markets` | Search and scan live markets in light mode |
| `08 / Markets · Dark` | `/markets` | Dark-mode parity for live markets |
| `04 / Market Detail · Light` | `/markets/:symbol` | Inspect live quote, local chart and paired market microstructure |
| `05 / Market Detail · Dark` | `/markets/:symbol` | Dark-mode parity for market detail |
| `06 / Spot Trading · Light` | `/trade/:symbol?` | Place a real spot order with live paired depth; independent from contract trading |
| `07 / Spot Trading · Dark` | `/trade/:symbol?` | Dark-mode parity for the same real-time independent spot workstation |
| `06 / Contract Trading` | `/trade/:symbol?mode=contract` | Place a real margin/contract order |
| `07 / Seconds Contract` | `/seconds` | Submit a duration-based seconds contract using spot wallet funds |
| `08 / Orders` | `/orders` | Review open positions, active and historical orders |

## Assets and profile

| Artboard | Production route | Primary intent |
| --- | --- | --- |
| `09 / Assets · Light · Guest` (`CUK3y`) | `/assets` | Guest：沉浸式卡片登录提示 + 登录按钮（无估值） |
| `09 / Assets · Dark · Guest` (`i6YDBr`) | `/assets` | Guest 暗色镜像 |
| `09 / Assets · Light · Member` (`p61z2Q`) | `/assets` | Member 空态：0.00 + 暂无持仓 |
| `09 / Assets · Dark · Member` (`Q4JYj`) | `/assets` | Member 持仓：每币种数量+估值 |
| `10 / Profile` | `/profile` | Identity, verification, security and preferences |
| `11 / Message Center` | `/messages` | Read account, funds, trading and announcement messages |
| `12 / News` | `/news` | Browse market information |
| `13 / News Detail` | `/news/:id` | Read one complete article |

## Products

| Artboard | Production route | Primary intent |
| --- | --- | --- |
| `14 / Product Hub` | `/products` | Enter the supported product domains |
| `15 / Swap` | `/swap` | Convert between supported assets |
| `16 / Earn` | `/products/earn` | Compare and subscribe to earn products |
| `17 / Loan` | `/products/loan` | Compare available loan products and manage debt |
| `18 / New Coins` | `/products/new-coins` | Browse active launch offerings |
| `19 / New Coin Detail` | `/products/new-coins/:symbol` | Review and subscribe to one offering |
| `20 / Prediction` | `/products/prediction` | Enter a real prediction market |

## Wallet flows

| Artboard | Production route | Primary intent |
| --- | --- | --- |
| `21 / Deposit Asset` | `/assets/deposit` | Select the asset to deposit |
| `22 / Deposit Network` | `/assets/deposit/:asset/networks` | Select a supported network |
| `23 / Deposit Detail` | `/assets/deposit/:asset/:network` | Copy/scan the assigned deposit address |
| `24 / Withdraw Asset` | `/assets/withdraw` | Select the asset to withdraw |
| `25 / Withdraw Form` | `/assets/withdraw/:asset` | Submit address, network and amount |
| `26 / Wallet Ledger` | `/assets/ledger` | Filter and inspect wallet movements |
| `27 / Withdrawal Records` | `/assets/withdrawals` | Track withdrawal processing |
| `28 / Quick Recharge` | `/assets/quick-recharge` | Buy/recharge USDT through configured channels |

## Authentication and account

| Artboard | Production route | Primary intent |
| --- | --- | --- |
| `29 / Login` | `/login` | Authenticate with a configured identity method |
| `30 / Register` | `/register` | Create an account and accept required terms |
| `31 / Two-Factor` | `/login/two-factor` | Complete a login challenge |
| `32 / Forgot Password` | `/forgot-password` | Reset account access safely |
| `33 / Security` | `/profile/security` | Configure login password, funds password and TOTP |
| `34 / KYC` | `/profile/kyc` | Submit identity verification |
| `35 / Account Bindings` | `/profile/bindings` | Manage email, phone and supported third-party bindings |
| `36 / Referrals` | `/profile/referrals` | Share referral identity and inspect rewards |
| `37 / Language` | `/profile/language` | Select a supported locale |

## Required state variants

Every implementation mapped from these artboards must cover loading, empty, failed,
guest, authenticated, focused, invalid, disabled, submitting and confirmed states where
the underlying workflow supports them. Missing backend data must never be replaced with
fabricated balances, returns, products, limits or market statistics.

## Gaps filled 2026-08-06

| Artboard | Production route | Primary intent |
| --- | --- | --- |
| `38 / New Coin Records · Light/Dark` (`A9It6g`/`h4gfd`) | `/products/new-coins/records` | 认购/分发/申购/解锁记录 |
| `39 / Transfer Sheet · Light/Dark` (`v6phV`/`TuWXq`) | `/assets` modal | 现货↔杠杆划转：数量英雄 + 玻璃路径 + 持仓行资产 |
| `39b / Transfer · Asset Picker · Light/Dark` (`tPkL1`/`tPkD1`) | `/assets` transfer modal | 划转「选择资产」二级 Sheet：搜索 + 持仓行列表 |
| `40 / Help & Support · Light/Dark` (`UouET`/`FM5tp`) | Proposed from `/profile` help entry | 常见问题与客服入口；生产路由尚未注册 |
| `07c / Seconds · Pair Picker · Light/Dark` (`vONcc`/`kLXCs`) | `/seconds` | 点击 header 交易对弹出底部选择器（搜索 + 收益/最新价列表） |
| `08c / Orders · Empty · Light/Dark` (`e5Qs1`/`hxe8l`) | `/orders` | 订单空态 |
| `26b / Wallet Ledger · Empty · Light/Dark` (`Bcug6`/`IVMAO`) | `/assets/ledger` | 资金账单空态 |
| `11b / Message Center · Empty · Light/Dark` (`t7j6n`/`eSMHf`) | `/messages` | 消息中心空态 |
| `20b / Prediction · Bet · Light/Dark` (`CzpTv`/`ZvGMv`) | `/products/prediction` | 预测下单确认 |
| `16b / Earn · Subscribe · Light/Dark` (`nqP6W`/`aXxul`) | `/products/earn` | 理财申购确认 |
