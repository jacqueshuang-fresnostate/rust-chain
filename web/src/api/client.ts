import { authStore, type AuthScope } from '../auth/authStore';
import type { ApiErrorPayload } from './types';

export class ApiError extends Error {
  readonly status: number;
  readonly code: string;

  constructor(status: number, code: string, message: string) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
    this.code = code;
  }
}

const apiBaseUrl = import.meta.env.VITE_API_BASE_URL ?? '';

type ApiRequestInit = RequestInit & {
  authScope?: AuthScope;
};

export async function apiRequest<T = unknown>(path: string, init: ApiRequestInit = {}): Promise<T> {
  const { authScope = 'admin', ...requestInit } = init;
  const headers = new Headers(requestInit.headers);
  const isFormData = typeof FormData !== 'undefined' && requestInit.body instanceof FormData;
  if (!isFormData && !headers.has('Content-Type')) {
    headers.set('Content-Type', 'application/json');
  }

  const session = authStore.getSession(authScope);
  if (session?.accessToken) {
    headers.set('Authorization', `Bearer ${session.accessToken}`);
  }

  const response = await fetch(`${apiBaseUrl}${path}`, { ...requestInit, headers });

  if (!response.ok) {
    const payload = await safeErrorPayload(response);
    if (response.status === 401) {
      authStore.clearSession(authScope);
    }
    throw new ApiError(response.status, payload.code ?? `HTTP_${response.status}`, payload.message ?? response.statusText);
  }

  if (response.status === 204) {
    return undefined as T;
  }

  return (await response.json()) as T;
}

async function safeErrorPayload(response: Response): Promise<ApiErrorPayload> {
  try {
    return (await response.json()) as ApiErrorPayload;
  } catch {
    return { code: `HTTP_${response.status}`, message: response.statusText };
  }
}
