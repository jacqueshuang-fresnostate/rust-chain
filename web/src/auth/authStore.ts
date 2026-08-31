export type AuthScope = 'admin' | 'agent' | 'user';

export interface AuthSession {
  accessToken: string;
  /** 每次交互式登录生成，refresh 只能 CAS 更新同一代会话。 */
  generation: string;
  refreshToken: string;
  scope: AuthScope;
  subject: string;
}

export type AuthSessionInput = Omit<AuthSession, 'generation'> & { generation?: string };

export const SESSION_STORAGE_KEY = 'exchange_admin_session';
export const AGENT_SESSION_STORAGE_KEY = 'exchange_agent_session';
export const AUTH_SYNC_STORAGE_KEY = 'exchange_auth_session_signal_v1';

const authScopes = new Set<AuthScope>(['admin', 'agent', 'user']);
const listeners = new Set<() => void>();
const sessionCache = new Map<string, { raw: string | null; session: AuthSession | null }>();
const sourceId = globalThis.crypto?.randomUUID?.() ?? `tab-${Date.now()}-${Math.random().toString(36).slice(2)}`;

type AuthSyncEvent = {
  at: number;
  generation?: string;
  scope: AuthScope;
  sourceId: string;
  type: 'cleared' | 'replaced';
};

let broadcastChannel: BroadcastChannel | null = null;

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0;
}

function isAuthScope(value: unknown): value is AuthScope {
  return typeof value === 'string' && authScopes.has(value as AuthScope);
}

