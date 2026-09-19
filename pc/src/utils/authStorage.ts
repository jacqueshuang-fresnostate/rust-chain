type PersistedUserStore = {
  token?: unknown
  refreshToken?: unknown
}

const USER_STORE_KEY = 'user'
const SESSION_SCOPE_KEY = 'hippo_pc_session_scope'

function storage(): Storage | null {
  try {
    return globalThis.localStorage || null
  } catch {
    return null
  }
}

function stringValue(value: unknown): string {
  return typeof value === 'string' ? value.trim() : ''
}

function readPersistedUserStore(): PersistedUserStore | null {
  const store = storage()
  if (!store) return null

  try {
    const raw = store.getItem(USER_STORE_KEY)
    if (!raw) return null
    const parsed = JSON.parse(raw)
    return parsed && typeof parsed === 'object' ? parsed as PersistedUserStore : null
  } catch {
    return null
  }
}

function writePersistedUserToken(token: string) {
  const store = storage()
  if (!store) return

  try {
    const persisted = readPersistedUserStore()
    if (!persisted) return
    store.setItem(USER_STORE_KEY, JSON.stringify({
      ...persisted,
      token,
    }))
  } catch {
    // Best-effort sync only; the standalone token remains the source of truth.
  }
}

export function readAuthToken(): string {
  const store = storage()
  const standaloneToken = stringValue(store?.getItem('token'))
  if (standaloneToken) return standaloneToken

  return stringValue(readPersistedUserStore()?.token)
}

export function readRefreshToken(): string {
  const store = storage()
  const standaloneRefreshToken = stringValue(store?.getItem('refresh_token'))
  if (standaloneRefreshToken) return standaloneRefreshToken

  return stringValue(readPersistedUserStore()?.refreshToken)
}

/** 登录创建身份边界；刷新只换令牌，保留结果未知的资金请求身份。 */
export function readAuthSessionScope(): string {
  if (!readAuthToken()) throw new Error('authenticated session is required')
  const store = storage()
  if (!store) throw new Error('session persistence is required')
  const current = store.getItem(SESSION_SCOPE_KEY)
  if (current) return current
  const scope = globalThis.crypto.randomUUID()
  store.setItem(SESSION_SCOPE_KEY, scope)
  return scope
}

export function writeAuthTokens(token: string, refreshToken?: string, newSession = false) {
  const store = storage()
  if (!store) return

  if (newSession) store.setItem(SESSION_SCOPE_KEY, globalThis.crypto.randomUUID())
  store.setItem('token', token)
  if (refreshToken) {
    store.setItem('refresh_token', refreshToken)
  }
  writePersistedUserToken(token)
}

export function clearAuthStorage() {
  const store = storage()
  if (!store) return

  store.removeItem('token')
  store.removeItem('refresh_token')
  store.removeItem(USER_STORE_KEY)
  store.removeItem(SESSION_SCOPE_KEY)
}
