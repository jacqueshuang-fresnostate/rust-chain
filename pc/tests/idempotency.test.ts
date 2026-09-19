import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

import { canonicalRequestIntent, RetryStableIdempotencyKeys } from '../src/api/idempotency.ts'
import { mapPcSpotOrderRequest } from '../src/api/backendAdapters.ts'

test('PC uncertain loan keys survive recreation and are scoped to the login session', () => {
  const values = new Map<string, string>()
  const storage = {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => { values.set(key, value) },
    removeItem: (key: string) => { values.delete(key) },
  }
  let sequence = 0
  const factory = () => `key-${++sequence}`
  const first = new RetryStableIdempotencyKeys('pc-loan', factory, () => storage)
  const intent = canonicalRequestIntent({ scope: 'alice', amount: '10' })
  const key = first.acquire(intent)
  const reloaded = new RetryStableIdempotencyKeys('pc-loan', factory, () => storage)
  assert.equal(reloaded.acquire(intent), key)
  assert.notEqual(reloaded.acquire(canonicalRequestIntent({ scope: 'bob', amount: '10' })), key)
  reloaded.complete(intent, key)
  const next = first.acquire(intent)
  assert.notEqual(next, key)
  assert.equal(reloaded.acquire(intent), next)
})

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

test('explicit spot trigger direction owns the retry identity and is never guessed', () => {
  let sequence = 0
  const keys = new RetryStableIdempotencyKeys('pc-trigger-test', () => `trigger-${++sequence}`)
  const order = { symbol: 'BTC/USDT', direction: 'BUY' as const, type: 'STOP_LIMIT' as const,
    triggerPrice: 10, price: 8, amount: 2 }
  assert.throws(() => mapPcSpotOrderRequest(order, ''), /trigger direction/)
  const rising = canonicalRequestIntent(mapPcSpotOrderRequest({ ...order, triggerDirection: 'rising' }, ''))
  const falling = canonicalRequestIntent(mapPcSpotOrderRequest({ ...order, triggerDirection: 'falling' }, ''))
  const first = keys.acquire(rising)
  assert.equal(keys.acquire(rising), first)
  assert.notEqual(keys.acquire(falling), first)
})
