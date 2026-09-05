import assert from 'node:assert/strict'
import test from 'node:test'
import axios, { AxiosError, type AxiosResponse, type InternalAxiosRequestConfig } from 'axios'
import {
  installAuthSessionInterceptors,
  isAuthBootstrapRequest,
  publicApiRequestConfig,
} from '../src/api/requestAuth.ts'
import { normalizeApiError, resolveSafeApiErrorMessage } from '../src/core/apiError.ts'
import { createSessionOwner, type SessionLease } from '../src/core/sessionOwner.ts'

const bootstrapPaths = [
  '/api/v1/auth/login',
  '/api/v1/auth/login/config',
  '/api/v1/auth/login/2fa',
  '/api/v1/auth/login/2fa/setup',
  '/api/v1/auth/login/2fa/setup/confirm',
  '/api/v1/auth/register',
  '/api/v1/auth/register/email-code',
  '/api/v1/auth/password/reset-code',
  '/api/v1/auth/password/reset',
  '/api/v1/auth/refresh',
]

test('auth bootstrap classification covers login, registration, 2FA, password reset, and refresh', () => {
  for (const path of bootstrapPaths) {
    assert.equal(isAuthBootstrapRequest(path), true, path)
  }
  assert.equal(isAuthBootstrapRequest('https://api.example.test/api/v1/user/profile'), false)
  assert.equal(isAuthBootstrapRequest('/api/v1/margin/products'), false)
})

test('bootstrap requests strip stale Bearer headers and never refresh on 401', async () => {
  for (const url of bootstrapPaths) {
    const instance = axios.create()
    let refreshCalls = 0
    let clearCalls = 0
    let capturedAuthorization: unknown
    installAuthSessionInterceptors(instance, {
      readAccessToken: () => 'stale-access',
      refreshAccessToken: async () => {
        refreshCalls += 1
        return 'fresh-access'
      },
      clearSession: () => { clearCalls += 1 },
      onSessionExpired: () => undefined,
    })

    await assert.rejects(instance.request({
      url,
      method: 'post',
      headers: { authorization: 'Bearer caller-stale' },
      adapter: async (config) => {
        capturedAuthorization = config.headers.Authorization
        throw unauthorized(config)
      },
    }))

    assert.equal(capturedAuthorization, undefined, url)
    assert.equal(refreshCalls, 0, url)
    assert.equal(clearCalls, 0, url)
  }
})

test('protected requests attach Bearer, share one refresh, and replay once with the fresh token', async () => {
  const instance = axios.create()
  let accessToken = 'stale-access'
  let refreshCalls = 0
  const attempts = new Map<string, number>()
  const authorizations: string[] = []
  installAuthSessionInterceptors(instance, {
    readAccessToken: () => accessToken,
    refreshAccessToken: async () => {
      refreshCalls += 1
      await new Promise<void>((resolve) => setImmediate(resolve))
      accessToken = 'fresh-access'
      return accessToken
    },
    clearSession: () => assert.fail('successful refresh must not clear the session'),
    onSessionExpired: () => assert.fail('successful refresh must not expire the session'),
  })

  const adapter = async (config: InternalAxiosRequestConfig): Promise<AxiosResponse> => {
    const url = String(config.url)
    const attempt = (attempts.get(url) || 0) + 1
    attempts.set(url, attempt)
    authorizations.push(String(config.headers.Authorization || ''))
    if (attempt === 1) throw unauthorized(config)
    return success(config)
  }

  await Promise.all([
    instance.get('/api/v1/user/profile', { adapter }),
    instance.get('/api/v1/wallet/accounts', { adapter }),
  ])

  assert.equal(refreshCalls, 1)
  assert.deepEqual([...attempts.values()], [2, 2])
  assert.equal(authorizations.filter((value) => value === 'Bearer stale-access').length, 2)
  assert.equal(authorizations.filter((value) => value === 'Bearer fresh-access').length, 2)
})

test('guest 401 responses stay local and cannot redirect a later public page', async () => {
  const instance = axios.create()
  let refreshCalls = 0
  let clearCalls = 0
  let expiredCalls = 0
  installAuthSessionInterceptors(instance, {
    readAccessToken: () => '',
    refreshAccessToken: async () => {
      refreshCalls += 1
      return null
    },
    clearSession: () => { clearCalls += 1 },
    onSessionExpired: () => { expiredCalls += 1 },
  })

  await assert.rejects(instance.get('/api/v1/earn/products', {
    adapter: async (config) => { throw unauthorized(config) },
  }))

  assert.equal(refreshCalls, 0)
  assert.equal(clearCalls, 0)
  assert.equal(expiredCalls, 0)
})

test('explicit public requests strip stale credentials and never expire the session', async () => {
  const instance = axios.create()
  let capturedAuthorization: unknown
  let refreshCalls = 0
  let clearCalls = 0
  let expiredCalls = 0
  installAuthSessionInterceptors(instance, {
    readAccessToken: () => 'stale-access',
    refreshAccessToken: async () => {
      refreshCalls += 1
      return null
    },
    clearSession: () => { clearCalls += 1 },
    onSessionExpired: () => { expiredCalls += 1 },
  })

  await assert.rejects(instance.get('/api/v1/new-coins', publicApiRequestConfig({
    headers: { Authorization: 'Bearer caller-stale' },
    adapter: async (config) => {
      capturedAuthorization = config.headers.Authorization
      throw unauthorized(config)
    },
  })))

  assert.equal(capturedAuthorization, undefined)
  assert.equal(refreshCalls, 0)
  assert.equal(clearCalls, 0)
  assert.equal(expiredCalls, 0)
})

