export type SessionPersistenceState = 'persistent' | 'memory'
export type SessionTransitionReason = 'login' | 'refresh' | 'logout' | 'expired' | 'external'

export interface PersistedSessionEnvelope {
  readonly version: 1
  readonly accessToken: string
  readonly refreshToken: string
  readonly scope: string
  readonly epoch: number
  readonly revision: number
  readonly updatedAt: number
  readonly mutationId: string
}

export interface SessionSnapshot {
  readonly accessToken: string
  readonly refreshToken: string
  /** Opaque identity boundary. It never contains an access token. */
  readonly scope: string
  /** Request generation. Every token transition advances it. */
  readonly epoch: number
  readonly revision: number
  readonly updatedAt: number
  readonly persistence: SessionPersistenceState
}

export interface SessionLease extends SessionSnapshot {
  readonly signal: AbortSignal
}

export interface SessionTransition {
  readonly previous: SessionSnapshot
  readonly current: SessionSnapshot
  readonly reason: SessionTransitionReason
  readonly external: boolean
}

export interface SessionPersistence {
  read(): PersistedSessionEnvelope | null
  /** False means the in-memory session remains active but was not persisted. */
  write(envelope: PersistedSessionEnvelope): boolean
}

export interface SessionSyncTransport {
  publish(envelope: PersistedSessionEnvelope): void
  subscribe(listener: (envelope: PersistedSessionEnvelope) => void): () => void
}

export interface SessionOwnerOptions {
  persistence?: SessionPersistence
  transport?: SessionSyncTransport
  now?: () => number
  createId?: () => string
}

export interface SessionOwner {
  snapshot(): SessionSnapshot
  capture(): SessionLease
  isCurrent(lease: Pick<SessionLease, 'scope' | 'epoch' | 'accessToken'>): boolean
  replace(tokens: { accessToken: string; refreshToken?: string }): SessionSnapshot
  commitRefresh(
    lease: Pick<SessionLease, 'scope' | 'epoch' | 'accessToken'>,
    tokens: { accessToken: string; refreshToken: string },
  ): SessionSnapshot | null
  clear(reason?: 'logout' | 'expired'): SessionSnapshot
  clearIfCurrent(
    lease: Pick<SessionLease, 'scope' | 'epoch' | 'accessToken'>,
    reason?: 'logout' | 'expired',
  ): SessionSnapshot | null
  subscribe(listener: (transition: SessionTransition) => void): () => void
  synchronizeFromPersistence(): SessionSnapshot
  startSynchronization(): void
  stopSynchronization(): void
}

type InternalState = SessionSnapshot & {
  readonly mutationId: string
}

const EMPTY_ENVELOPE: PersistedSessionEnvelope = {
  version: 1,
  accessToken: '',
  refreshToken: '',
  scope: '',
  epoch: 0,
  revision: 0,
  updatedAt: 0,
  mutationId: '',
}

/**
 * Single owner for token persistence and request generations. Token refresh is
 * a compare-and-swap against a captured lease, so logout or a newer login can
 * never be overwritten by a late refresh response.
 */
export function createSessionOwner(options: SessionOwnerOptions = {}): SessionOwner {
  const now = options.now ?? Date.now
  const createId = options.createId ?? defaultCreateId
  const persistence = options.persistence
  const listeners = new Set<(transition: SessionTransition) => void>()

  const initialEnvelope = safeRead(persistence) ?? EMPTY_ENVELOPE
  let state: InternalState = stateFromEnvelope(
    initialEnvelope,
    persistence && initialEnvelope !== EMPTY_ENVELOPE ? 'persistent' : 'memory',
  )
  let generationController = new AbortController()
  let stopTransport: (() => void) | null = null

  const snapshot = (): SessionSnapshot => publicSnapshot(state)

  const emit = (
    previous: SessionSnapshot,
    reason: SessionTransitionReason,
    external: boolean,
  ): void => {
    const transition: SessionTransition = {
      previous,
      current: snapshot(),
      reason,
      external,
    }
    for (const listener of [...listeners]) listener(transition)
  }

  const advanceController = (): void => {
    generationController.abort('session-generation-changed')
    generationController = new AbortController()
  }

  const nextEnvelope = (
    accessToken: string,
    refreshToken: string,
    scope: string,
  ): PersistedSessionEnvelope => ({
    version: 1,
    accessToken,
    refreshToken,
    scope,
    epoch: state.epoch + 1,
    revision: state.revision + 1,
    updatedAt: Math.max(safeNow(now), state.updatedAt + 1),
    mutationId: createId(),
  })

  const commitLocal = (
    envelope: PersistedSessionEnvelope,
    reason: Exclude<SessionTransitionReason, 'external'>,
  ): SessionSnapshot => {
    const previous = snapshot()
    const persisted = safeWrite(persistence, envelope)
    state = stateFromEnvelope(envelope, persisted ? 'persistent' : 'memory')
    advanceController()
    emit(previous, reason, false)
    safePublish(options.transport, envelope)
    return snapshot()
  }

  const applyExternal = (candidate: PersistedSessionEnvelope | null): boolean => {
    if (!candidate || !isNewerEnvelope(candidate, state)) return false
    const previous = snapshot()
    state = stateFromEnvelope(candidate, 'persistent')
    advanceController()
    emit(previous, 'external', true)
    return true
  }

  const currentMatches = (
    lease: Pick<SessionLease, 'scope' | 'epoch' | 'accessToken'>,
  ): boolean => (
    state.scope === lease.scope
    && state.epoch === lease.epoch
    && state.accessToken === lease.accessToken
  )

  return {
    snapshot,
    capture(): SessionLease {
      return {
        ...snapshot(),
        signal: generationController.signal,
      }
    },
    isCurrent: currentMatches,
    replace(tokens): SessionSnapshot {
      applyExternal(safeRead(persistence))
      const accessToken = tokens.accessToken.trim()
      const refreshToken = tokens.refreshToken?.trim() || ''
      if (!accessToken) return commitLocal(nextEnvelope('', '', ''), 'logout')
      return commitLocal(nextEnvelope(accessToken, refreshToken, createId()), 'login')
    },
    commitRefresh(lease, tokens): SessionSnapshot | null {
      // Re-read the shared tombstone before CAS in case a storage event or
      // container message is delayed. A persisted logout therefore wins.
      applyExternal(safeRead(persistence))
      if (!currentMatches(lease) || !state.scope || !state.accessToken) return null
      const accessToken = tokens.accessToken.trim()
      const refreshToken = tokens.refreshToken.trim()
      if (!accessToken || !refreshToken) return null
      return commitLocal(nextEnvelope(accessToken, refreshToken, state.scope), 'refresh')
    },
    clear(reason = 'logout'): SessionSnapshot {
      applyExternal(safeRead(persistence))
      return commitLocal(nextEnvelope('', '', ''), reason)
    },
    clearIfCurrent(lease, reason = 'logout'): SessionSnapshot | null {
      applyExternal(safeRead(persistence))
      if (!currentMatches(lease)) return null
      return commitLocal(nextEnvelope('', '', ''), reason)
    },
    subscribe(listener): () => void {
      listeners.add(listener)
      return () => { listeners.delete(listener) }
    },
    synchronizeFromPersistence(): SessionSnapshot {
      applyExternal(safeRead(persistence))
      return snapshot()
    },
    startSynchronization(): void {
      if (stopTransport || !options.transport) return
      stopTransport = options.transport.subscribe((envelope) => {
        applyExternal(normalizeEnvelope(envelope))
      })
      applyExternal(safeRead(persistence))
    },
    stopSynchronization(): void {
      stopTransport?.()
      stopTransport = null
    },
  }
}

