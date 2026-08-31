import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

import { canonicalRequestIntent, RetryStableIdempotencyKeys } from '../src/api/idempotency.ts'

test('reuses one client key after failure and rotates only after success or intent change', () => {
  let sequence = 0
  const keys = new RetryStableIdempotencyKeys('pc-test', (prefix) => `${prefix}-${++sequence}`)
  const firstIntent = canonicalRequestIntent({ amount: '1.000', asset: ' USDT ', from: 'spot', to: 'margin' })
  const equivalentIntent = canonicalRequestIntent({ to: 'margin', from: 'spot', asset: 'USDT', amount: '1.000' })
  const changedIntent = canonicalRequestIntent({ amount: '2.000', asset: 'USDT', from: 'spot', to: 'margin' })

  const firstKey = keys.acquire(firstIntent)
  assert.equal(keys.acquire(equivalentIntent), firstKey)
  assert.notEqual(keys.acquire(changedIntent), firstKey)

  keys.complete(firstIntent, firstKey)
  assert.notEqual(keys.acquire(firstIntent), firstKey)
})

test('PC spot and margin transfer adapters complete a key only after the request succeeds', () => {
  const spot = readFileSync(new URL('../src/api/exchange.ts', import.meta.url), 'utf8')
  const margin = readFileSync(new URL('../src/api/contract.ts', import.meta.url), 'utf8')

  assert.match(
    spot,
    /const idempotencyKey = spotOrderIdempotencyKeys\.acquire\(intent\)[\s\S]*?await request\.instance\.post[\s\S]*?spotOrderIdempotencyKeys\.complete\(intent, idempotencyKey\)/,
  )
  assert.match(
    margin,
    /const idempotencyKey = marginTransferIdempotencyKeys\.acquire\(intent\)[\s\S]*?await request\.instance\.post[\s\S]*?marginTransferIdempotencyKeys\.complete\(intent, idempotencyKey\)/,
  )
})
