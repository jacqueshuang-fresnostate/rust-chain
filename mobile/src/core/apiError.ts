export type ApiErrorKind = 'abort' | 'http' | 'network' | 'stale-session' | 'unknown'

export interface ApiErrorDiagnostic {
  readonly requestId?: string
  readonly serverMessage?: string
  readonly transportCode?: string
  /** Original value for diagnostics/logging only; never render it as UI copy. */
  readonly cause?: unknown
}

export interface NormalizedApiError {
  readonly kind: ApiErrorKind
  readonly code: string
  readonly status?: number
  readonly messageKey?: string
  /** Controlled compatibility token, not the raw backend message. */
  readonly compatibilityMessage?: string
  readonly diagnostic: ApiErrorDiagnostic
}

export interface NormalizeApiErrorOptions {
  offline?: boolean
}

const MESSAGE_KEYS_BY_CODE: Readonly<Record<string, string>> = {
  INVALID_2FA_CODE: 'auth.twoFactorFailed',
  INVALID_TOTP_CODE: 'auth.twoFactorFailed',
  TOTP_INVALID: 'auth.twoFactorFailed',
  AUTH_CHALLENGE_EXPIRED: 'auth.challengeExpired',
  LOGIN_CHALLENGE_EXPIRED: 'auth.challengeExpired',
  CHALLENGE_EXPIRED: 'auth.challengeExpired',
  REQUEST_TIMEOUT: 'common.requestTimeout',
  DEVICE_OFFLINE: 'common.deviceOffline',
  NETWORK_UNAVAILABLE: 'common.networkUnavailable',
}

export class StaleSessionError extends Error {
  readonly code = 'STALE_SESSION'

  constructor() {
    super('stale session generation')
    this.name = 'StaleSessionError'
  }
}

/**
 * Normalizes transport failures around stable machine codes. Raw backend copy
 * is retained only under diagnostic and is never selected as the user message.
 */
export function normalizeApiError(
  error: unknown,
  options: NormalizeApiErrorOptions = {},
): NormalizedApiError {
  if (error instanceof StaleSessionError || readString(error, 'code') === 'STALE_SESSION') {
    return {
      kind: 'stale-session',
      code: 'STALE_SESSION',
      diagnostic: { cause: error },
    }
  }

  const transportCode = normalizeCode(readString(error, 'code'))
  const axiosLike = isAxiosLikeError(error)
  const response = readRecord(error, 'response')
  const status = readFiniteNumber(response?.status)
  const responseData = response?.data
  const serverCode = normalizeCode(readBackendCode(responseData))
  const serverMessage = readBackendMessage(responseData)
  const requestId = readRequestId(response)
  const diagnostic: ApiErrorDiagnostic = {
    ...(requestId ? { requestId } : {}),
    ...(serverMessage ? { serverMessage } : {}),
    ...(transportCode ? { transportCode } : {}),
    cause: error,
  }

  if (transportCode === 'ERR_CANCELED' || transportCode === 'ABORT_ERR') {
    return { kind: 'abort', code: 'REQUEST_ABORTED', diagnostic }
  }
  if (transportCode === 'ECONNABORTED' || transportCode === 'ETIMEDOUT') {
    return {
      kind: 'network',
      code: 'REQUEST_TIMEOUT',
      messageKey: MESSAGE_KEYS_BY_CODE.REQUEST_TIMEOUT,
      diagnostic,
    }
  }

  if (axiosLike && !response) {
    const code = options.offline ? 'DEVICE_OFFLINE' : 'NETWORK_UNAVAILABLE'
    return {
      kind: 'network',
      code,
      messageKey: MESSAGE_KEYS_BY_CODE[code],
      diagnostic,
    }
  }

  if (response) {
    const code = serverCode || (status ? `HTTP_${status}` : 'HTTP_ERROR')
    return {
      kind: 'http',
      code,
      ...(status === undefined ? {} : { status }),
      ...(messageKeyFor(code, status) ? { messageKey: messageKeyFor(code, status) } : {}),
      ...(controlledCompatibilityMessage(code, serverMessage)
        ? { compatibilityMessage: controlledCompatibilityMessage(code, serverMessage) }
        : {}),
      diagnostic,
    }
  }

  return {
    kind: 'unknown',
    code: transportCode || 'UNKNOWN_ERROR',
    diagnostic,
  }
}

