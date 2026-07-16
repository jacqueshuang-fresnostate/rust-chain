type PersistedUserStore = {
  token?: unknown
  refreshToken?: unknown
}

const USER_STORE_KEY = 'user'

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

export function writeAuthTokens(token: string, refreshToken?: string) {
  const store = storage()
  if (!store) return

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
}

