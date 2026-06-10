import axios, { type AxiosInstance, type AxiosRequestConfig, type AxiosResponse } from 'axios'
import { createToastInterface, globalEventBus } from 'vue-toastification'
import { APP_CONFIG } from '@/config/app.ts'
import { createAuthorizationHeader } from './backendAdapters'

function handleLoginExpired() {
    localStorage.removeItem('token')
    localStorage.removeItem('refresh_token')
    localStorage.removeItem('user')
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
        if (shouldJsonEncodeBody(config)) {
          setHeader(config, 'Content-Type', 'application/json')
        }

        const token = localStorage.getItem('token')
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