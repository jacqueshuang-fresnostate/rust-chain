import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

import { canonicalRequestIntent, RetryStableIdempotencyKeys } from '../src/api/idempotency.ts'

test('pending financial identity survives reload, isolates sessions, and retains uncertain requests', async () => {
  const values = new Map<string, string>()
  const storage = {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => { values.set(key, value) },
    removeItem: (key: string) => { values.delete(key) },
  }
  let sequence = 0
  const factory = () => `key-${++sequence}`
  const first = new RetryStableIdempotencyKeys('loan', factory, () => storage)
  const intent = canonicalRequestIntent({ scope: 'alice', amount: '10', product: 1 })
  const key = first.acquire(intent)
  await assert.rejects(async () => { throw new Error('response lost after commit') })
  const reloaded = new RetryStableIdempotencyKeys('loan', factory, () => storage)
  assert.equal(reloaded.acquire(intent), key)
  assert.notEqual(reloaded.acquire(canonicalRequestIntent({ scope: 'bob', amount: '10', product: 1 })), key)
  assert.notEqual(reloaded.acquire(canonicalRequestIntent({ scope: 'alice', amount: '11', product: 1 })), key)
  reloaded.complete(intent, 'old-response')
  assert.equal(reloaded.acquire(intent), key)
  reloaded.complete(intent, key)
  const next = first.acquire(intent)
  assert.notEqual(next, key)
  assert.equal(reloaded.acquire(intent), next)
})

test('financial intent persistence failure prevents acquiring a sendable key', () => {
  const keys = new RetryStableIdempotencyKeys('loan', () => 'key', () => ({
    getItem: () => null,
    setItem: () => { throw new Error('storage full') },
    removeItem: () => {},
  }))
  assert.throws(() => keys.acquire('intent'), /storage full/)
})

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