function newGeneration(): string {
  return globalThis.crypto?.randomUUID?.() ?? `generation-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

/**
 * 登录旧响应未显式返回 subject 时，只从 JWT payload 提取 sub 用于缓存命名。
 * 该值不参与授权决策；真实身份仍由后端验签和 access/me 决定。
 */
export function authSubjectFromAccessToken(accessToken: string, scope: AuthScope): string {
  try {
    const payloadPart = accessToken.split('.')[1];
    if (!payloadPart) return `${scope}:unknown`;
    const normalized = payloadPart.replaceAll('-', '+').replaceAll('_', '/');
    const padded = normalized.padEnd(Math.ceil(normalized.length / 4) * 4, '=');
    const payload = JSON.parse(globalThis.atob(padded)) as { sub?: unknown };
    if (isNonEmptyString(payload.sub) && payload.sub.startsWith(`${scope}:`)) return payload.sub;
  } catch {
    // 非 JWT 的兼容 token 仍由 generation 完成会话隔离。
  }
  return `${scope}:unknown`;
}

function parseSession(raw: string | null): AuthSession | null {
  if (!raw) return null;
  try {
    const value = JSON.parse(raw) as Partial<AuthSession>;
    if (!isNonEmptyString(value.accessToken) || !isNonEmptyString(value.refreshToken) || !isAuthScope(value.scope) || !isNonEmptyString(value.subject)) {
      return null;
    }
    return {
      accessToken: value.accessToken,
      generation: isNonEmptyString(value.generation) ? value.generation : newGeneration(),
      refreshToken: value.refreshToken,
      scope: value.scope,
      subject: value.subject
    };
  } catch {
    return null;
  }
}

function storageKeyForScope(scope: AuthScope = 'admin'): string {
  return scope === 'agent' ? AGENT_SESSION_STORAGE_KEY : SESSION_STORAGE_KEY;
}

function safeSessionStorage(): Storage | null {
  try {
    return typeof window === 'undefined' ? null : window.sessionStorage;
  } catch {
    return null;
  }
}

function safeLocalStorage(): Storage | null {
  try {
    return typeof window === 'undefined' ? null : window.localStorage;
  } catch {
    return null;
  }
}

/** 从旧 localStorage 一次性迁移，使已部署会话在升级后不会立即丢失。 */
function migrateLegacySession(key: string): string | null {
  const sessionStorage = safeSessionStorage();
  if (!sessionStorage) return null;
  const current = sessionStorage.getItem(key);
  if (current) return current;

  const localStorage = safeLocalStorage();
  const legacyRaw = localStorage?.getItem(key) ?? null;
  if (!legacyRaw) return null;
  const legacy = parseSession(legacyRaw);
  localStorage?.removeItem(key);
  if (!legacy) return null;
  const migratedRaw = JSON.stringify(legacy);
  sessionStorage.setItem(key, migratedRaw);
  return migratedRaw;
}

function readSession(key: string): AuthSession | null {
  const storage = safeSessionStorage();
  const raw = storage?.getItem(key) ?? migrateLegacySession(key);
  const cached = sessionCache.get(key);
  if (cached && cached.raw === raw) return cached.session;

  const session = parseSession(raw);
  // 也升级早期 sessionStorage 中没有 generation 的数据。
  const normalizedRaw = session ? JSON.stringify(session) : raw;
  if (session && normalizedRaw && raw !== normalizedRaw) storage?.setItem(key, normalizedRaw);
  sessionCache.set(key, { raw: session ? normalizedRaw : raw, session });
  return session;
}

function notifyListeners(): void {
  listeners.forEach((listener) => listener());
}

function parseSyncEvent(value: unknown): AuthSyncEvent | null {
  if (!value || typeof value !== 'object') return null;
  const event = value as Partial<AuthSyncEvent>;
  if (
    typeof event.at !== 'number' ||
    !isAuthScope(event.scope) ||
    !isNonEmptyString(event.sourceId) ||
    (event.type !== 'cleared' && event.type !== 'replaced')
  ) {
    return null;
  }
  return event as AuthSyncEvent;
}

function handleExternalEvent(rawEvent: unknown): void {
  const event = parseSyncEvent(rawEvent);
  if (!event || event.sourceId === sourceId) return;
  const key = storageKeyForScope(event.scope);
  const current = readSession(key);
  if (!current) return;
  // 其他标签登录了新会话或退出时，本标签只清除自己的令牌，不在标签间传播令牌。
  safeSessionStorage()?.removeItem(key);
  sessionCache.delete(key);
  notifyListeners();
}

function ensureBroadcastChannel(): BroadcastChannel | null {
  if (broadcastChannel || typeof BroadcastChannel === 'undefined') return broadcastChannel;
  broadcastChannel = new BroadcastChannel('exchange-auth-session-v1');
  broadcastChannel.addEventListener('message', (message: MessageEvent<unknown>) => handleExternalEvent(message.data));
  return broadcastChannel;
}

function publish(event: Omit<AuthSyncEvent, 'at' | 'sourceId'>): void {
  const message: AuthSyncEvent = { ...event, at: Date.now(), sourceId };
  ensureBroadcastChannel()?.postMessage(message);
  const localStorage = safeLocalStorage();
  if (!localStorage) return;
  // storage 事件只携带无敏感信号，不持久化 access/refresh token。
  localStorage.setItem(AUTH_SYNC_STORAGE_KEY, JSON.stringify(message));
  localStorage.removeItem(AUTH_SYNC_STORAGE_KEY);
}

if (typeof window !== 'undefined') {
  window.addEventListener('storage', (event) => {
    if (event.key !== AUTH_SYNC_STORAGE_KEY || !event.newValue) return;
    try {
      handleExternalEvent(JSON.parse(event.newValue));
    } catch {
      // 非法的同源 storage 消息不得影响当前会话。
    }
  });
}

export const authStore = {
  getSession(scope: AuthScope = 'admin'): AuthSession | null {
    return readSession(storageKeyForScope(scope));
  },

  setSession(input: AuthSessionInput): AuthSession {
    const session: AuthSession = { ...input, generation: input.generation?.trim() || newGeneration() };
    const key = storageKeyForScope(session.scope);
    const raw = JSON.stringify(session);
    safeSessionStorage()?.setItem(key, raw);
    // 即使新版已存入 sessionStorage，也清理可能残留的旧长期副本。
    safeLocalStorage()?.removeItem(key);
    sessionCache.set(key, { raw, session });
    notifyListeners();
    publish({ generation: session.generation, scope: session.scope, type: 'replaced' });
    return session;
  },

  compareAndSetSession(
    scope: AuthScope,
    expectedGeneration: string,
    expectedRefreshToken: string,
    tokens: { accessToken: string; refreshToken: string }
  ): boolean {
    const current = this.getSession(scope);
    if (!current || current.generation !== expectedGeneration || current.refreshToken !== expectedRefreshToken) return false;
    const next: AuthSession = { ...current, ...tokens };
    const key = storageKeyForScope(scope);
    const raw = JSON.stringify(next);
    safeSessionStorage()?.setItem(key, raw);
    sessionCache.set(key, { raw, session: next });
    notifyListeners();
    return true;
  },

  clearSession(scope: AuthScope = 'admin', expectedGeneration?: string): boolean {
    const current = this.getSession(scope);
    if (expectedGeneration && current?.generation !== expectedGeneration) return false;
    const key = storageKeyForScope(scope);
    safeSessionStorage()?.removeItem(key);
    safeLocalStorage()?.removeItem(key);
    sessionCache.delete(key);
    notifyListeners();
    publish({ generation: current?.generation, scope, type: 'cleared' });
    return true;
  },

  subscribe(listener: () => void): () => void {
    listeners.add(listener);
    ensureBroadcastChannel();
    return () => listeners.delete(listener);
  }
};
