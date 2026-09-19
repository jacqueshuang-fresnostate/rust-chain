// Local-only actual-app fixture. Every HTTP/WS operation is isolated from the backend.
import { client, persistAuthTokens } from '/src/api/client.ts'

const large = '9007199254740993.000000000000000001'
const atom = '0.000000000000000001'
const now = Date.now()
const target = new URLSearchParams(location.search).get('route') || '/swap'
window.__numericRequests = []
window.WebSocket = class extends EventTarget {
  readyState = 0
  send() {}
  close() { this.readyState = 3 }
}
const product = {
  id: 1, asset_id: 1, asset_symbol: 'USDT', name: 'Precision product', category: 'fixed',
  term_days: 7, apr_rate: atom, min_subscribe: atom, max_subscribe: large, status: 'active',
  redemption_fee_rate: atom, maturity_profit_fee_rate: atom, early_redeem_fee_rate: atom,
  early_redeem_fee_basis: 'principal', interest_rate: atom, loan_type: 'credit',
  min_amount: atom, max_amount: large, min_kyc_level: 0, interest_calculation_mode: 'full_term',
}
const pairs = [{
  id: 1, from_asset_id: 1, from_asset_symbol: 'USDT', to_asset_id: 2, to_asset_symbol: 'BTC',
  min_amount: atom, max_amount: large, fee_rate: atom,
}]
const wallet = { asset_id: 1, symbol: 'USDT', precision_scale: 18, available: large, frozen: atom, locked: atom }
client.defaults.adapter = async (config) => {
  const path = new URL(config.url, location.origin).pathname.replace(/^.*\/api\/(?:v1|user)/, '')
  window.__numericRequests.push({ path, method: config.method, data: config.data })
  let data = {}
  if (path.endsWith('/convert/pairs')) data = { pairs }
  else if (path.endsWith('/convert/orders')) data = { orders: [] }
  else if (path.endsWith('/convert/quote')) data = {
    quote_id: 'local-precision-quote', convert_pair_id: 1, from_amount: large,
    to_amount: atom, rate: atom, fee_amount: atom, expires_at: now + 3_600_000,
  }
  else if (path.endsWith('/earn/products') || path.endsWith('/loan/products')) data = { products: [product] }
  else if (path.endsWith('/earn/subscriptions')) data = { subscriptions: [{
    id: 1, product_id: 1, asset_id: 1, amount: large, apr_rate: atom, term_days: 7,
    status: 'active', subscribed_at: now, matures_at: now + 604_800_000,
  }] }
  else if (path.endsWith('/loan/orders')) data = { orders: [{
    id: 1, product_id: 1, product_name: 'Precision loan', loan_type: 'credit', asset_symbol: 'USDT',
    amount: large, interest_rate: atom, term_days: 7, collateral_amount: null, status: 'disbursed',
    interest_amount: atom, repayment_amount: large, due_at: now + 604_800_000, created_at: now,
  }] }
  else if (path.endsWith('/wallet/accounts')) data = { accounts: [wallet] }
  else if (path.endsWith('/margin/wallets')) data = { wallets: [], positions: [], cross_accounts: [] }
  else if (path.endsWith('/prediction/config')) data = { allowed_assets: [{ asset_id: 1, asset_symbol: 'USDT', max_payout_amount: large }] }
  else if (path.endsWith('/prediction/markets')) data = { markets: [{
    id: 1, title: 'Precision market', yes_price: '0.5', no_price: '0.5',
    display_status: 'open', settlement_status: 'pending', end_at: now + 3_600_000,
  }] }
  else if (path.endsWith('/prediction/orders')) data = { orders: [] }
  else if (path.endsWith('/prediction/quotes')) data = {
    quote_id: 'local-prediction', outcome: 'yes', asset_id: 1, asset_symbol: 'USDT',
    stake_amount: large, fee_amount: atom, shares: large, theoretical_payout: large, expires_at: now + 3_600_000,
  }
  else if (path.endsWith('/markets')) data = { markets: [] }
  else if (path.endsWith('/support/conversation')) data = { conversation: null }
  return { data, status: 200, statusText: 'OK', headers: {}, config }
}
persistAuthTokens('local-numeric-access', 'local-numeric-refresh')
history.replaceState(null, '', `/#${target}`)
if (target === '/numeric-margin-close') {
  const { createApp, h } = await import('vue')
  const { default: MarginCloseSheet } = await import('/src/components/MarginCloseSheet.vue')
  const { default: i18n } = await import('/src/i18n/index.ts')
  await import('/src/styles/base.css')
  createApp({
    render: () => h(MarginCloseSheet, {
      open: true, saving: false, symbol: 'BTC/USDT', direction: 'long', marginMode: 'isolated',
      leverage: 2, baseAsset: 'BTC', quoteAsset: 'USDT',
      markPrice: large, positionQuantity: large, estimatedPnl: atom,
    }),
  }).use(i18n).mount('#app')
} else {
  await import('/src/main.ts')
}
