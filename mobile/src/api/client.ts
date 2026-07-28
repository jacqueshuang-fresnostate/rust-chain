import axios, { type AxiosError } from 'axios'
import { backendApiUrl } from '@/config/app'
import { BackendConfigurationError } from '@/config/backend'
import { i18n } from '@/i18n'
import { installAuthSessionInterceptors } from './requestAuth'

const ACCESS_TOKEN_KEY = 'hippo_mobile_access_token'
const REFRESH_TOKEN_KEY = 'hippo_mobile_refresh_token'

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
  if (refreshToken?.trim()) localStorage.setItem(REFRESH_TOKEN_KEY, refreshToken)
  else localStorage.removeItem(REFRESH_TOKEN_KEY)
}

export function clearAuthTokens(): void {
  try {
    localStorage.removeItem(ACCESS_TOKEN_KEY)
    localStorage.removeItem(REFRESH_TOKEN_KEY)
  } catch {
    // Storage may be unavailable in a restricted WebView; in-memory session state still clears.
  }
}

function readRefreshToken(): string {
  try {
    return localStorage.getItem(REFRESH_TOKEN_KEY)?.trim() || ''
  } catch {
    return ''
  }
}

export function apiErrorMessage(error: unknown, fallback = i18n.global.t('common.serviceUnavailable')): string {
  if (error instanceof BackendConfigurationError) {
    return i18n.global.t('common.backendNotConfigured')
  }
  const axiosError = error as AxiosError<{ message?: string }>
  if (axiosError.response?.data?.message) return axiosError.response.data.message
  if (axiosError.code === 'ECONNABORTED' || axiosError.code === 'ETIMEDOUT') {
    return i18n.global.t('common.requestTimeout')
  }
  if (axios.isAxiosError(error) && !axiosError.response) {
    if (typeof navigator !== 'undefined' && navigator.onLine === false) {
      return i18n.global.t('common.deviceOffline')
    }
    return i18n.global.t('common.networkUnavailable')
  }
  return error instanceof Error && error.message ? error.message : fallback
}

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

installAuthSessionInterceptors(client, {
  readAccessToken,
  refreshAccessToken,
  clearSession: clearAuthTokens,
  onSessionExpired: () => {
    if (typeof window !== 'undefined') {
      window.dispatchEvent(new Event('hippo-mobile-auth-expired'))
    }
  },
})

export function requestUrl(path: string): string {
  return backendApiUrl(path)
}
