import type {
  PrivateUserEvent,
  PrivateUserTransportState,
} from '../api/privateUserStream.ts'

export type PrivateUserTopic = 'margin' | 'support'
export type PrivateUserConnectionState = 'idle' | 'connecting' | 'live' | 'stale' | 'offline'

export interface PrivateUserManagerSession {
  readonly accessToken: string
  readonly scope: string
  readonly generation: number
}

export interface PrivateUserManagedTransport {
  start(): boolean
  stop(): void
}

export interface PrivateUserManagedTransportContext {
  readonly accessToken: string
  onOpen(): void
  onInboundFrame(receivedAt: number): void
  onStateChange(state: PrivateUserTransportState): void
  onEvent(event: PrivateUserEvent): void
}

export interface PrivateUserStreamManagerOptions {
  readSession(): PrivateUserManagerSession
  openTransport(context: PrivateUserManagedTransportContext): PrivateUserManagedTransport
  isOnline?: () => boolean
}

export interface PrivateUserTopicLeaseOptions {
  readonly topic: PrivateUserTopic
  readonly consumerId: string
  readonly onOpen?: () => void
  readonly onEvent: (event: PrivateUserEvent) => void
}

export interface PrivateUserTopicLease {
  release(): void
}

export interface PrivateUserStreamManagerSnapshot {
  readonly connection: PrivateUserConnectionState
  readonly connecting: boolean
  readonly live: boolean
  readonly stale: boolean
  readonly offline: boolean
  readonly lastFrameAt: number
  readonly consumerCount: number
  readonly marginConsumerCount: number
  readonly supportConsumerCount: number
  readonly sessionGeneration: number
}

export interface PrivateUserStreamManager {
  acquire(options: PrivateUserTopicLeaseOptions): PrivateUserTopicLease
  synchronizeSession(session?: PrivateUserManagerSession): void
  setOnline(online: boolean): void
  snapshot(): PrivateUserStreamManagerSnapshot
  subscribe(listener: (snapshot: PrivateUserStreamManagerSnapshot) => void): () => void
  dispose(): void
}

interface LeaseRecord extends PrivateUserTopicLeaseOptions {
  readonly identity: object
}

const MARGIN_EVENT_TYPES = new Set([
  'margin.position.liquidated',
  'margin.position.partially_closed',
  'margin.position.closed',
])

/**
 * Owns the one private connection for the current authenticated generation.
 * Views lease business topics; releasing one view never tears down another
 * consumer, while any token generation change invalidates the whole transport.
 */
