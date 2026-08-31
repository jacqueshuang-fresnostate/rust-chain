type BackendEnvironment = {
  VITE_API_BASE_URL?: string;
  VITE_API_SAME_ORIGIN?: string;
};

export type BackendRuntimeConfig = {
  apiBaseUrl: string;
  mode: 'absolute' | 'same-origin';
};

function normalizedFlag(value: string | undefined): string {
  return value?.trim().toLowerCase() ?? '';
}

function normalizedApiBaseUrl(value: string, production: boolean): string {
  let url: URL;
  try {
    url = new URL(value);
  } catch {
    throw new Error('VITE_API_BASE_URL 必须是完整的 HTTP(S) 地址');
  }

  if (url.protocol !== 'http:' && url.protocol !== 'https:') {
    throw new Error('VITE_API_BASE_URL 仅支持 HTTP(S) 协议');
  }
  if (url.username || url.password || url.search || url.hash) {
    throw new Error('VITE_API_BASE_URL 不得包含账号、查询参数或哈希');
  }
  if (url.pathname !== '/' && url.pathname !== '') {
    throw new Error('VITE_API_BASE_URL 必须是纯 Origin，不得包含路径');
  }
  const loopback = url.hostname === 'localhost' || url.hostname === '127.0.0.1' || url.hostname === '[::1]';
  if (production && url.protocol !== 'https:' && !loopback) {
    throw new Error('生产环境 VITE_API_BASE_URL 必须使用 HTTPS');
  }
  return url.origin;
}

/**
 * REST 与 WebSocket 共用唯一后端模式开关。
 * - 同源：VITE_API_SAME_ORIGIN=true，且不得同时指定 VITE_API_BASE_URL。
 * - 独立后端：VITE_API_SAME_ORIGIN=false，且必须指定完整 VITE_API_BASE_URL。
 */
export function resolveBackendRuntimeConfig(
  environment: BackendEnvironment,
  { production = false }: { production?: boolean } = {}
): BackendRuntimeConfig {
  const sameOrigin = normalizedFlag(environment.VITE_API_SAME_ORIGIN);
  const baseUrl = environment.VITE_API_BASE_URL?.trim() ?? '';

  if (sameOrigin === 'true') {
    if (baseUrl) throw new Error('同源模式不得同时设置 VITE_API_BASE_URL');
    return { apiBaseUrl: '', mode: 'same-origin' };
  }
  if (sameOrigin !== 'false') {
    throw new Error('VITE_API_SAME_ORIGIN 必须显式设置为 true 或 false');
  }
  if (!baseUrl) throw new Error('非同源模式必须设置 VITE_API_BASE_URL');
  return { apiBaseUrl: normalizedApiBaseUrl(baseUrl, production), mode: 'absolute' };
}

export const backendRuntimeConfig = resolveBackendRuntimeConfig({
  VITE_API_BASE_URL: import.meta.env.VITE_API_BASE_URL,
  VITE_API_SAME_ORIGIN: import.meta.env.VITE_API_SAME_ORIGIN
}, {
  production: import.meta.env.PROD
});

export function buildApiUrl(path: string): string {
  if (!path.startsWith('/')) throw new Error('API 路径必须以 / 开头');
  return `${backendRuntimeConfig.apiBaseUrl}${path}`;
}

export function buildWebSocketUrl(path: string, browserLocation: Pick<Location, 'href'> | undefined = globalThis.location): string {
  if (!path.startsWith('/')) throw new Error('WebSocket 路径必须以 / 开头');
  const base = backendRuntimeConfig.mode === 'absolute' ? backendRuntimeConfig.apiBaseUrl : browserLocation?.href;
  if (!base) throw new Error('同源 WebSocket 模式需要浏览器地址');
  const url = new URL(path, base);
  url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
  return url.toString();
}
