import {
  CanceledError,
  type AxiosError,
  type AxiosInstance,
  type AxiosRequestConfig,
  type GenericAbortSignal,
  type InternalAxiosRequestConfig,
} from 'axios'
import { StaleSessionError } from '../core/apiError.ts'
import { composeAbortSignals } from '../core/apiRequest.ts'

const AUTH_BOOTSTRAP_PATTERN = /\/auth\/(?:login|register|password|refresh)(?:\/|$)/

export interface AuthRequestSession {
  readonly accessToken: string
  readonly refreshToken: string
  readonly scope: string
  readonly epoch: number
  readonly signal?: AbortSignal
}

export interface AuthRefreshResult {
  readonly accessToken: string
  readonly session?: AuthRequestSession
}

type RetriableRequest = InternalAxiosRequestConfig & {
  _hippoCallerSignal?: GenericAbortSignal
  _hippoCallerSignalCaptured?: boolean
  _hippoDisposeSignal?: () => void
  _hippoHadAuth?: boolean
  _hippoPublic?: boolean
  _hippoRetried?: boolean
  _hippoSession?: AuthRequestSession
}

/** Marks a backend endpoint as public even when stale credentials remain in storage. */
export function publicApiRequestConfig(config: AxiosRequestConfig = {}): AxiosRequestConfig {
  return { ...config, _hippoPublic: true } as AxiosRequestConfig
}

export interface AuthSessionInterceptorDependencies {
  clearSession: (session?: AuthRequestSession) => boolean | void
  onSessionExpired: () => void
  /** Kept for API compatibility; readSession is the authoritative path. */
  readAccessToken: () => string
  readSession?: () => AuthRequestSession
  isSessionCurrent?: (session: AuthRequestSession) => boolean
  refreshAccessToken: (
    session?: AuthRequestSession,
  ) => Promise<AuthRefreshResult | string | null>
}