export function createPrivateUserStreamManager(
  options: PrivateUserStreamManagerOptions,
): PrivateUserStreamManager {
  const leases = new Map<object, LeaseRecord>()
  const listeners = new Set<(snapshot: PrivateUserStreamManagerSnapshot) => void>()

  let online = safeOnline(options.isOnline)
  let session = normalizeSession(safeReadSession(options.readSession))
  let connection: PrivateUserConnectionState = online ? 'idle' : 'offline'
  let lastFrameAt = 0
  let transport: PrivateUserManagedTransport | null = null
  let transportLease: object | null = null

  const snapshot = (): PrivateUserStreamManagerSnapshot => {
    let marginConsumerCount = 0
    let supportConsumerCount = 0
    for (const lease of leases.values()) {
      if (lease.topic === 'margin') marginConsumerCount += 1
      else supportConsumerCount += 1
    }
    return Object.freeze({
      connection,
      connecting: connection === 'connecting',
      live: connection === 'live',
      stale: connection === 'stale',
      offline: connection === 'offline',
      lastFrameAt,
      consumerCount: leases.size,
      marginConsumerCount,
      supportConsumerCount,
      sessionGeneration: session.generation,
    })
  }

  const emit = (): void => {
    const current = snapshot()
    for (const listener of [...listeners]) {
      try {
        listener(current)
      } catch {
        // Diagnostics subscribers do not own the shared connection.
      }
    }
  }

  const setConnection = (next: PrivateUserConnectionState): void => {
    if (connection === next) return
    connection = next
    emit()
  }

  const stopTransport = (): void => {
    transportLease = null
    const current = transport
    transport = null
    if (!current) return
    try {
      current.stop()
    } catch {
      // Manager state and generation identity remain authoritative.
    }
  }

  const dispatchOpen = (): void => {
    for (const lease of [...leases.values()]) {
      try {
        lease.onOpen?.()
      } catch {
        // One consumer cannot interrupt another consumer or the transport.
      }
    }
  }

  const dispatchEvent = (event: PrivateUserEvent): void => {
    for (const lease of [...leases.values()]) {
      if (!eventMatchesTopic(event, lease.topic)) continue
      try {
        lease.onEvent(event)
      } catch {
        // A page callback cannot interrupt shared event delivery.
      }
    }
  }

  const ensureConnection = (): void => {
    if (!online) {
      stopTransport()
      setConnection('offline')
      return
    }
    if (!leases.size || !session.accessToken || !session.scope) {
      stopTransport()
      setConnection('idle')
      return
    }
    if (transport) return

    const generationSession = session
    const generationLease = {}
    transportLease = generationLease
    setConnection('connecting')

    let next: PrivateUserManagedTransport
    try {
      next = options.openTransport({
        accessToken: generationSession.accessToken,
        onOpen: () => {
          if (!isCurrentTransportGeneration(generationLease, generationSession)) return
          setConnection('live')
          dispatchOpen()
        },
        onInboundFrame: (receivedAt) => {
          if (!isCurrentTransportGeneration(generationLease, generationSession)) return
          if (Number.isFinite(receivedAt) && receivedAt >= 0) {
            lastFrameAt = Math.floor(receivedAt)
          }
          connection = 'live'
          emit()
        },
        onStateChange: (state) => {
          if (!isCurrentTransportGeneration(generationLease, generationSession)) return
          if (state === 'live') setConnection('live')
          else if (state === 'stale') setConnection('stale')
          else if (state === 'connecting') setConnection('connecting')
          else if (leases.size && online && session.accessToken) setConnection('stale')
          else setConnection(online ? 'idle' : 'offline')
        },
        onEvent: (event) => {
          if (!isCurrentTransportGeneration(generationLease, generationSession)) return
          dispatchEvent(event)
        },
      })
    } catch {
      transportLease = null
      setConnection('stale')
      return
    }

    if (transportLease !== generationLease) {
      try { next.stop() } catch { /* no-op */ }
      return
    }
    transport = next
    let started = false
    try {
      started = next.start()
    } catch {
      started = false
    }
    if (started) return

    transportLease = null
    transport = null
    try { next.stop() } catch { /* no-op */ }
    setConnection('stale')
  }

  const isCurrentTransportGeneration = (
    candidateLease: object,
    candidateSession: PrivateUserManagerSession,
  ): boolean => (
    transportLease === candidateLease
    && sameSession(session, candidateSession)
    && online
    && leases.size > 0
  )

  const synchronizeSession = (candidate?: PrivateUserManagerSession): void => {
    const next = normalizeSession(candidate ?? safeReadSession(options.readSession))
    if (sameSession(session, next)) {
      ensureConnection()
      return
    }

    // Invalidate identity before stop(), so synchronous late close/state
    // handlers from the old socket cannot alter the replacement generation.
    stopTransport()
    session = next
    lastFrameAt = 0
    connection = online ? 'idle' : 'offline'
    emit()
    ensureConnection()
  }

  return {
    acquire(leaseOptions): PrivateUserTopicLease {
      const consumerId = leaseOptions.consumerId.trim()
      if (!consumerId) throw new TypeError('private user stream consumerId is required')
      const identity = {}
      const record: LeaseRecord = Object.freeze({
        ...leaseOptions,
        consumerId,
        identity,
      })
      leases.set(identity, record)
      emit()
      synchronizeSession()

      let released = false
      return Object.freeze({
        release(): void {
          if (released) return
          released = true
          if (!leases.delete(identity)) return
          ensureConnection()
          emit()
        },
      })
    },
    synchronizeSession,
    setOnline(nextOnline): void {
      if (online === nextOnline) return
      online = nextOnline
      if (!online) {
        stopTransport()
        setConnection('offline')
        return
      }
      connection = 'idle'
      emit()
      synchronizeSession()
    },
    snapshot,
    subscribe(listener): () => void {
      listeners.add(listener)
      listener(snapshot())
      return () => { listeners.delete(listener) }
    },
    dispose(): void {
      leases.clear()
      stopTransport()
      connection = online ? 'idle' : 'offline'
      listeners.clear()
    },
  }
}

export function eventMatchesTopic(
  event: Pick<PrivateUserEvent, 'type'>,
  topic: PrivateUserTopic,
): boolean {
  if (topic === 'support') return event.type === 'support.refresh'
  return MARGIN_EVENT_TYPES.has(event.type)
}

function safeReadSession(readSession: () => PrivateUserManagerSession): PrivateUserManagerSession {
  try {
    return readSession()
  } catch {
    return { accessToken: '', scope: '', generation: 0 }
  }
}

function normalizeSession(value: PrivateUserManagerSession): PrivateUserManagerSession {
  const accessToken = typeof value.accessToken === 'string' ? value.accessToken.trim() : ''
  const scope = typeof value.scope === 'string' ? value.scope.trim() : ''
  const generation = Number.isSafeInteger(value.generation) && value.generation >= 0
    ? value.generation
    : 0
  if (!accessToken || !scope) return Object.freeze({ accessToken: '', scope: '', generation })
  return Object.freeze({ accessToken, scope, generation })
}

function sameSession(
  left: PrivateUserManagerSession,
  right: PrivateUserManagerSession,
): boolean {
  return left.generation === right.generation
    && left.scope === right.scope
    && left.accessToken === right.accessToken
}

function safeOnline(readOnline: (() => boolean) | undefined): boolean {
  if (!readOnline) return true
  try {
    return readOnline()
  } catch {
    return true
  }
}
