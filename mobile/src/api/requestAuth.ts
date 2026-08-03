import type { AxiosError, AxiosInstance, InternalAxiosRequestConfig } from 'axios'

const AUTH_BOOTSTRAP_PATTERN = /\/auth\/(?:login|register|password|refresh)(?:\/|$)/

type RetriableRequest = InternalAxiosRequestConfig & {
  _hippoHadAuth?: boolean
  _hippoRetried?: boolean
}

export interface AuthSessionInterceptorDependencies {
  clearSession: () => void
  onSessionExpired: () => void
  readAccessToken: () => string
  refreshAccessToken: () => Promise<string | null>
}

export function isAuthBootstrapRequest(url: unknown): boolean {
  if (typeof url !== 'string' || !url.trim()) return false
  try {
    return AUTH_BOOTSTRAP_PATTERN.test(new URL(url, 'https://mobile.invalid').pathname)
  } catch {
    return AUTH_BOOTSTRAP_PATTERN.test(url.split(/[?#]/, 1)[0] || '')
  }
}

export function installAuthSessionInterceptors(
  instance: AxiosInstance,
  dependencies: AuthSessionInterceptorDependencies,
): void {
  let refreshPromise: Promise<string | null> | null = null

  function refreshAccessTokenOnce(): Promise<string | null> {
    if (!refreshPromise) {
      refreshPromise = dependencies.refreshAccessToken()
        .catch(() => null)
        .finally(() => { refreshPromise = null })
    }
    return refreshPromise
  }

  instance.interceptors.request.use((config: RetriableRequest) => {
    if (isAuthBootstrapRequest(config.url)) {
      config.headers.delete('Authorization')
      config._hippoHadAuth = false
      return config
    }
    const token = dependencies.readAccessToken()
    if (token) config.headers.set('Authorization', `Bearer ${token}`)
    config._hippoHadAuth = Boolean(token || config.headers.get('Authorization'))
    return config
  })

  instance.interceptors.response.use(
    (response) => response,
    async (error: AxiosError) => {
      const request = error.config as RetriableRequest | undefined
      const protectedRequest = Boolean(
        request
        && !isAuthBootstrapRequest(request.url)
        && request._hippoHadAuth,
      )
      if (error.response?.status === 401 && request && protectedRequest && !request._hippoRetried) {
        const nextToken = await refreshAccessTokenOnce()
        if (nextToken) {
          request._hippoRetried = true
          request.headers.set('Authorization', `Bearer ${nextToken}`)
          return instance.request(request)
        }
      }
      if (error.response?.status === 401 && protectedRequest) {
        dependencies.clearSession()
        dependencies.onSessionExpired()
      }
      return Promise.reject(error)
    },
  )
}