function stateFromEnvelope(
  envelope: PersistedSessionEnvelope,
  persistence: SessionPersistenceState,
): InternalState {
  return {
    accessToken: envelope.accessToken,
    refreshToken: envelope.refreshToken,
    scope: envelope.scope,
    epoch: envelope.epoch,
    revision: envelope.revision,
    updatedAt: envelope.updatedAt,
    mutationId: envelope.mutationId,
    persistence,
  }
}

function publicSnapshot(state: InternalState): SessionSnapshot {
  return {
    accessToken: state.accessToken,
    refreshToken: state.refreshToken,
    scope: state.scope,
    epoch: state.epoch,
    revision: state.revision,
    updatedAt: state.updatedAt,
    persistence: state.persistence,
  }
}

function safeRead(persistence: SessionPersistence | undefined): PersistedSessionEnvelope | null {
  if (!persistence) return null
  try {
    return normalizeEnvelope(persistence.read())
  } catch {
    return null
  }
}

function safeWrite(
  persistence: SessionPersistence | undefined,
  envelope: PersistedSessionEnvelope,
): boolean {
  if (!persistence) return false
  try {
    return persistence.write(envelope)
  } catch {
    return false
  }
}

function safePublish(
  transport: SessionSyncTransport | undefined,
  envelope: PersistedSessionEnvelope,
): void {
  if (!transport) return
  try {
    transport.publish(envelope)
  } catch {
    // Persistence and the current in-memory session remain authoritative.
  }
}

function normalizeEnvelope(value: unknown): PersistedSessionEnvelope | null {
  if (!value || typeof value !== 'object') return null
  const record = value as Partial<Record<keyof PersistedSessionEnvelope, unknown>>
  if (record.version !== 1) return null
  const accessToken = typeof record.accessToken === 'string' ? record.accessToken.trim() : ''
  const refreshToken = typeof record.refreshToken === 'string' ? record.refreshToken.trim() : ''
  const scope = typeof record.scope === 'string' ? record.scope.trim() : ''
  const epoch = safeInteger(record.epoch)
  const revision = safeInteger(record.revision)
  const updatedAt = safeInteger(record.updatedAt)
  const mutationId = typeof record.mutationId === 'string' ? record.mutationId.trim() : ''
  if (accessToken && !scope) return null
  return {
    version: 1,
    accessToken,
    refreshToken: accessToken ? refreshToken : '',
    scope: accessToken ? scope : '',
    epoch,
    revision,
    updatedAt,
    mutationId,
  }
}

function isNewerEnvelope(
  candidate: PersistedSessionEnvelope,
  current: Pick<InternalState, 'revision' | 'updatedAt' | 'mutationId'>,
): boolean {
  if (candidate.revision !== current.revision) return candidate.revision > current.revision
  if (candidate.updatedAt !== current.updatedAt) return candidate.updatedAt > current.updatedAt
  return candidate.mutationId > current.mutationId
}

function safeInteger(value: unknown): number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 ? value : 0
}

function safeNow(now: () => number): number {
  try {
    const value = now()
    return Number.isFinite(value) && value >= 0 ? Math.floor(value) : 0
  } catch {
    return 0
  }
}

function defaultCreateId(): string {
  try {
    if (typeof globalThis.crypto?.randomUUID === 'function') return globalThis.crypto.randomUUID()
  } catch {
    // Fall through to a process-local opaque identifier.
  }
  return `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`
}
