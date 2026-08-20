import {
  resolveBackendApiUrl,
  resolveBackendHealthUrl,
  resolvePrivateUserWebSocketUrl,
  resolveBackendRuntimeConfig,
  resolveBackendWebSocketUrl,
} from './backend'
import { resolveProductBackendOrigin } from './product'

const env = import.meta.env
const backend = resolveBackendRuntimeConfig({
  apiDomain: resolveProductBackendOrigin(env.VITE_BACKEND_API_DOMAIN),
  apiPrefix: env.VITE_BACKEND_API_PREFIX,
  dev: env.DEV,
  native: env.MODE === 'tauri' || Boolean(env.TAURI_ENV_PLATFORM),
})

export const APP_CONFIG = {
  backend,
  fallbackBrand: 'Hippo',
}

export function backendApiUrl(path: string): string {
  return resolveBackendApiUrl(APP_CONFIG.backend, path)
}

export function backendHealthUrl(): string {
  return resolveBackendHealthUrl(APP_CONFIG.backend)
}

export function publicMarketWebSocketUrl(pageOrigin = typeof window === 'undefined' ? '' : window.location.origin): string {
  return resolveBackendWebSocketUrl(APP_CONFIG.backend, '/ws/public', pageOrigin)
}

export function privateUserWebSocketUrl(
  accessToken: string,
  pageOrigin = typeof window === 'undefined' ? '' : window.location.origin,
): string | null {
  return resolvePrivateUserWebSocketUrl(APP_CONFIG.backend, accessToken, pageOrigin)
}
