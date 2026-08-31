import assert from 'node:assert/strict'
import test from 'node:test'
import {
  createMemoryRequestRegistry,
  createReferenceRequestKey,
} from '../src/api/requestCache.ts'

test('TTL 从成功完成时起算，命中返回隔离副本并支持 force/invalidate', async () => {
  let now = 0
  let calls = 0
  const registry = createMemoryRequestRegistry(() => now)
  const loader = async () => {
    calls += 1
    now = 100
    return [{ id: 1, nested: { enabled: true } }]
  }

  const first = await registry.request('countries', 50, loader)
  first[0].nested.enabled = false
  now = 149
  const cached = await registry.request('countries', 50, loader)
  assert.equal(calls, 1)
  assert.equal(cached[0].nested.enabled, true)

  await registry.request('countries', 50, loader, { force: true })
  assert.equal(calls, 2)
  registry.invalidate('countries')
  await registry.request('countries', 50, loader)
  assert.equal(calls, 3)
})

test('相同 key 并发复用 in-flight，失败不缓存且过期后重取', async () => {
  let now = 0
  let calls = 0
  let resolveRequest: ((value: string[]) => void) | undefined
  const registry = createMemoryRequestRegistry(() => now)
  const pending = () => {
    calls += 1
    return new Promise<string[]>((resolve) => { resolveRequest = resolve })
  }
  const left = registry.request('pairs', 10, pending)
  const right = registry.request('pairs', 10, pending)
  assert.equal(calls, 1)
  resolveRequest?.(['BTC/USDT'])
  assert.deepEqual(await left, ['BTC/USDT'])
  assert.deepEqual(await right, ['BTC/USDT'])

  now = 11
  await registry.request('pairs', 10, async () => { calls += 1; return ['ETH/USDT'] })
  assert.equal(calls, 2)

  let failures = 0
  await assert.rejects(registry.request('error', 10, async () => { failures += 1; throw new Error('boom') }))
  await assert.rejects(registry.request('error', 10, async () => { failures += 1; throw new Error('boom') }))
  assert.equal(failures, 2)
})

test('规范化参数键隔离不同参数并消除对象键顺序差异', () => {
  assert.equal(
    createReferenceRequestKey('/products', { limit: 50, locale: 'zh-CN' }),
    createReferenceRequestKey('/products', { locale: 'zh-CN', limit: 50 }),
  )
  assert.notEqual(
    createReferenceRequestKey('/products', { limit: 50 }),
    createReferenceRequestKey('/products', { limit: 100 }),
  )
  assert.notEqual(
    createReferenceRequestKey('/networks', { asset_symbol: 'BTC' }, 'wallet:a'),
    createReferenceRequestKey('/networks', { asset_symbol: 'ETH' }, 'wallet:a'),
  )
})

test('key 与全局 invalidate 都阻止旧 in-flight 在完成后回填缓存', async () => {
  const registry = createMemoryRequestRegistry(() => 0)
  let resolveKeyOld: ((value: string) => void) | undefined
  const keyOld = registry.request('key', 100, () => new Promise<string>((resolve) => { resolveKeyOld = resolve }))
  registry.invalidate('key')
  assert.equal(await registry.request('key', 100, async () => 'key-new'), 'key-new')
  resolveKeyOld?.('key-old')
  assert.equal(await keyOld, 'key-old')
  assert.equal(await registry.request('key', 100, async () => 'unexpected'), 'key-new')

  let resolveGlobalOld: ((value: string) => void) | undefined
  const globalOld = registry.request('global', 100, () => new Promise<string>((resolve) => { resolveGlobalOld = resolve }))
  registry.invalidate()
  assert.equal(await registry.request('global', 100, async () => 'global-new'), 'global-new')
  resolveGlobalOld?.('global-old')
  assert.equal(await globalOld, 'global-old')
  assert.equal(await registry.request('global', 100, async () => 'unexpected'), 'global-new')
})
