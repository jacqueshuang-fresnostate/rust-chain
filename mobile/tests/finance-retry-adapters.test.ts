import assert from 'node:assert/strict'
import test from 'node:test'
import { createServer } from 'vite'
import type { DecimalText } from '../src/core/decimal.ts'

test('earn and loan adapters replay committed-but-lost requests and isolate a new login', async (context) => {
  const values = new Map<string, string>()
  const storage = {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => { values.set(key, value) },
    removeItem: (key: string) => { values.delete(key) },
  }
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'sessionStorage')
  Object.defineProperty(globalThis, 'sessionStorage', { value: storage, configurable: true })
  context.after(() => {
    if (previous) Object.defineProperty(globalThis, 'sessionStorage', previous)
    else Reflect.deleteProperty(globalThis, 'sessionStorage')
  })
  const server = await createServer({ appType: 'custom', logLevel: 'silent', server: { middlewareMode: true } })
  context.after(async () => server.close())
  const clientModule = await server.ssrLoadModule('/src/api/client.ts')
  const earn = await server.ssrLoadModule('/src/api/earn.ts')
  const loan = await server.ssrLoadModule('/src/api/loan.ts')
  clientModule.persistAuthTokens('alice-access', 'alice-refresh')
  const committed = new Set<string>()
  const requests: Array<Record<string, unknown>> = []
  let loseResponse = true
  clientModule.client.post = async (_url: string, body: Record<string, unknown>) => {
    requests.push(body)
    committed.add(String(body.idempotency_key))
    if (loseResponse) throw new Error('response lost')
    return { data: {} }
  }
  const operations = [
    () => earn.subscribeEarnProduct(3, '10.00' as DecimalText),
    () => loan.applyLoan({ productId: 4, amount: '20.000' as DecimalText, collateralAssetId: 2, collateralAmount: '1.00' as DecimalText }),
  ]
  for (const operation of operations) {
    loseResponse = true
    const count = committed.size
    await assert.rejects(operation, /response lost/)
    const key = requests.at(-1)?.idempotency_key
    loseResponse = false
    await operation()
    assert.equal(requests.at(-1)?.idempotency_key, key)
    assert.equal(committed.size, count + 1)
    await operation()
    assert.notEqual(requests.at(-1)?.idempotency_key, key)
  }
  loseResponse = true
  await assert.rejects(operations[0]!, /response lost/)
  const aliceKey = requests.at(-1)?.idempotency_key
  clientModule.persistAuthTokens('bob-access', 'bob-refresh')
  await assert.rejects(operations[0]!, /response lost/)
  assert.notEqual(requests.at(-1)?.idempotency_key, aliceKey)
  assert.equal(requests.at(-1)?.amount, '10')
})
