import axios, { type AxiosError, type InternalAxiosRequestConfig } from 'axios'
import { backendApiUrl } from '@/config/app'
import { i18n } from '@/i18n'

const ACCESS_TOKEN_KEY = 'hippo_mobile_access_token'
const REFRESH_TOKEN_KEY = 'hippo_mobile_refresh_token'
let refreshPromise: Promise<string | null> | null = null

export const client = axios.create({
  timeout: 12_000,
  headers: {
    'Content-Type': 'application/json',
  },
})

export function readAccessToken(): string {
  try {
    return localStorage.getItem(ACCESS_TOKEN_KEY)?.trim() || ''
  } catch {
    return ''
  }
}

export function persistAuthTokens(accessToken: string, refreshToken?: string): void {
  localStorage.setItem(ACCESS_TOKEN_KEY, accessToken)
  if (refreshToken) localStorage.setItem(REFRESH_TOKEN_KEY, refreshToken)
}

export function clearAuthTokens(): void {
  localStorage.removeItem(ACCESS_TOKEN_KEY)
  localStorage.removeItem(REFRESH_TOKEN_KEY)
}

function readRefreshToken(): string {
  try {
    return localStorage.getItem(REFRESH_TOKEN_KEY)?.trim() || ''
  } catch {
    return ''
  }
}

export function apiErrorMessage(error: unknown, fallback = i18n.global.t('common.serviceUnavailable')): string {
  const axiosError = error as AxiosError<{ message?: string }>
  return axiosError.response?.data?.message || axiosError.message || fallback
}

client.interceptors.request.use((config) => {
  const token = readAccessToken()
  if (token) config.headers.Authorization = `Bearer ${token}`
  return config
})

async function refreshAccessToken(): Promise<string | null> {
  const refreshToken = readRefreshToken()
  if (!refreshToken) return null
  try {
    const response = await axios.post<{ access_token?: string; refresh_token?: string; scope?: string }>(requestUrl('/auth/refresh'), {
      refresh_token: refreshToken,
    }, { timeout: 12_000 })
    const accessToken = response.data.access_token?.trim()
    const nextRefreshToken = response.data.refresh_token?.trim()
    if (!accessToken || !nextRefreshToken || response.data.scope !== 'user') return null
    persistAuthTokens(accessToken, nextRefreshToken)
    return accessToken
  } catch {
    return null
  }
}

function refreshAccessTokenOnce(): Promise<string | null> {
  if (!refreshPromise) {
    refreshPromise = refreshAccessToken().finally(() => { refreshPromise = null })
  }
  return refreshPromise
}

type RetriableRequest = InternalAxiosRequestConfig & { _hippoRetried?: boolean }

client.interceptors.response.use(
  (response) => response,
  async (error: AxiosError) => {
    const request = error.config as RetriableRequest | undefined
    const isRefreshRequest = request?.url?.includes('/auth/refresh')
    const wasAuthenticatedRequest = Boolean(request?.headers?.Authorization)
    if (error.response?.status === 401 && request && !request._hippoRetried && !isRefreshRequest) {
      const nextToken = await refreshAccessTokenOnce()
      if (nextToken) {
        request._hippoRetried = true
        request.headers.Authorization = `Bearer ${nextToken}`
        return client.request(request)
      }
    }
    if (error.response?.status === 401 && wasAuthenticatedRequest) {
      clearAuthTokens()
      window.dispatchEvent(new Event('hippo-mobile-auth-expired'))
    }
    return Promise.reject(error)
  },
)

export function requestUrl(path: string): string {
  return backendApiUrl(path)
}
