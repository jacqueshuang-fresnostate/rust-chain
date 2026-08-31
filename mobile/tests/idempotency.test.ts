import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

import { canonicalRequestIntent, RetryStableIdempotencyKeys } from '../src/api/idempotency.ts'

test('mobile financial retries retain a key until the canonical intent succeeds', () => {
  let sequence = 0
  const keys = new RetryStableIdempotencyKeys('mobile-test', (prefix) => `${prefix}-${++sequence}`)
  const intent = canonicalRequestIntent({ pair_id: 'BTC-USDT', price: '10', quantity: '2', side: 'buy' })
  const reordered = canonicalRequestIntent({ side: 'buy', quantity: '2', price: '10', pair_id: 'BTC-USDT' })

  const firstKey = keys.acquire(intent)
  assert.equal(keys.acquire(reordered), firstKey)
  keys.complete(intent, 'stale-key')
  assert.equal(keys.acquire(intent), firstKey)
  keys.complete(intent, firstKey)
  assert.notEqual(keys.acquire(intent), firstKey)
})

test('mobile spot and margin transfer adapters retain acquired keys across rejected requests', () => {
  const trading = readFileSync(new URL('../src/api/trading.ts', import.meta.url), 'utf8')
  const wallet = readFileSync(new URL('../src/api/wallet.ts', import.meta.url), 'utf8')

  assert.match(
    trading,
    /const idempotencyKey = spotOrderIdempotencyKeys\.acquire\(intent\)[\s\S]*?await client\.post[\s\S]*?spotOrderIdempotencyKeys\.complete\(intent, idempotencyKey\)/,
  )
  assert.match(
    wallet,
    /const idempotencyKey = walletTransferIdempotencyKeys\.acquire\(intent\)[\s\S]*?await client\.post[\s\S]*?walletTransferIdempotencyKeys\.complete\(intent, idempotencyKey\)/,
  )
})
