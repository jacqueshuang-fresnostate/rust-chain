const env = import.meta.env

export const APP_CONFIG = {
  // 开发期经 Vite 同源代理转发，避免浏览器端跨域阻断；原生与生产构建使用部署注入的后端域名。
  backendDomain: (env.VITE_BACKEND_API_DOMAIN || (env.DEV ? '' : 'http://127.0.0.1:8080')).replace(/\/$/, ''),
  backendPrefix: (env.VITE_BACKEND_API_PREFIX || '/api/v1').replace(/\/$/, ''),
  fallbackBrand: 'Hippo',
}

export function backendApiUrl(path: string): string {
  const suffix = path.startsWith('/') ? path : `/${path}`
  return `${APP_CONFIG.backendDomain}${APP_CONFIG.backendPrefix}${suffix}`
}
