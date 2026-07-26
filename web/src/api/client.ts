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

const refreshPaths: Record<AuthScope, string> = {
  admin: '/admin/api/v1/auth/refresh',
  agent: '/agent/api/v1/auth/refresh',
  user: '/api/v1/auth/refresh'
};

const refreshRequests = new Map<AuthScope, Promise<boolean>>();

type ApiRequestInit = RequestInit & {
  authScope?: AuthScope;
};

async function sendRequest(path: string, authScope: AuthScope, requestInit: RequestInit): Promise<Response> {
  const headers = new Headers(requestInit.headers);
  const isFormData = typeof FormData !== 'undefined' && requestInit.body instanceof FormData;
  if (!isFormData && !headers.has('Content-Type')) {
    headers.set('Content-Type', 'application/json');
  }

  const session = authStore.getSession(authScope);
  if (session?.accessToken) {
    headers.set('Authorization', `Bearer ${session.accessToken}`);
  }

  return fetch(`${apiBaseUrl}${path}`, { ...requestInit, headers });
}

async function requestRefresh(authScope: AuthScope): Promise<boolean> {
  const session = authStore.getSession(authScope);
  if (!session?.refreshToken) {
    return false;
  }

  try {
    const response = await fetch(`${apiBaseUrl}${refreshPaths[authScope]}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ refresh_token: session.refreshToken })
    });
    if (!response.ok) {
      return false;
    }

    const tokens = (await response.json()) as { access_token?: string; refresh_token?: string };
    if (!tokens.access_token || !tokens.refresh_token) {
      return false;
    }

    authStore.setSession({ ...session, accessToken: tokens.access_token, refreshToken: tokens.refresh_token });
    return true;
  } catch {
    return false;
  }
}

function refreshSession(authScope: AuthScope): Promise<boolean> {
  const inFlight = refreshRequests.get(authScope);
  if (inFlight) {
    return inFlight;
  }

  const request = requestRefresh(authScope).finally(() => refreshRequests.delete(authScope));
  refreshRequests.set(authScope, request);
  return request;
}

async function parseResponse<T>(response: Response): Promise<T> {
  if (response.status === 204) {
    return undefined as T;
  }

  return (await response.json()) as T;
}

async function toApiError(response: Response): Promise<ApiError> {
  const payload = await safeErrorPayload(response);
  return new ApiError(response.status, payload.code ?? `HTTP_${response.status}`, payload.message ?? response.statusText);
}

export async function apiRequest<T = unknown>(path: string, init: ApiRequestInit = {}): Promise<T> {
  const { authScope = 'admin', ...requestInit } = init;
  const response = await sendRequest(path, authScope, requestInit);
  if (response.ok) {
    return parseResponse<T>(response);
  }

  if (response.status === 401 && authStore.getSession(authScope)) {
    if (await refreshSession(authScope)) {
      const retried = await sendRequest(path, authScope, requestInit);
      if (retried.ok) {
        return parseResponse<T>(retried);
      }
      if (retried.status === 401) {
        authStore.clearSession(authScope);
      }
      throw await toApiError(retried);
    }

    authStore.clearSession(authScope);
  }

  throw await toApiError(response);
}

async function safeErrorPayload(response: Response): Promise<ApiErrorPayload> {
  try {
    return (await response.json()) as ApiErrorPayload;
  } catch {
    return { code: `HTTP_${response.status}`, message: response.statusText };
  }
}
