import assert from 'node:assert/strict'
import test from 'node:test'
import {
  createSessionOwner,
  type PersistedSessionEnvelope,
  type SessionPersistence,
  type SessionSyncTransport,
} from '../src/core/sessionOwner.ts'
import { createMemoryRequestRegistry } from '../src/api/requestCache.ts'

class SharedPersistence implements SessionPersistence {
  envelope: PersistedSessionEnvelope | null = null
  failWrites = false

  read(): PersistedSessionEnvelope | null {
    return this.envelope
  }

  write(envelope: PersistedSessionEnvelope): boolean {
    if (this.failWrites) return false
    this.envelope = structuredClone(envelope)
    return true
  }
}

class SessionBus {
  private readonly listeners = new Set<(envelope: PersistedSessionEnvelope) => void>()

  transport(): SessionSyncTransport {
    return {
      publish: (envelope) => {
        for (const listener of [...this.listeners]) listener(structuredClone(envelope))
      },
      subscribe: (listener) => {
        this.listeners.add(listener)
        return () => { this.listeners.delete(listener) }
      },
    }
  }
}

test('logout wins over a deferred refresh CAS and the stale result cannot restore tokens', async () => {
  let now = 1_000
  let id = 0
  const owner = createSessionOwner({
    now: () => now,
    createId: () => `id-${++id}`,
  })
  owner.replace({ accessToken: 'ACCESS_A', refreshToken: 'REFRESH_A' })
  const lease = owner.capture()
  const refresh = deferred<{ accessToken: string; refreshToken: string }>()
  const commit = refresh.promise.then((tokens) => owner.commitRefresh(lease, tokens))

  now += 1
  owner.clear('logout')
  refresh.resolve({ accessToken: 'ACCESS_A2', refreshToken: 'REFRESH_A2' })

  assert.equal(await commit, null)
  assert.equal(owner.snapshot().accessToken, '')
  assert.equal(owner.snapshot().refreshToken, '')
  assert.equal(owner.isCurrent(lease), false)
})

test('logout then login B rejects the old A refresh while rotating identity scope', async () => {
  let id = 0
  const owner = createSessionOwner({ createId: () => `id-${++id}` })
  owner.replace({ accessToken: 'ACCESS_A', refreshToken: 'REFRESH_A' })
  const sessionA = owner.capture()
  const scopeA = sessionA.scope

  owner.clear('logout')
  owner.replace({ accessToken: 'ACCESS_B', refreshToken: 'REFRESH_B' })
  const sessionB = owner.capture()
  assert.notEqual(sessionB.scope, scopeA)

  assert.equal(owner.commitRefresh(sessionA, {
    accessToken: 'ACCESS_A_LATE',
    refreshToken: 'REFRESH_A_LATE',
  }), null)
  assert.equal(owner.snapshot().accessToken, 'ACCESS_B')
  assert.equal(owner.snapshot().scope, sessionB.scope)

  const refreshedB = owner.commitRefresh(sessionB, {
    accessToken: 'ACCESS_B2',
    refreshToken: 'REFRESH_B2',
  })
  assert.ok(refreshedB)
  assert.equal(refreshedB.scope, sessionB.scope, 'refresh keeps identity scope')
  assert.ok(refreshedB.epoch > sessionB.epoch, 'refresh advances request generation')
})

test('a persisted cross-container logout tombstone beats refresh even before its event arrives', () => {
  const persistence = new SharedPersistence()
  const first = createSessionOwner({ persistence, createId: incrementingId('first') })
  first.replace({ accessToken: 'ACCESS_A', refreshToken: 'REFRESH_A' })

  // The second owner captures A but deliberately has no sync transport.
  const second = createSessionOwner({ persistence, createId: incrementingId('second') })
  const staleRefreshLease = second.capture()
  first.clear('logout')

  assert.equal(second.commitRefresh(staleRefreshLease, {
    accessToken: 'ACCESS_A_LATE',
    refreshToken: 'REFRESH_A_LATE',
  }), null)
  assert.equal(second.snapshot().accessToken, '')
})

test('cross-tab/container transport propagates login and logout and runs invalidation hooks once', () => {
  let id = 0
  let now = 10_000
  const persistence = new SharedPersistence()
  const bus = new SessionBus()
  const first = createSessionOwner({
    persistence,
    transport: bus.transport(),
    createId: () => `first-${++id}`,
    now: () => now,
  })
  const second = createSessionOwner({
    persistence,
    transport: bus.transport(),
    createId: () => `second-${++id}`,
    now: () => now,
  })
  const transitions: string[] = []
  let privateCacheInvalidations = 0
  let privateSocketInvalidations = 0
  second.subscribe((transition) => {
    transitions.push(`${transition.external}:${transition.reason}:${transition.current.accessToken || 'guest'}`)
    if (transition.previous.scope !== transition.current.scope) {
      privateCacheInvalidations += 1
      privateSocketInvalidations += 1
    }
  })
  first.startSynchronization()
  second.startSynchronization()

  first.replace({ accessToken: 'ACCESS_A', refreshToken: 'REFRESH_A' })
  assert.equal(second.snapshot().accessToken, 'ACCESS_A')
  const secondScope = second.snapshot().scope

  now += 1
  first.clear('logout')
  assert.equal(second.snapshot().accessToken, '')
  assert.equal(second.snapshot().scope, '')
  assert.deepEqual(transitions, [
    'true:external:ACCESS_A',
    'true:external:guest',
  ])
  assert.equal(privateCacheInvalidations, 2)
  assert.equal(privateSocketInvalidations, 2)
  assert.ok(secondScope)

  first.stopSynchronization()
  second.stopSynchronization()
})

test('session invalidation prevents a deferred private cache load from writing back', async () => {
  const owner = createSessionOwner({ createId: incrementingId('cache') })
  const registry = createMemoryRequestRegistry(() => 100)
  owner.subscribe(() => registry.invalidate())
  owner.replace({ accessToken: 'ACCESS_A', refreshToken: 'REFRESH_A' })

  const stale = deferred<string>()
  const first = registry.request('private:wallet', 1_000, () => stale.promise)
  owner.clear('logout')
  stale.resolve('wallet-a')
  assert.equal(await first, 'wallet-a', 'the original caller may finish its own local work')

  let freshLoads = 0
  const fresh = await registry.request('private:wallet', 1_000, async () => {
    freshLoads += 1
    return 'guest-safe'
  })
  assert.equal(fresh, 'guest-safe')
  assert.equal(freshLoads, 1, 'the stale value must not have repopulated the invalidated cache')
})

test('restricted storage keeps the in-memory session coherent and reports memory persistence', () => {
  const persistence = new SharedPersistence()
  persistence.failWrites = true
  const owner = createSessionOwner({ persistence, createId: () => 'memory-session' })

  const snapshot = owner.replace({ accessToken: 'ACCESS', refreshToken: 'REFRESH' })
  assert.equal(snapshot.accessToken, 'ACCESS')
  assert.equal(snapshot.persistence, 'memory')
  assert.equal(persistence.envelope, null)

  owner.clear('logout')
  assert.equal(owner.snapshot().accessToken, '')
})

function deferred<T>(): {
  promise: Promise<T>
  resolve(value: T | PromiseLike<T>): void
} {
  let resolve!: (value: T | PromiseLike<T>) => void
  const promise = new Promise<T>((next) => { resolve = next })
  return { promise, resolve }
}

function incrementingId(prefix: string): () => string {
  let next = 0
  return () => `${prefix}-${++next}`
}
