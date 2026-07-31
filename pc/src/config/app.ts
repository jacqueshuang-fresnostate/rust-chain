const env = (import.meta as { env?: Record<string, string | undefined> }).env ?? {}

export const APP_CONFIG = {
    BACKEND_API_DOMAIN: env.VITE_BACKEND_API_DOMAIN || 'https://hipoex.cllbmz.kdns.fr',
    BACKEND_API_PREFIX: env.VITE_BACKEND_API_PREFIX || '/api/v1',
    DEFAULT_PROMOTION_CODE: '66666'
}