export function resolveSafeApiErrorMessage(
  error: NormalizedApiError,
  fallback: string,
  translate: (key: string) => string,
): string {
  if (error.compatibilityMessage) return error.compatibilityMessage
  if (error.messageKey) return translate(error.messageKey)
  return fallback
}

function messageKeyFor(code: string, status: number | undefined): string | undefined {
  if (status !== undefined && status >= 500) return 'common.serviceUnavailable'
  return MESSAGE_KEYS_BY_CODE[code]
}

function controlledCompatibilityMessage(
  code: string,
  serverMessage: string | undefined,
): string | undefined {
  if (code !== 'VALIDATION_ERROR' || !serverMessage) return undefined
  const normalized = serverMessage.toLowerCase()
  if (normalized.includes('margin amount is below product minimum')) {
    return 'margin amount is below product minimum'
  }
  if (normalized.includes('margin amount exceeds product maximum')) {
    return 'margin amount exceeds product maximum'
  }
  return undefined
}

function isAxiosLikeError(error: unknown): boolean {
  if (!error || typeof error !== 'object') return false
  const record = error as Record<string, unknown>
  return record.isAxiosError === true
    || 'config' in record
    || 'request' in record
    || 'response' in record
}

function readBackendCode(value: unknown): string {
  if (!value || typeof value !== 'object') return ''
  const record = value as Record<string, unknown>
  const nested = record.error && typeof record.error === 'object'
    ? record.error as Record<string, unknown>
    : null
  return firstString(record.code, record.error_code, nested?.code)
}

function readBackendMessage(value: unknown): string | undefined {
  if (typeof value === 'string') return value.trim() || undefined
  if (!value || typeof value !== 'object') return undefined
  const record = value as Record<string, unknown>
  const nested = record.error && typeof record.error === 'object'
    ? record.error as Record<string, unknown>
    : null
  return firstString(record.message, record.detail, nested?.message) || undefined
}

function readRequestId(response: Record<string, unknown> | null): string | undefined {
  if (!response) return undefined
  const headers = response.headers
  if (!headers || typeof headers !== 'object') return undefined
  const record = headers as Record<string, unknown>
  const direct = firstString(
    record['x-request-id'],
    record['X-Request-Id'],
    record['x-correlation-id'],
  )
  if (direct) return direct
  const getter = (headers as { get?: (name: string) => unknown }).get
  if (typeof getter !== 'function') return undefined
  try {
    return firstString(getter.call(headers, 'x-request-id'), getter.call(headers, 'x-correlation-id')) || undefined
  } catch {
    return undefined
  }
}

function readRecord(value: unknown, key: string): Record<string, unknown> | null {
  if (!value || typeof value !== 'object') return null
  const child = (value as Record<string, unknown>)[key]
  return child && typeof child === 'object' ? child as Record<string, unknown> : null
}

function readString(value: unknown, key: string): string {
  if (!value || typeof value !== 'object') return ''
  const child = (value as Record<string, unknown>)[key]
  return typeof child === 'string' ? child.trim() : ''
}

function readFiniteNumber(value: unknown): number | undefined {
  return typeof value === 'number' && Number.isFinite(value) ? value : undefined
}

function normalizeCode(value: string): string {
  return value.trim().toUpperCase().replace(/[^A-Z0-9]+/g, '_').replace(/^_+|_+$/g, '')
}

function firstString(...values: unknown[]): string {
  for (const value of values) {
    if (typeof value === 'string' && value.trim()) return value.trim()
  }
  return ''
}
