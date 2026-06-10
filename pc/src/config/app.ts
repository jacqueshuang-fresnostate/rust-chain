const env = (import.meta as { env?: Record<string, string | undefined> }).env ?? {}

export const APP_CONFIG = {
    BACKEND_API_DOMAIN: env.VITE_BACKEND_API_DOMAIN || 'http://127.0.0.1:8080',
    BACKEND_API_PREFIX: env.VITE_BACKEND_API_PREFIX || '/api/v1',
    DEFAULT_PROMOTION_CODE: '66666'
}
