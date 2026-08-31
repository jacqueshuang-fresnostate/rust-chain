import assert from 'node:assert/strict'
import test from 'node:test'
import {
  normalizeRealizedReturnAssetSymbol,
  normalizeRealizedReturnAssetSymbols,
  normalizeRealizedReturnTimestamp,
  nullableRealizedReturnDecimal,
  requiredRealizedReturnDecimal,
} from '../src/core/realizedReturn.ts'
import { createSessionRequestLifecycle } from '../src/core/sessionRequest.ts'

test('session request generations discard superseded and cross-session completions', async () => {
  let sessionKey = 'SESSION_A'
  const first = deferred<string>()
  const second = deferred<string>()
  let calls = 0
  const lifecycle = createSessionRequestLifecycle({
    sessionKey: () => sessionKey,
    request: () => (++calls === 1 ? first.promise : second.promise),
  })

  const oldLoad = lifecycle.load()
  sessionKey = 'SESSION_B'
  const currentLoad = lifecycle.load()
  second.resolve('CURRENT')
  assert.deepEqual(await currentLoad, { state: 'loaded', value: 'CURRENT' })
  first.resolve('STALE')
  assert.deepEqual(await oldLoad, { state: 'stale' })
  assert.equal(calls, 2)
})

test('session request lifecycle treats guest, invalidation, stop, and current errors distinctly', async () => {
  let sessionKey = ''
  let requestCalls = 0
  const pending = deferred<string>()
  const lifecycle = createSessionRequestLifecycle({
    sessionKey: () => sessionKey,
    request: () => {
      requestCalls += 1
      return pending.promise
    },
  })

  assert.deepEqual(await lifecycle.load(), { state: 'guest' })
  assert.equal(requestCalls, 0)

  sessionKey = 'SESSION_A'
  const invalidated = lifecycle.load()
  lifecycle.invalidate()
  pending.resolve('LATE')
  assert.deepEqual(await invalidated, { state: 'stale' })

  const failed = createSessionRequestLifecycle({
    sessionKey: () => 'SESSION_A',
    request: async () => { throw new Error('fixture failure') },
  })
  const result = await failed.load()
  assert.equal(result.state, 'error')
  if (result.state === 'error') assert.match(String(result.error), /fixture failure/)

  failed.stop()
  assert.deepEqual(await failed.load(), { state: 'stale' })
})

test('realized-return primitives preserve Decimal text, timestamp units, and normalized asset identity', () => {
  assert.equal(
    requiredRealizedReturnDecimal('9007199254740993.000000000000000001', 'amount', 'today return'),
    '9007199254740993.000000000000000001',
  )
  assert.equal(nullableRealizedReturnDecimal(null, 'rate', 'today return'), null)
  assert.equal(normalizeRealizedReturnTimestamp(1_786_307_400, 'created_at', 'history'), 1_786_307_400_000)
  assert.equal(normalizeRealizedReturnTimestamp('1786307400000', 'created_at', 'history'), 1_786_307_400_000)
  assert.equal(normalizeRealizedReturnAssetSymbol(' usdt ', 'asset', 'history'), 'USDT')
  assert.deepEqual(
    normalizeRealizedReturnAssetSymbols(['btc', 'BTC', 'eth'], 'assets', 'history'),
    ['BTC', 'ETH'],
  )

  assert.throws(
    () => requiredRealizedReturnDecimal('1e-18', 'amount', 'today return'),
    /invalid today return amount/,
  )
  assert.throws(
    () => normalizeRealizedReturnTimestamp(0, 'created_at', 'history'),
    /invalid history created_at/,
  )
  assert.throws(
    () => normalizeRealizedReturnAssetSymbol('BTC/USDT', 'asset', 'history'),
    /invalid history asset/,
  )
})

function deferred<T>(): {
  promise: Promise<T>
  resolve: (value: T) => void
} {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((done) => { resolve = done })
  return { promise, resolve }
}
