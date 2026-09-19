import assert from 'node:assert/strict'
import test from 'node:test'
import { readFileSync } from 'node:fs'
import { mapSecondsOrdersToPcOrders } from '../src/api/backendAdapters.ts'

test('seconds refund remains a distinct terminal status without a fabricated outcome', () => {
  const rows = mapSecondsOrdersToPcOrders({ orders: [{
    id: 1, user_id: 2, product_id: 3, pair_id: 4, stake_asset: 5, symbol: 'BTC-USDT',
    direction: 'up', stake_amount: '10', payout_rate: '0.8', entry_price: '100',
    settlement_price: null, status: 'refunded', result: null,
    idempotency_key: 'original', expires_at: 1000, created_at: 500,
  }] }).data
  assert.equal(rows[0].status, 'REFUNDED')
  assert.notEqual(rows[0].result, 'WIN')
  assert.notEqual(rows[0].result, 'LOSE')
  assert.equal(rows[0].betAmount, 10)
  const view = readFileSync(new URL('../src/views/SecondOptions.vue', import.meta.url), 'utf8')
  assert.match(view, /order\.status === 'REFUNDED' \|\| order\.status === 'MANUAL_REVIEW'">--<\/template>/)
})

test('wallet principal refund is classified without treating it as a win', () => {
  const api = readFileSync(new URL('../src/api/transaction.ts', import.meta.url), 'utf8')
  const locales = readFileSync(new URL('../src/i18n/index.ts', import.meta.url), 'utf8')
  assert.match(api, /'seconds_contract_principal_refund'/)
  assert.match(locales, /type_seconds_contract_principal_refund: 'Seconds Contract Principal Refund'/)
  assert.match(locales, /type_seconds_contract_principal_refund: '秒合约本金退还'/)
})
