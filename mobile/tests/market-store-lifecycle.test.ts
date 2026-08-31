import assert from 'node:assert/strict'
import test from 'node:test'
import {
  createSharedMarketLifecycle,
  type MarketLifecycleScheduler,
} from '../src/core/marketLifecycle.ts'

class FakeScheduler implements MarketLifecycleScheduler {
  readonly timers = new Map<object, { callback: () => void; delay: number }>()

  setTimeout(callback: () => void, delay: number): object {
    const handle = {}
    this.timers.set(handle, { callback, delay })
    return handle
  }

  clearTimeout(handle: unknown): void {
    this.timers.delete(handle as object)
  }

  run(delay: number): void {
    const timer = [...this.timers.entries()].find(([, value]) => value.delay === delay)
    assert.ok(timer, `expected a pending ${delay}ms timer`)
    this.timers.delete(timer[0])
    timer[1].callback()
  }
}

test('deferred A -> B cold start joins one refresh and starts one idempotent live lease', async () => {
  const initial = deferred<void>()
  let hasData = false
  let loadCalls = 0
  let connectCalls = 0
  let stopCalls = 0
  let frame: (() => void) | undefined
  const lifecycle = createSharedMarketLifecycle({
    load: async () => {
      loadCalls += 1
      await initial.promise
      hasData = true
    },
    hasData: () => hasData,
    liveKey: () => hasData ? 'BTCUSDT|ETHUSDT' : '',
    connect: (onFrame) => {
      connectCalls += 1
      frame = onFrame
      return () => { stopCalls += 1 }
    },
    now: () => 1_000,
    isOnline: () => true,
  })

  const routeA = lifecycle.refresh()
  const routeB = lifecycle.refresh()
  assert.strictEqual(routeB, routeA, 'concurrent refresh callers must receive the exact same Promise')
  assert.equal(loadCalls, 1)

  // Route A leaves before its continuation. Route B already owns the desired
  // lease while the shared REST request is still pending.
  lifecycle.acquire('route-b')
  lifecycle.acquire('route-b')
  assert.equal(connectCalls, 0)
  initial.resolve()
  await routeB

  assert.equal(connectCalls, 1)
  assert.equal(lifecycle.snapshot().connection, 'connecting')
  lifecycle.ensureLive()
  assert.equal(connectCalls, 1, 'ensureLive must be idempotent for the same symbol set')

  frame?.()
  assert.equal(lifecycle.snapshot().connection, 'live')
  assert.equal(lifecycle.snapshot().lastFrameAt, 1_000)

  lifecycle.release('route-b')
  lifecycle.release('route-b')
  assert.equal(stopCalls, 1)
  assert.equal(lifecycle.snapshot().connection, 'idle')
})

test('freshness becomes stale, offline tears down transport, and online reconnects the lease', () => {
  const scheduler = new FakeScheduler()
  let now = 5_000
  let connectCalls = 0
  let stopCalls = 0
  let frame: (() => void) | undefined
  const lifecycle = createSharedMarketLifecycle({
    load: async () => undefined,
    hasData: () => true,
    liveKey: () => 'BTCUSDT',
    connect: (onFrame) => {
      connectCalls += 1
      frame = onFrame
      return () => { stopCalls += 1 }
    },
    scheduler,
    staleAfterMs: 65,
    now: () => now,
    isOnline: () => true,
  })

  lifecycle.acquire('markets')
  assert.equal(lifecycle.snapshot().connection, 'connecting')
  frame?.()
  assert.equal(lifecycle.snapshot().connection, 'live')

  now += 65
  scheduler.run(65)
  assert.equal(lifecycle.snapshot().connection, 'stale')

  lifecycle.setOnline(false)
  assert.equal(lifecycle.snapshot().connection, 'offline')
  assert.equal(stopCalls, 1)

  lifecycle.setOnline(true)
  assert.equal(lifecycle.snapshot().connection, 'connecting')
  assert.equal(connectCalls, 2)
  lifecycle.release('markets')
  assert.equal(stopCalls, 2)
  assert.equal(scheduler.timers.size, 0)
})

test('overlapping route consumers release only their own share of the live lease', () => {
  let connects = 0
  let disconnects = 0
  const lifecycle = createSharedMarketLifecycle({
    load: async () => undefined,
    hasData: () => true,
    liveKey: () => 'BTCUSDT',
    connect: () => {
      connects += 1
      return () => { disconnects += 1 }
    },
  })

  lifecycle.acquire('route-a')
  lifecycle.acquire('route-b')
  assert.equal(connects, 1)
  assert.equal(lifecycle.snapshot().consumerCount, 2)

  lifecycle.release('route-a')
  assert.equal(disconnects, 0)
  assert.equal(lifecycle.snapshot().consumerCount, 1)

  lifecycle.release('route-b')
  assert.equal(disconnects, 1)
  assert.equal(lifecycle.snapshot().consumerCount, 0)
})

test('failed shared refresh remains retryable and a waiting consumer is connected after recovery', async () => {
  let attempts = 0
  let hasData = false
  let connectCalls = 0
  const lifecycle = createSharedMarketLifecycle({
    load: async () => {
      attempts += 1
      if (attempts === 1) throw new Error('temporary')
      hasData = true
    },
    hasData: () => hasData,
    liveKey: () => hasData ? 'BTCUSDT' : '',
    connect: () => {
      connectCalls += 1
      return () => undefined
    },
    isOnline: () => true,
  })

  lifecycle.acquire('home')
  await lifecycle.refresh()
  assert.equal(lifecycle.snapshot().refreshFailed, true)
  assert.equal(connectCalls, 0)

  await lifecycle.refresh(true)
  assert.equal(lifecycle.snapshot().refreshFailed, false)
  assert.equal(connectCalls, 1)
  lifecycle.release('home')
})

function deferred<T>(): {
  promise: Promise<T>
  resolve(value: T | PromiseLike<T>): void
} {
  let resolve!: (value: T | PromiseLike<T>) => void
  const promise = new Promise<T>((next) => { resolve = next })
  return { promise, resolve }
}
