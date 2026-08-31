import assert from 'node:assert/strict'
import test from 'node:test'
import {
  composeAbortSignals,
  createApiHttpClient,
  DEFAULT_API_REQUEST_TIMEOUT_MS,
} from '../src/core/apiRequest.ts'
import {
  normalizeApiError,
  resolveSafeApiErrorMessage,
} from '../src/core/apiError.ts'

test('API client applies a bounded default timeout and preserves caller AbortSignal', async () => {
  const client = createApiHttpClient()
  assert.equal(client.defaults.timeout, DEFAULT_API_REQUEST_TIMEOUT_MS)

  const controller = new AbortController()
  controller.abort('route-replaced')
  let adapterCalls = 0
  await assert.rejects(client.get('/fixture', {
    signal: controller.signal,
    adapter: async () => {
      adapterCalls += 1
      throw new Error('adapter must not run for a pre-aborted request')
    },
  }), (error: unknown) => {
    const value = error as { code?: string }
    return value.code === 'ERR_CANCELED'
  })
  assert.equal(adapterCalls, 0)
})

test('composed caller/session signals abort together and dispose listeners idempotently', () => {
  const caller = new AbortController()
  const session = new AbortController()
  const combined = composeAbortSignals([caller.signal, session.signal])
  assert.ok(combined.signal)
  assert.equal(combined.signal.aborted, false)

  session.abort('logout')
  assert.equal(combined.signal.aborted, true)
  combined.dispose()
  combined.dispose()
})

test('error normalization is code-first and keeps raw server details diagnostics-only', () => {
  const normalized = normalizeApiError({
    isAxiosError: true,
    response: {
      status: 400,
      data: {
        code: 'invalid_2fa_code',
        message: 'raw provider detail with TOKEN=secret',
      },
      headers: { 'x-request-id': 'request-42' },
    },
  })

  assert.equal(normalized.code, 'INVALID_2FA_CODE')
  assert.equal(normalized.status, 400)
  assert.equal(normalized.messageKey, 'auth.twoFactorFailed')
  assert.equal(normalized.diagnostic.serverMessage, 'raw provider detail with TOKEN=secret')
  assert.equal(normalized.diagnostic.requestId, 'request-42')
  assert.equal(
    resolveSafeApiErrorMessage(normalized, 'fallback', (key) => `translated:${key}`),
    'translated:auth.twoFactorFailed',
  )
})

test('unknown 5xx and arbitrary Error messages resolve to safe localized copy', () => {
  const server = normalizeApiError({
    isAxiosError: true,
    response: {
      status: 503,
      data: { code: 'UPSTREAM_HTML', message: '<html>secret diagnostics</html>' },
    },
  })
  assert.equal(server.code, 'UPSTREAM_HTML')
  assert.equal(server.messageKey, 'common.serviceUnavailable')
  assert.equal(
    resolveSafeApiErrorMessage(server, 'caller fallback', (key) => `translated:${key}`),
    'translated:common.serviceUnavailable',
  )

  const local = normalizeApiError(new Error('filesystem and token diagnostics'))
  assert.equal(resolveSafeApiErrorMessage(local, 'caller fallback', (key) => key), 'caller fallback')
})

test('timeout, offline, network, abort, and stable boundary diagnostics have deterministic codes', () => {
  assert.equal(normalizeApiError({ isAxiosError: true, code: 'ETIMEDOUT' }).code, 'REQUEST_TIMEOUT')
  assert.equal(normalizeApiError({ isAxiosError: true, code: 'ERR_CANCELED' }).code, 'REQUEST_ABORTED')
  assert.equal(normalizeApiError({ isAxiosError: true }, { offline: true }).code, 'DEVICE_OFFLINE')
  assert.equal(normalizeApiError({ isAxiosError: true }).code, 'NETWORK_UNAVAILABLE')

  const boundary = normalizeApiError({
    isAxiosError: true,
    response: {
      status: 400,
      data: {
        code: 'VALIDATION_ERROR',
        message: 'validation error: margin amount is below product minimum; internal=hidden',
      },
    },
  })
  assert.equal(boundary.compatibilityMessage, 'margin amount is below product minimum')
})
