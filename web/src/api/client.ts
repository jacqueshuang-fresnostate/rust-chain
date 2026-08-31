import { authStore, type AuthScope, type AuthSession } from '../auth/authStore';
import { buildApiUrl } from '../config/backend';
import type { ApiErrorPayload } from './types';

export const DEFAULT_API_TIMEOUT_MS = 15_000;

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

export class ApiTimeoutError extends Error {
  readonly code = 'API_TIMEOUT';
  readonly timeoutMs: number;

  constructor(timeoutMs: number) {
    super(`请求超过 ${timeoutMs}ms 未完成，结果尚未确认`);
    this.name = 'ApiTimeoutError';
    this.timeoutMs = timeoutMs;
  }
}

export class ApiAbortError extends Error {
  readonly code = 'API_ABORTED';

  constructor() {
    super('请求已取消');
    this.name = 'ApiAbortError';
  }
}

export class ApiNetworkError extends Error {
  readonly code = 'API_NETWORK_ERROR';
  readonly cause: unknown;

  constructor(cause: unknown) {
    super('网络连接失败，请检查网络后重试');
    this.name = 'ApiNetworkError';
    this.cause = cause;
  }
}

export class ContractError extends Error {
  readonly code = 'API_CONTRACT_ERROR';
  readonly path: string;
  readonly requestId?: string;
  readonly status?: number;

  constructor(message: string, { path, requestId, status }: { path: string; requestId?: string; status?: number }) {
    super(message);
    this.name = 'ContractError';
    this.path = path;
    this.requestId = requestId;
    this.status = status;
  }
}

const refreshPaths: Record<AuthScope, string> = {
  admin: '/admin/api/v1/auth/refresh',
  agent: '/agent/api/v1/auth/refresh',
  user: '/api/v1/auth/refresh'
};

const refreshRequests = new Map<string, Promise<boolean>>();

export type ApiRequestInit = RequestInit & {
  /** none 用于登录、2FA、login/config 等公开认证启动路径。 */
  auth?: 'none' | 'required';
  authScope?: AuthScope;
  timeoutMs?: number;
};

type RequestSignal = {
  cleanup: () => void;
  didTimeout: () => boolean;
  signal: AbortSignal;
};

function requestId(response: Response): string | undefined {
  return response.headers.get('x-request-id') ?? response.headers.get('request-id') ?? undefined;
}

function createRequestSignal(callerSignal: AbortSignal | null | undefined, timeoutMs: number): RequestSignal {
  const controller = new AbortController();
  let timedOut = false;
  const onCallerAbort = () => controller.abort(callerSignal?.reason);
  if (callerSignal?.aborted) {
    controller.abort(callerSignal.reason);
  } else {
    callerSignal?.addEventListener('abort', onCallerAbort, { once: true });
  }
  const timeout = globalThis.setTimeout(() => {
    timedOut = true;
    controller.abort();
  }, timeoutMs);
  return {
    cleanup: () => {
      globalThis.clearTimeout(timeout);
      callerSignal?.removeEventListener('abort', onCallerAbort);
    },
    didTimeout: () => timedOut,
    signal: controller.signal
  };
}

async function fetchWithLifecycleResult<T>(
  url: string,
  init: RequestInit,
  timeoutMs: number,
  consume: (response: Response) => Promise<T>
): Promise<T> {
  const requestSignal = createRequestSignal(init.signal, timeoutMs);
  try {
    const response = await fetch(url, { ...init, signal: requestSignal.signal });
    // 超时覆盖整个响应体消费期，而不是只覆盖“收到 headers”。
    return await consume(response);
  } catch (error) {
    if (requestSignal.didTimeout()) throw new ApiTimeoutError(timeoutMs);
    if (requestSignal.signal.aborted) throw new ApiAbortError();
    if (error instanceof ApiError || error instanceof ContractError) throw error;
    throw new ApiNetworkError(error);
  } finally {
    requestSignal.cleanup();
  }
}

type RequestResult<T> =
  | { ok: true; value: T }
  | { error: ApiError; ok: false; status: number };

async function sendRequest<T>(
  path: string,
  authScope: AuthScope,
  auth: 'none' | 'required',
  requestInit: RequestInit,
  timeoutMs: number
): Promise<RequestResult<T>> {
  const headers = new Headers(requestInit.headers);
  const isFormData = typeof FormData !== 'undefined' && requestInit.body instanceof FormData;
  if (!isFormData && requestInit.body !== undefined && requestInit.body !== null && !headers.has('Content-Type')) {
    headers.set('Content-Type', 'application/json');
  }

  if (auth === 'required') {
    const session = authStore.getSession(authScope);
    if (session?.accessToken) headers.set('Authorization', `Bearer ${session.accessToken}`);
  } else {
    headers.delete('Authorization');
  }

  return fetchWithLifecycleResult(buildApiUrl(path), { ...requestInit, headers }, timeoutMs, async (response) => {
    if (response.ok) return { ok: true, value: await parseResponse<T>(path, response) };
    return { error: await toApiError(response), ok: false, status: response.status };
  });
}