export function isAuthBootstrapRequest(url: unknown): boolean {
  if (typeof url !== 'string' || !url.trim()) return false
  try {
    return AUTH_BOOTSTRAP_PATTERN.test(new URL(url, 'https://mobile.invalid').pathname)
  } catch {
    return AUTH_BOOTSTRAP_PATTERN.test(url.split(/[?#]/, 1)[0] || '')
  }
}

/**
 * Adds one generation-aware authentication lifecycle to Axios. Requests are
 * tied to the session epoch that created them; logout aborts that epoch and a
 * late response is rejected before application code can write it back.
 */
export function installAuthSessionInterceptors(
  instance: AxiosInstance,
  dependencies: AuthSessionInterceptorDependencies,
): void {
  const refreshes = new Map<string, Promise<AuthRefreshResult | string | null>>()

  const captureSession = (): AuthRequestSession => {
    if (dependencies.readSession) return dependencies.readSession()
    const accessToken = dependencies.readAccessToken().trim()
    return {
      accessToken,
      refreshToken: '',
      scope: accessToken ? `legacy-${stableTokenHash(accessToken)}` : '',
      epoch: 0,
    }
  }

  const isCurrent = (session: AuthRequestSession): boolean => {
    if (dependencies.isSessionCurrent) return dependencies.isSessionCurrent(session)
    return dependencies.readAccessToken().trim() === session.accessToken
  }

  const refreshAccessTokenOnce = (
    session: AuthRequestSession,
  ): Promise<AuthRefreshResult | string | null> => {
    const key = `${session.scope}:${session.epoch}`
    const pending = refreshes.get(key)
    if (pending) return pending
    let current: Promise<AuthRefreshResult | string | null>
    current = dependencies.refreshAccessToken(session)
      .catch(() => null)
      .finally(() => {
        if (refreshes.get(key) === current) refreshes.delete(key)
      })
    refreshes.set(key, current)
    return current
  }

  const disposeRequestSignal = (request: RetriableRequest | undefined): void => {
    request?._hippoDisposeSignal?.()
    if (request) request._hippoDisposeSignal = undefined
  }

  instance.interceptors.request.use((config: RetriableRequest) => {
    disposeRequestSignal(config)
    if (!config._hippoCallerSignalCaptured) {
      config._hippoCallerSignal = config.signal
      config._hippoCallerSignalCaptured = true
    }

    if (config._hippoPublic || isAuthBootstrapRequest(config.url)) {
      config.headers.delete('Authorization')
      config._hippoHadAuth = false
      config._hippoSession = undefined
      config.signal = config._hippoCallerSignal
      return config
    }

    const session = captureSession()
    if (session.accessToken) {
      config.headers.set('Authorization', `Bearer ${session.accessToken}`)
    } else {
      config.headers.delete('Authorization')
    }
    config._hippoHadAuth = Boolean(session.accessToken)
    config._hippoSession = session

    const composed = composeAbortSignals([
      asAbortSignal(config._hippoCallerSignal),
      session.accessToken ? session.signal : undefined,
    ])
    config.signal = composed.signal
    config._hippoDisposeSignal = composed.dispose
    return config
  })

  instance.interceptors.response.use(
    (response) => {
      const request = response.config as RetriableRequest
      disposeRequestSignal(request)
      if (request._hippoHadAuth && request._hippoSession && !isCurrent(request._hippoSession)) {
        return Promise.reject(new StaleSessionError())
      }
      return response
    },
    async (error: AxiosError) => {
      const request = error.config as RetriableRequest | undefined
      disposeRequestSignal(request)
      const protectedRequest = Boolean(
        request
        && !isAuthBootstrapRequest(request.url)
        && request._hippoHadAuth
        && request._hippoSession,
      )
      const requestSession = request?._hippoSession

      if (protectedRequest && requestSession && !isCurrent(requestSession)) {
        return Promise.reject(new StaleSessionError())
      }

      if (
        error.response?.status === 401
        && request
        && requestSession
        && protectedRequest
        && !request._hippoRetried
      ) {
        const refreshed = await refreshAccessTokenOnce(requestSession)
        if (request._hippoCallerSignal?.aborted) {
          return Promise.reject(new CanceledError('request canceled while refreshing session'))
        }

        const nextSession = resolveRefreshedSession(refreshed, captureSession)
        if (nextSession && nextSession.accessToken && isCurrent(nextSession)) {
          request._hippoRetried = true
          request._hippoSession = nextSession
          request.headers.set('Authorization', `Bearer ${nextSession.accessToken}`)
          // The request interceptor composes the new generation signal.
          request.signal = request._hippoCallerSignal
          return instance.request(request)
        }
      }

      if (
        error.response?.status === 401
        && protectedRequest
        && requestSession
        && isCurrent(requestSession)
      ) {
        const cleared = dependencies.clearSession(requestSession)
        if (cleared !== false) dependencies.onSessionExpired()
      }
      return Promise.reject(error)
    },
  )
}

function resolveRefreshedSession(
  result: AuthRefreshResult | string | null,
  captureSession: () => AuthRequestSession,
): AuthRequestSession | null {
  if (!result) return null
  if (typeof result === 'string') {
    const current = captureSession()
    return current.accessToken === result.trim() ? current : null
  }
  const accessToken = result.accessToken.trim()
  if (!accessToken) return null
  const current = result.session ?? captureSession()
  return current.accessToken === accessToken ? current : null
}

function stableTokenHash(token: string): string {
  let hash = 2_166_136_261
  for (let index = 0; index < token.length; index += 1) {
    hash ^= token.charCodeAt(index)
    hash = Math.imul(hash, 16_777_619)
  }
  return (hash >>> 0).toString(36)
}

function asAbortSignal(signal: GenericAbortSignal | undefined): AbortSignal | undefined {
  return signal && typeof signal.addEventListener === 'function'
    && typeof signal.removeEventListener === 'function'
    ? signal as AbortSignal
    : undefined
}
