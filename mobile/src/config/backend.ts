export const DEFAULT_BACKEND_API_PREFIX = '/api/v1'
export const DEFAULT_BACKEND_DEV_PROXY_TARGET = 'http://127.0.0.1:8080'

export class BackendConfigurationError extends Error {
  readonly code = 'BACKEND_CONFIGURATION_ERROR'

  constructor(message: string) {
    super(message)
    this.name = 'BackendConfigurationError'
  }
}

export interface BackendRuntimeInput {
  apiDomain?: string
  apiPrefix?: string
  dev?: boolean
  native?: boolean
}

export interface BackendRuntimeConfig {
  apiOrigin: string
  apiPrefix: string
  configurationError?: BackendConfigurationError
}

export function normalizeBackendApiPrefix(value?: string): string {
  const normalized = String(value || DEFAULT_BACKEND_API_PREFIX).trim().replace(/^\/+|\/+$/g, '')
  if (!normalized) return DEFAULT_BACKEND_API_PREFIX
  return `/${normalized}`
}

export function resolveBackendRuntimeConfig(input: BackendRuntimeInput): BackendRuntimeConfig {
  const apiPrefix = normalizeBackendApiPrefix(input.apiPrefix)
  const apiDomain = String(input.apiDomain || '').trim()
  const production = !input.dev

  if (input.dev) {
    return { apiOrigin: '', apiPrefix }
  }

  if (!apiDomain) {
    if (input.native && production) {
      return {
        apiOrigin: '',
        apiPrefix,
        configurationError: new BackendConfigurationError(
          'Backend API origin is not configured for the Tauri production build. Set VITE_BACKEND_API_DOMAIN to a reachable HTTPS origin.',
        ),
      }
    }
    return { apiOrigin: '', apiPrefix }
  }

  try {
    const apiOrigin = normalizeBackendOrigin(apiDomain)
    const url = new URL(apiOrigin)
    if (production && isLoopbackHostname(url.hostname)) {
      throw new BackendConfigurationError(
        'A production backend origin cannot target localhost or a loopback address.',
      )
    }
    if (production && url.protocol !== 'https:') {
      throw new BackendConfigurationError('A production backend origin must use HTTPS.')
    }
    return { apiOrigin, apiPrefix }
  } catch (error) {
    return {
      apiOrigin: '',
      apiPrefix,
      configurationError: error instanceof BackendConfigurationError
        ? error
        : new BackendConfigurationError('VITE_BACKEND_API_DOMAIN must be a valid HTTP or HTTPS origin.'),
    }
  }
}

export function resolveBackendDevProxyTarget(value?: string): string {
  return normalizeBackendOrigin(String(value || DEFAULT_BACKEND_DEV_PROXY_TARGET).trim())
}

export function resolveBackendApiUrl(config: BackendRuntimeConfig, path: string): string {
  assertBackendConfigured(config)
  return `${config.apiOrigin}${config.apiPrefix}${normalizePath(path)}`
}

export function resolveBackendHealthUrl(config: BackendRuntimeConfig): string {
  assertBackendConfigured(config)
  return `${config.apiOrigin}/health`
}

export function resolveBackendWebSocketUrl(
  config: BackendRuntimeConfig,
  path: string,
  pageOrigin?: string,
): string {
  assertBackendConfigured(config)
  const baseOrigin = config.apiOrigin || String(pageOrigin || '').trim()
  if (!baseOrigin) {
    throw new BackendConfigurationError('The current page origin is unavailable for the WebSocket connection.')
  }
  const url = new URL(`${config.apiPrefix}${normalizePath(path)}`, normalizeBackendOrigin(baseOrigin))
  url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:'
  return url.toString()
}

function assertBackendConfigured(config: BackendRuntimeConfig): void {
  if (config.configurationError) throw config.configurationError
}

function normalizeBackendOrigin(value: string): string {
  let url: URL
  try {
    url = new URL(value)
  } catch {
    throw new BackendConfigurationError('Backend origin must be an absolute HTTP or HTTPS URL.')
  }
  if (url.protocol !== 'http:' && url.protocol !== 'https:') {
    throw new BackendConfigurationError('Backend origin must use HTTP or HTTPS.')
  }
  if (url.username || url.password || url.search || url.hash || (url.pathname && url.pathname !== '/')) {
    throw new BackendConfigurationError('Backend origin must not include credentials, a path, query, or fragment.')
  }
  return url.origin
}

function isLoopbackHostname(hostname: string): boolean {
  const normalized = hostname.trim().toLowerCase().replace(/^\[|\]$/g, '')
  return normalized === 'localhost'
    || normalized === '::1'
    || normalized === '0.0.0.0'
    || normalized.startsWith('127.')
}

function normalizePath(path: string): string {
  const normalized = String(path || '').trim()
  return normalized.startsWith('/') ? normalized : `/${normalized}`
}
