import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import axios, { AxiosError, type AxiosResponse, type InternalAxiosRequestConfig } from 'axios'
import { installAuthSessionInterceptors, isAuthBootstrapRequest } from '../src/api/requestAuth.ts'

const clientSource = readFileSync(new URL('../src/api/client.ts', import.meta.url), 'utf8')

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

test('HTTP failures without a backend message keep the localized caller fallback', () => {
  assert.match(
    clientSource,
    /if \(axios\.isAxiosError\(error\)\) \{[\s\S]*?if \(!axiosError\.response\) \{[\s\S]*?return i18n\.global\.t\('common\.networkUnavailable'\)[\s\S]*?\}[\s\S]*?return fallback/,
  )
  assert.doesNotMatch(clientSource, /return error instanceof Error[\s\S]*?axiosError\.response/)
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
