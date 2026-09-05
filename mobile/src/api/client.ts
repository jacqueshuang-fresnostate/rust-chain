import axios from 'axios'
import { backendApiUrl } from '@/config/app'
import { BackendConfigurationError } from '@/config/backend'
import { i18n } from '@/i18n'
import {
  normalizeApiError,
  resolveSafeApiErrorMessage,
  type ApiErrorDiagnostic,
} from '@/core/apiError'
import { createApiHttpClient, DEFAULT_API_REQUEST_TIMEOUT_MS } from '@/core/apiRequest'
import {
  createSessionOwner,
  type PersistedSessionEnvelope,
  type SessionLease,
  type SessionPersistence,
  type SessionSnapshot,
  type SessionSyncTransport,
  type SessionTransition,
} from '@/core/sessionOwner'
import {
  installAuthSessionInterceptors,
  type AuthRefreshResult,
  type AuthRequestSession,
} from './requestAuth'

export { publicApiRequestConfig } from './requestAuth'

const ACCESS_TOKEN_KEY = 'hippo_mobile_access_token'
const REFRESH_TOKEN_KEY = 'hippo_mobile_refresh_token'
const SESSION_STORAGE_KEY = 'hippo_mobile_session_v2'
const SESSION_CHANNEL_NAME = 'hippo-mobile-session-v2'
const SESSION_SYNC_EVENT = 'hippo-mobile-session-sync'

export interface SessionInvalidationContext {
  readonly previousScope: string
  readonly currentScope: string
  readonly epoch: number
  readonly external: boolean
  readonly reason: SessionTransition['reason']
}

type SessionInvalidationHook = (context: SessionInvalidationContext) => void

const browserPersistence = createBrowserSessionPersistence()
const browserTransport = createBrowserSessionTransport()
const sessionOwner = createSessionOwner({
  persistence: browserPersistence,
  transport: browserTransport,
})
const sessionInvalidationHooks = new Set<SessionInvalidationHook>()

export const client = createApiHttpClient()

sessionOwner.subscribe((transition) => {
  const context: SessionInvalidationContext = {
    previousScope: transition.previous.scope,
    currentScope: transition.current.scope,
    epoch: transition.current.epoch,
    external: transition.external,
    reason: transition.reason,
  }
  for (const hook of [...sessionInvalidationHooks]) {
    try { hook(context) } catch { /* hooks do not own session state */ }
  }
})

export function readAccessToken(): string {
  return sessionOwner.snapshot().accessToken
}

export function readAuthSessionSnapshot(): SessionSnapshot {
  return sessionOwner.snapshot()
}

export function persistAuthTokens(accessToken: string, refreshToken?: string): void {
  sessionOwner.replace({ accessToken, refreshToken })
}

export function clearAuthTokens(): void {
  sessionOwner.clear('logout')
}

export function synchronizeAuthSession(): SessionSnapshot {
  return sessionOwner.synchronizeFromPersistence()
}

export function startAuthSessionSynchronization(): void {
  sessionOwner.startSynchronization()
}

export function stopAuthSessionSynchronization(): void {
  sessionOwner.stopSynchronization()
}

export function subscribeAuthSession(
  listener: (transition: SessionTransition) => void,
): () => void {
  return sessionOwner.subscribe(listener)
}

/** Registers identity/cache/WS cleanup without exposing token material. */
export function registerSessionInvalidationHook(hook: SessionInvalidationHook): () => void {
  sessionInvalidationHooks.add(hook)
  return () => { sessionInvalidationHooks.delete(hook) }
}

export function apiErrorDiagnostic(error: unknown): ApiErrorDiagnostic {
  return normalizeApiError(error, { offline: isDeviceOffline() }).diagnostic
}

export function apiErrorMessage(
  error: unknown,
  fallback = i18n.global.t('common.serviceUnavailable'),
): string {
  if (error instanceof BackendConfigurationError) {
    return i18n.global.t('common.backendNotConfigured')
  }
  const normalized = normalizeApiError(error, { offline: isDeviceOffline() })
  return resolveSafeApiErrorMessage(
    normalized,
    fallback,
    (key) => i18n.global.t(key),
  )
}

async function refreshAccessToken(session?: AuthRequestSession): Promise<AuthRefreshResult | null> {
  if (!session?.accessToken || !session.refreshToken || !session.scope) return null
  try {
    const response = await axios.post<{
      access_token?: string
      refresh_token?: string
      scope?: string
    }>(requestUrl('/auth/refresh'), {
      refresh_token: session.refreshToken,
    }, {
      timeout: DEFAULT_API_REQUEST_TIMEOUT_MS,
      signal: session.signal,
    })
    const accessToken = response.data.access_token?.trim()
    const refreshToken = response.data.refresh_token?.trim()
    if (!accessToken || !refreshToken || response.data.scope !== 'user') return null

    const committed = sessionOwner.commitRefresh(session, { accessToken, refreshToken })
    if (!committed) return null
    const current = sessionOwner.capture()
    return {
      accessToken: current.accessToken,
      session: requestSessionFromLease(current),
    }
  } catch {
    return null
  }
}