test('failed refresh clears the protected session without recursive replay', async () => {
  const instance = axios.create()
  let refreshCalls = 0
  let clearCalls = 0
  let expiredCalls = 0
  let requestCalls = 0
  installAuthSessionInterceptors(instance, {
    readAccessToken: () => 'expired-access',
    refreshAccessToken: async () => {
      refreshCalls += 1
      return null
    },
    clearSession: () => { clearCalls += 1 },
    onSessionExpired: () => { expiredCalls += 1 },
  })

  await assert.rejects(instance.get('/api/v1/user/profile', {
    adapter: async (config) => {
      requestCalls += 1
      throw unauthorized(config)
    },
  }))

  assert.equal(requestCalls, 1)
  assert.equal(refreshCalls, 1)
  assert.equal(clearCalls, 1)
  assert.equal(expiredCalls, 1)
})

test('HTTP failures without a stable backend code keep the localized caller fallback', () => {
  const normalized = normalizeApiError({
    isAxiosError: true,
    response: { status: 400, data: {} },
  })
  assert.equal(
    resolveSafeApiErrorMessage(normalized, 'localized fallback', (key) => key),
    'localized fallback',
  )
})

test('logout during a deferred refresh prevents token writeback and request replay', async () => {
  const instance = axios.create()
  const owner = createSessionOwner({ createId: incrementingId() })
  owner.replace({ accessToken: 'ACCESS_A', refreshToken: 'REFRESH_A' })
  const refreshStarted = deferred<void>()
  const refreshResponse = deferred<{ accessToken: string; refreshToken: string }>()
  let adapterCalls = 0
  let expiredCalls = 0

  installAuthSessionInterceptors(instance, {
    readAccessToken: () => owner.snapshot().accessToken,
    readSession: () => requestSession(owner.capture()),
    isSessionCurrent: (session) => owner.isCurrent(session),
    refreshAccessToken: async (session) => {
      assert.ok(session)
      refreshStarted.resolve(undefined)
      const tokens = await refreshResponse.promise
      const committed = owner.commitRefresh(session, tokens)
      if (!committed) return null
      const current = owner.capture()
      return { accessToken: current.accessToken, session: requestSession(current) }
    },
    clearSession: (session) => Boolean(session && owner.clearIfCurrent(session, 'expired')),
    onSessionExpired: () => { expiredCalls += 1 },
  })

  const pending = instance.get('/api/v1/user/profile', {
    adapter: async (config) => {
      adapterCalls += 1
      throw unauthorized(config)
    },
  })
  await refreshStarted.promise
  owner.clear('logout')
  refreshResponse.resolve({ accessToken: 'ACCESS_A_LATE', refreshToken: 'REFRESH_A_LATE' })

  await assert.rejects(pending)
  assert.equal(adapterCalls, 1)
  assert.equal(expiredCalls, 0)
  assert.equal(owner.snapshot().accessToken, '')
})

test('a successful private response from an aborted session generation never reaches callers', async () => {
  const instance = axios.create()
  const owner = createSessionOwner({ createId: incrementingId() })
  owner.replace({ accessToken: 'ACCESS_A', refreshToken: 'REFRESH_A' })
  const adapterStarted = deferred<void>()
  const adapterResponse = deferred<AxiosResponse>()
  let writes = 0

  installAuthSessionInterceptors(instance, {
    readAccessToken: () => owner.snapshot().accessToken,
    readSession: () => requestSession(owner.capture()),
    isSessionCurrent: (session) => owner.isCurrent(session),
    refreshAccessToken: async () => null,
    clearSession: () => false,
    onSessionExpired: () => undefined,
  })

  const pending = instance.get('/api/v1/wallet/accounts', {
    adapter: async (config) => {
      adapterStarted.resolve(undefined)
      return adapterResponse.promise.then((response) => ({ ...response, config }))
    },
  }).then(() => { writes += 1 })

  await adapterStarted.promise
  owner.clear('logout')
  adapterResponse.resolve(success({} as InternalAxiosRequestConfig))
  await assert.rejects(pending)
  assert.equal(writes, 0)
})

function unauthorized(config: InternalAxiosRequestConfig): AxiosError {
  const response: AxiosResponse = {
    config,
    data: { message: 'unauthorized' },
    headers: {},
    status: 401,
    statusText: 'Unauthorized',
  }
  return new AxiosError('unauthorized', 'ERR_BAD_REQUEST', config, undefined, response)
}

function success(config: InternalAxiosRequestConfig): AxiosResponse {
  return {
    config,
    data: { ok: true },
    headers: {},
    status: 200,
    statusText: 'OK',
  }
}

function requestSession(lease: SessionLease) {
  return {
    accessToken: lease.accessToken,
    refreshToken: lease.refreshToken,
    scope: lease.scope,
    epoch: lease.epoch,
    signal: lease.signal,
  }
}

function deferred<T>(): {
  promise: Promise<T>
  resolve(value: T | PromiseLike<T>): void
} {
  let resolve!: (value: T | PromiseLike<T>) => void
  const promise = new Promise<T>((next) => { resolve = next })
  return { promise, resolve }
}

function incrementingId(): () => string {
  let next = 0
  return () => `session-${++next}`
}
