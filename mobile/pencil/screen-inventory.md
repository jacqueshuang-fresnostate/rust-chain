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
| `09 / Assets · Light` (`CUK3y`) | `/assets` | Guest：登录引导、资金操作入口与工具列表 |
| `09 / Assets · Dark` (`i6YDBr`) | `/assets` | Guest 暗色镜像 |
| `09 / Assets · Light · Member` (`p61z2Q`) | `/assets` | Member：总估值、我的持仓（每币种数量+估值）、空持仓态、资金工具 |
| `09 / Assets · Dark · Member` (`Q4JYj`) | `/assets` | Member 暗色镜像 |
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