function isRefreshTokens(value: unknown): value is { access_token: string; refresh_token: string } {
  if (!value || typeof value !== 'object') return false;
  const tokens = value as Record<string, unknown>;
  return typeof tokens.access_token === 'string' && tokens.access_token.length > 0 && typeof tokens.refresh_token === 'string' && tokens.refresh_token.length > 0;
}

async function requestRefresh(authScope: AuthScope, captured: AuthSession, timeoutMs: number): Promise<boolean> {
  try {
    const tokens = await fetchWithLifecycleResult(
      buildApiUrl(refreshPaths[authScope]),
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ refresh_token: captured.refreshToken })
      },
      timeoutMs,
      async (response) => {
        if (!response.ok) return null;
        try {
          return (await response.json()) as unknown;
        } catch (error) {
          if (error instanceof SyntaxError) return null;
          throw error;
        }
      }
    );
    if (!isRefreshTokens(tokens)) return false;
    return authStore.compareAndSetSession(authScope, captured.generation, captured.refreshToken, {
      accessToken: tokens.access_token,
      refreshToken: tokens.refresh_token
    });
  } catch {
    return false;
  }
}

function refreshSession(authScope: AuthScope, captured: AuthSession, timeoutMs: number): Promise<boolean> {
  const refreshKey = `${authScope}:${captured.generation}`;
  const inFlight = refreshRequests.get(refreshKey);
  if (inFlight) return inFlight;
  const request = requestRefresh(authScope, captured, timeoutMs).finally(() => refreshRequests.delete(refreshKey));
  refreshRequests.set(refreshKey, request);
  return request;
}

async function parseResponse<T>(path: string, response: Response): Promise<T> {
  if (response.status === 204) return undefined as T;
  try {
    return (await response.json()) as T;
  } catch (error) {
    // 语法错误是 DTO/媒体合约问题；响应体断流则交给请求生命周期分类为网络或超时。
    if (!(error instanceof SyntaxError)) throw error;
    throw new ContractError('服务端返回了无效 JSON', { path, requestId: requestId(response), status: response.status });
  }
}

async function toApiError(response: Response): Promise<ApiError> {
  const payload = await safeErrorPayload(response);
  return new ApiError(response.status, payload.code ?? `HTTP_${response.status}`, payload.message ?? (response.statusText || `HTTP ${response.status}`));
}

export async function apiRequest<T = unknown>(path: string, init: ApiRequestInit = {}): Promise<T> {
  const {
    auth = 'required',
    authScope = 'admin',
    timeoutMs = DEFAULT_API_TIMEOUT_MS,
    ...requestInit
  } = init;
  if (!Number.isFinite(timeoutMs) || timeoutMs <= 0) throw new Error('timeoutMs 必须为正数');

  const capturedSession = auth === 'required' ? authStore.getSession(authScope) : null;
  const response = await sendRequest<T>(path, authScope, auth, requestInit, timeoutMs);
  if (response.ok) return response.value;

  if (response.status === 401 && auth === 'required' && capturedSession) {
    const current = authStore.getSession(authScope);
    // 旧请求的 401 不得刷新或清除新登录会话。
    if (current?.generation !== capturedSession.generation) throw response.error;

    if (await refreshSession(authScope, capturedSession, timeoutMs)) {
      const retried = await sendRequest<T>(path, authScope, auth, requestInit, timeoutMs);
      if (retried.ok) return retried.value;
      if (retried.status === 401) authStore.clearSession(authScope, capturedSession.generation);
      throw retried.error;
    }

    authStore.clearSession(authScope, capturedSession.generation);
  }

  throw response.error;
}

async function safeErrorPayload(response: Response): Promise<ApiErrorPayload> {
  let value: unknown;
  try {
    value = await response.json();
  } catch (error) {
    if (!(error instanceof SyntaxError)) throw error;
    return { code: `HTTP_${response.status}`, message: response.statusText || `HTTP ${response.status}` };
  }
  if (!value || typeof value !== 'object') {
    return { code: `HTTP_${response.status}`, message: response.statusText || `HTTP ${response.status}` };
  }
  const payload = value as Record<string, unknown>;
  return {
    code: typeof payload.code === 'string' ? payload.code : undefined,
    message: typeof payload.message === 'string' ? payload.message : undefined
  };
}
