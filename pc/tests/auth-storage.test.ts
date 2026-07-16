import test from 'node:test'
import assert from 'node:assert/strict'

import { clearAuthStorage, readAuthToken, readRefreshToken, writeAuthTokens } from '../src/utils/authStorage.ts'

function installMemoryStorage(initial: Record<string, string> = {}) {
  const values = new Map(Object.entries(initial))
  const memoryStorage = {
    get length() {
      return values.size
    },
    clear() {
      values.clear()
    },
    getItem(key: string) {
      return values.has(key) ? values.get(key)! : null
    },
    key(index: number) {
      return Array.from(values.keys())[index] || null
    },
    removeItem(key: string) {
      values.delete(key)
    },
    setItem(key: string, value: string) {
      values.set(key, String(value))
    },
  } as Storage

  Object.defineProperty(globalThis, 'localStorage', {
    value: memoryStorage,
    configurable: true,
  })

  return memoryStorage
}

test('auth storage reads standalone token before persisted pinia token', () => {
  installMemoryStorage({
    token: 'standalone-token',
    refresh_token: 'standalone-refresh',
    user: JSON.stringify({
      token: 'pinia-token',
      refreshToken: 'pinia-refresh',
    }),
  })

  assert.equal(readAuthToken(), 'standalone-token')
  assert.equal(readRefreshToken(), 'standalone-refresh')
})

test('auth storage falls back to persisted pinia token when standalone token is missing', () => {
  installMemoryStorage({
    user: JSON.stringify({
      token: 'pinia-token',
      refreshToken: 'pinia-refresh',
    }),
  })

  assert.equal(readAuthToken(), 'pinia-token')
  assert.equal(readRefreshToken(), 'pinia-refresh')
})

test('auth storage writes refreshed tokens and clears all login stores', () => {
  const storage = installMemoryStorage({
    user: JSON.stringify({
      token: 'old-token',
      user: { email: 'user@example.com' },
    }),
  })

  writeAuthTokens('new-token', 'new-refresh')
  assert.equal(storage.getItem('token'), 'new-token')
  assert.equal(storage.getItem('refresh_token'), 'new-refresh')
  assert.equal(JSON.parse(storage.getItem('user') || '{}').token, 'new-token')

  clearAuthStorage()
  assert.equal(storage.getItem('token'), null)
  assert.equal(storage.getItem('refresh_token'), null)
  assert.equal(storage.getItem('user'), null)
})

