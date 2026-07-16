# Mobile User API Inventory

## Included contract groups

| Mobile area | Existing contract family | Required mobile workflow |
| --- | --- | --- |
| Authentication | `/auth/*`, `/countries` | Login, email-code registration, password reset, 2FA follow-up states |
| Public market | `/markets/*`, `/news`, `/platform/brand` | Market list, ticker, K-line, depth, trades, news list/detail |
| Spot trading | `/spot/orders`, `/spot/trades` | Create order, list current/history orders, cancel order |
| Margin trading | `/margin/*` | Products, wallets, transfer, positions, close/cancel, batch actions, leverage and margin mode |
| Wallet | `/wallet/*`, `/margin/transfers` | Account list, ledger, deposit assets/networks/address, withdrawal assets/request, quick recharge, transfer |
| Convert | `/convert/*` | Pair list, balance, quote, confirm, history |
| User profile | `/user/*` | KYC (personal/enterprise), security/2FA/password, profile, referrals/invites |
| Product modules | `/seconds-contracts/*`, `/new-coins/*`, `/earn/*`, `/loans/*`, `/prediction/*` | Browse, quote or subscribe/order, and view/manage user records |

## Explicitly excluded

* `/admin/*`, staff operations, audit views, and agent-management endpoints.
* Server-side source-of-truth calculations. The mobile client renders and submits inputs; the backend remains responsible for balances, fees, risk controls, order state, and settlement.

## Completion rule

Each mobile workflow must have a route, a typed API adapter, and a request-result state. Where a feature requires authentication, the route may render an explicit login gate but must not substitute a mock financial result.