installAuthSessionInterceptors(client, {
  readAccessToken,
  readSession: () => requestSessionFromLease(sessionOwner.capture()),
  isSessionCurrent: (session) => sessionOwner.isCurrent(session),
  refreshAccessToken,
  clearSession: (session) => {
    if (!session) {
      sessionOwner.clear('expired')
      return true
    }
    return Boolean(sessionOwner.clearIfCurrent(session, 'expired'))
  },
  onSessionExpired: () => {
    if (typeof window !== 'undefined') {
      window.dispatchEvent(new Event('hippo-mobile-auth-expired'))
    }
  },
})

export function requestUrl(path: string): string {
  return backendApiUrl(path)
}

function requestSessionFromLease(lease: SessionLease): AuthRequestSession {
  return {
    accessToken: lease.accessToken,
    refreshToken: lease.refreshToken,
    scope: lease.scope,
    epoch: lease.epoch,
    signal: lease.signal,
  }
}

function isDeviceOffline(): boolean {
  return typeof navigator !== 'undefined' && navigator.onLine === false
}

function createBrowserSessionPersistence(): SessionPersistence | undefined {
  if (typeof globalThis.localStorage === 'undefined') return undefined
  return {
    read(): PersistedSessionEnvelope | null {
      const current = parseEnvelope(globalThis.localStorage.getItem(SESSION_STORAGE_KEY))
      if (current) return current

      const accessToken = globalThis.localStorage.getItem(ACCESS_TOKEN_KEY)?.trim() || ''
      const refreshToken = globalThis.localStorage.getItem(REFRESH_TOKEN_KEY)?.trim() || ''
      if (!accessToken) return null
      const migrated: PersistedSessionEnvelope = {
        version: 1,
        accessToken,
        refreshToken,
        scope: opaqueId(),
        epoch: 1,
        revision: 1,
        updatedAt: Date.now(),
        mutationId: opaqueId(),
      }
      this.write(migrated)
      return migrated
    },
    write(envelope): boolean {
      try {
        globalThis.localStorage.setItem(SESSION_STORAGE_KEY, JSON.stringify(envelope))
        globalThis.localStorage.removeItem(ACCESS_TOKEN_KEY)
        globalThis.localStorage.removeItem(REFRESH_TOKEN_KEY)
        return true
      } catch {
        return false
      }
    },
  }
}

function createBrowserSessionTransport(): SessionSyncTransport | undefined {
  if (typeof globalThis.window === 'undefined') return undefined
  return {
    publish(envelope): void {
      if (typeof globalThis.BroadcastChannel !== 'undefined') {
        try {
          const channel = new globalThis.BroadcastChannel(SESSION_CHANNEL_NAME)
          channel.postMessage(envelope)
          channel.close()
        } catch {
          // The storage/custom-event transports still propagate the transition.
        }
      }
      try {
        globalThis.window.dispatchEvent(new CustomEvent(SESSION_SYNC_EVENT, { detail: envelope }))
      } catch {
        // The owner has already committed the local transition.
      }
    },
    subscribe(listener): () => void {
      const cleanups: Array<() => void> = []
      if (typeof globalThis.BroadcastChannel !== 'undefined') {
        try {
          const channel = new globalThis.BroadcastChannel(SESSION_CHANNEL_NAME)
          const onMessage = (event: MessageEvent<unknown>): void => {
            const envelope = parseEnvelope(event.data)
            if (envelope) listener(envelope)
          }
          channel.addEventListener('message', onMessage)
          cleanups.push(() => channel.close())
        } catch {
          // Continue with storage and same-window transport.
        }
      }

      const onStorage = (event: StorageEvent): void => {
        if (event.key !== SESSION_STORAGE_KEY) return
        const envelope = parseEnvelope(event.newValue)
        if (envelope) listener(envelope)
      }
      const onCustom = (event: Event): void => {
        const envelope = parseEnvelope((event as CustomEvent<unknown>).detail)
        if (envelope) listener(envelope)
      }
      globalThis.window.addEventListener('storage', onStorage)
      globalThis.window.addEventListener(SESSION_SYNC_EVENT, onCustom)
      cleanups.push(() => globalThis.window.removeEventListener('storage', onStorage))
      cleanups.push(() => globalThis.window.removeEventListener(SESSION_SYNC_EVENT, onCustom))
      return () => {
        for (const cleanup of cleanups.splice(0)) cleanup()
      }
    },
  }
}

function parseEnvelope(value: unknown): PersistedSessionEnvelope | null {
  let parsed = value
  if (typeof value === 'string') {
    try { parsed = JSON.parse(value) } catch { return null }
  }
  if (!parsed || typeof parsed !== 'object') return null
  const record = parsed as Partial<PersistedSessionEnvelope>
  if (record.version !== 1) return null
  if (
    typeof record.accessToken !== 'string'
    || typeof record.refreshToken !== 'string'
    || typeof record.scope !== 'string'
    || typeof record.epoch !== 'number'
    || typeof record.revision !== 'number'
    || typeof record.updatedAt !== 'number'
    || typeof record.mutationId !== 'string'
  ) return null
  return record as PersistedSessionEnvelope
}

function opaqueId(): string {
  try {
    if (typeof globalThis.crypto?.randomUUID === 'function') return globalThis.crypto.randomUUID()
  } catch {
    // Fall through to a local opaque identifier.
  }
  return `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`
}
