import { authStore } from '../auth/authStore';
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

export async function apiRequest<T = unknown>(path: string, init: RequestInit = {}): Promise<T> {
  const headers = new Headers(init.headers);
  headers.set('Content-Type', 'application/json');

  const session = authStore.getSession();
  if (session?.accessToken) {
    headers.set('Authorization', `Bearer ${session.accessToken}`);
  }

  const response = await fetch(`${apiBaseUrl}${path}`, { ...init, headers });

  if (!response.ok) {
    const payload = await safeErrorPayload(response);
    if (response.status === 401) {
      authStore.clearSession();
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
