import axios, { type AxiosInstance, type AxiosRequestConfig, type AxiosResponse } from 'axios'
import { createToastInterface, globalEventBus } from 'vue-toastification'
import { APP_CONFIG } from '@/config/app.ts'
import { clearAuthStorage, readAuthToken, readRefreshToken, writeAuthTokens } from '@/utils/authStorage'
import { createAuthorizationHeader, type BackendAuthTokenResponse } from './backendAdapters'
import { stompService } from './stomp'

type RetriableRequestConfig = AxiosRequestConfig & {
    _authRetry?: boolean
}

let tokenRefreshPromise: Promise<string | null> | null = null

function handleLoginExpired() {
    stompService.disconnect('private')
    clearAuthStorage()
    window.location.href = '/login'
}

function backendBaseUrl(): string {
    const domain = APP_CONFIG.BACKEND_API_DOMAIN.replace(/\/$/, '')
    const prefix = APP_CONFIG.BACKEND_API_PREFIX.replace(/\/$/, '')
    return `${domain}${prefix}`
}

function isBackendRequest(url?: string): boolean {
    if (!url) return true
    if (url.startsWith('http://') || url.startsWith('https://')) {
        return url.startsWith(backendBaseUrl()) || url.startsWith(APP_CONFIG.BACKEND_API_DOMAIN)
    }
    return true
}

function requestPath(url?: string): string {
    if (!url) return ''
    try {
        return new URL(url, backendBaseUrl()).pathname
    } catch {
        return url
    }
}

function isAuthSessionRoute(url?: string): boolean {
    const path = requestPath(url)
    return [
        '/auth/login',
        '/auth/login/2fa',
        '/auth/login/2fa/reset-code',
        '/auth/login/2fa/reset',
        '/auth/register',
        '/auth/register/email-code',
        '/auth/password/reset-code',
        '/auth/password/reset',
        '/auth/refresh',
    ].some((authPath) => path.endsWith(authPath))
}

function contentTypeOf(headers: unknown): string {
    const values = headers as (Record<string, string | undefined> & { get?: (name: string) => unknown }) | undefined
    return String(
        values?.get?.('Content-Type') ||
        values?.get?.('content-type') ||
        values?.['Content-Type'] ||
        values?.['content-type'] ||
        ''
    )
}

function setHeader(config: AxiosRequestConfig, name: string, value: string) {
    const headers = config.headers as (Record<string, string> & { set?: (name: string, value: string) => void }) | undefined
    if (headers?.set) {
        headers.set(name, value)
        return
    }
    config.headers = { ...(headers || {}), [name]: value }
}

function deleteHeader(config: AxiosRequestConfig, name: string) {
    const headers = config.headers as (Record<string, string | undefined> & { delete?: (name: string) => void }) | undefined
    if (headers?.delete) {
        headers.delete(name)
        return
    }
    if (!headers) return
    delete headers[name]
    delete headers[name.toLowerCase()]
}

function shouldJsonEncodeBody(config: AxiosRequestConfig): boolean {
    return !!config.data && typeof config.data === 'object' && !(config.data instanceof FormData)
}

function transformRequestData(data: unknown, headers: unknown) {
    if (!data || typeof data !== 'object' || data instanceof FormData) return data

    const contentType = contentTypeOf(headers)
    if (contentType.includes('application/json')) {
        return JSON.stringify(data)
    }

    return data
}

async function refreshAccessToken(): Promise<string | null> {
    const refreshToken = readRefreshToken()
    if (!refreshToken) return null

    try {
        const response = await axios.post<BackendAuthTokenResponse>(
            `${backendBaseUrl()}/auth/refresh`,
            { refresh_token: refreshToken },
            {
                timeout: 10000,
                headers: {
                    'Content-Type': 'application/json',
                },
                transformRequest: [transformRequestData],
            },
        )
        const nextToken = response.data?.access_token?.trim()
        const nextRefreshToken = response.data?.refresh_token?.trim()
        if (!nextToken || !nextRefreshToken || response.data?.scope !== 'user') {
            return null
        }
        writeAuthTokens(nextToken, nextRefreshToken)
        return nextToken
    } catch {
        return null
    }
}

function refreshAccessTokenOnce(): Promise<string | null> {
    if (!tokenRefreshPromise) {
        tokenRefreshPromise = refreshAccessToken().finally(() => {
            tokenRefreshPromise = null
        })
    }
    return tokenRefreshPromise
}

function shouldRefreshAfterUnauthorized(config?: AxiosRequestConfig): config is RetriableRequestConfig {
    if (!config || !isBackendRequest(config.url) || isAuthSessionRoute(config.url)) return false
    if ((config as RetriableRequestConfig)._authRetry) return false
    return Boolean(readRefreshToken())
}

class Request {
  instance: AxiosInstance
  baseConfig: AxiosRequestConfig = {
    baseURL: backendBaseUrl(),
    timeout: 10000,
    headers: {
      'Content-Type': 'application/json'
    },
    transformRequest: [transformRequestData]
  }
  toast = createToastInterface(globalEventBus)

  constructor(config: AxiosRequestConfig) {
    this.instance = axios.create(Object.assign(this.baseConfig, config))

    this.instance.interceptors.request.use(
      (config) => {
        if (config.data instanceof FormData) {
          deleteHeader(config, 'Content-Type')
        } else if (shouldJsonEncodeBody(config)) {
          setHeader(config, 'Content-Type', 'application/json')
        }

        const token = readAuthToken()
        if (token && isBackendRequest(config.url)) {
          setHeader(config, 'Authorization', createAuthorizationHeader(token))
        }
        return config
      },
      (error) => Promise.reject(error)
    )

    this.instance.interceptors.response.use(
      (response: AxiosResponse) => response,
      async (error: any) => {
          const status = error.response?.status
          const data = error.response?.data

          if (status === 401 && isBackendRequest(error.config?.url)) {
              const retryConfig = error.config as RetriableRequestConfig | undefined
              if (shouldRefreshAfterUnauthorized(retryConfig)) {
                  const nextToken = await refreshAccessTokenOnce()
                  if (nextToken) {
                      retryConfig._authRetry = true
                      setHeader(retryConfig, 'Authorization', createAuthorizationHeader(nextToken))
                      return this.instance.request(retryConfig)
                  }
              }

              this.toast.error(data?.message || 'Unauthorized, please login again.')
              handleLoginExpired()
              return Promise.reject(error)
          }

          if (status === 403) {
              this.toast.error(data?.message || 'Access denied.')
          } else if (status === 500) {
              this.toast.error(data?.message || 'Server internal error.')
          }

          return Promise.reject(error)
      }
    )
  }

  public request(config: AxiosRequestConfig): Promise<AxiosResponse> {
    return this.instance.request(config)
  }
}

export default new Request({})
