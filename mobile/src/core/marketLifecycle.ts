export type MarketConnectionState = 'idle' | 'connecting' | 'live' | 'stale' | 'offline'

export interface MarketLifecycleScheduler {
  setTimeout(callback: () => void, delay: number): unknown
  clearTimeout(handle: unknown): void
}

export interface MarketLifecycleSnapshot {
  readonly connection: MarketConnectionState
  readonly consumerCount: number
  readonly lastFrameAt: number
  readonly refreshing: boolean
  readonly refreshFailed: boolean
  readonly updatedAt: number
}

export interface SharedMarketLifecycleOptions {
  /** Loads and commits one REST snapshot. Errors are reflected in state. */
  load(): Promise<void>
  /** True after a usable REST snapshot has been committed. */
  hasData(): boolean
  /** Stable key for the complete live symbol lease. */
  liveKey(): string
  /** Opens one shared live transport and reports each accepted data frame. */
  connect(onFrame: () => void): () => void
  isOnline?: () => boolean
  now?: () => number
  scheduler?: MarketLifecycleScheduler
  refreshTtlMs?: number
  staleAfterMs?: number
}

export interface SharedMarketLifecycle {
  /** The exact in-flight Promise is returned to every concurrent caller. */
  refresh(force?: boolean): Promise<void>
  acquire(consumerId: string): void
  release(consumerId: string): void
  ensureLive(): void
  setOnline(online: boolean): void
  snapshot(): MarketLifecycleSnapshot
  subscribe(listener: (snapshot: MarketLifecycleSnapshot) => void): () => void
  dispose(): void
}

const DEFAULT_REFRESH_TTL_MS = 20_000
const DEFAULT_STALE_AFTER_MS = 65_000

const defaultScheduler: MarketLifecycleScheduler = {
  setTimeout: (callback, delay) => globalThis.setTimeout(callback, delay),
  clearTimeout: (handle) => globalThis.clearTimeout(
    handle as ReturnType<typeof globalThis.setTimeout>,
  ),
}

/**
 * Coordinates REST cold starts and the single ticker lease independently of
 * view lifetimes. A view that joins while another view is loading receives
 * the same Promise and its lease is fulfilled after the snapshot commits.
 */
export function createSharedMarketLifecycle(
  options: SharedMarketLifecycleOptions,
): SharedMarketLifecycle {
  const now = options.now ?? Date.now
  const scheduler = options.scheduler ?? defaultScheduler
  const refreshTtlMs = positiveDelay(options.refreshTtlMs, DEFAULT_REFRESH_TTL_MS)
  const staleAfterMs = positiveDelay(options.staleAfterMs, DEFAULT_STALE_AFTER_MS)
  const consumers = new Set<string>()
  const listeners = new Set<(snapshot: MarketLifecycleSnapshot) => void>()

  let online = safeOnlineState(options.isOnline)
  let connection: MarketConnectionState = online ? 'idle' : 'offline'
  let lastFrameAt = 0
  let updatedAt = 0
  let refreshing = false
  let refreshFailed = false
  let refreshPromise: Promise<void> | null = null
  let disconnect: (() => void) | null = null
  let connectedKey = ''
  let connectionLease: object | null = null
  let staleTimer: unknown = null

  const snapshot = (): MarketLifecycleSnapshot => ({
    connection,
    consumerCount: consumers.size,
    lastFrameAt,
    refreshing,
    refreshFailed,
    updatedAt,
  })

  const emit = (): void => {
    const current = snapshot()
    for (const listener of [...listeners]) listener(current)
  }

  const clearStaleTimer = (): void => {
    if (staleTimer === null) return
    scheduler.clearTimeout(staleTimer)
    staleTimer = null
  }

  const armStaleTimer = (lease: object): void => {
    clearStaleTimer()
    staleTimer = scheduler.setTimeout(() => {
      staleTimer = null
      if (connectionLease !== lease || !disconnect || !online || !consumers.size) return
      connection = 'stale'
      emit()
    }, staleAfterMs)
  }

  const closeLive = (): void => {
    clearStaleTimer()
    connectionLease = null
    connectedKey = ''
    const stop = disconnect
    disconnect = null
    if (!stop) return
    try {
      stop()
    } catch {
      // The lifecycle state remains authoritative if a transport disposer fails.
    }
  }

  const ensureLive = (): void => {
    if (!consumers.size) {
      const changed = connection !== (online ? 'idle' : 'offline') || Boolean(disconnect)
      closeLive()
      connection = online ? 'idle' : 'offline'
      if (changed) emit()
      return
    }
    if (!online) {
      const changed = connection !== 'offline' || Boolean(disconnect)
      closeLive()
      connection = 'offline'
      if (changed) emit()
      return
    }

    const nextKey = options.liveKey().trim()
    if (!options.hasData() || !nextKey) {
      const changed = Boolean(disconnect) || connection !== 'idle'
      closeLive()
      connection = 'idle'
      if (changed) emit()
      return
    }
    if (disconnect && connectedKey === nextKey) return

    closeLive()
    connection = 'connecting'
    connectedKey = nextKey
    const lease = {}
    connectionLease = lease
    emit()

    try {
      const stop = options.connect(() => {
        if (connectionLease !== lease || !online || !consumers.size) return
        const frameAt = now()
        lastFrameAt = frameAt
        updatedAt = frameAt
        connection = 'live'
        armStaleTimer(lease)
        emit()
      })
      if (connectionLease !== lease) {
        try { stop() } catch { /* no-op */ }
        return
      }
      disconnect = stop
      armStaleTimer(lease)
    } catch {
      if (connectionLease !== lease) return
      closeLive()
      connection = 'stale'
      emit()
    }
  }

  const refresh = (force = false): Promise<void> => {
    if (refreshPromise) return refreshPromise
    if (
      !force
      && options.hasData()
      && updatedAt > 0
      && now() - updatedAt < refreshTtlMs
    ) {
      ensureLive()
      return Promise.resolve()
    }

    refreshing = true
    emit()
    let loadPromise: Promise<void>
    try {
      loadPromise = options.load()
    } catch (error) {
      loadPromise = Promise.reject(error)
    }
    const current = Promise.resolve(loadPromise)
      .then(() => {
        updatedAt = now()
        refreshFailed = false
      })
      .catch(() => {
        refreshFailed = true
      })
      .finally(() => {
        refreshing = false
        if (refreshPromise === current) refreshPromise = null
        ensureLive()
        emit()
      })
    refreshPromise = current
    return current
  }

  return {
    refresh,
    acquire(consumerId: string): void {
      const consumer = consumerId.trim()
      if (!consumer || consumers.has(consumer)) return
      consumers.add(consumer)
      emit()
      ensureLive()
    },
    release(consumerId: string): void {
      const consumer = consumerId.trim()
      if (!consumer || !consumers.delete(consumer)) return
      ensureLive()
      emit()
    },
    ensureLive,
    setOnline(nextOnline: boolean): void {
      if (online === nextOnline) return
      online = nextOnline
      ensureLive()
    },
    snapshot,
    subscribe(listener): () => void {
      listeners.add(listener)
      listener(snapshot())
      return () => { listeners.delete(listener) }
    },
    dispose(): void {
      consumers.clear()
      closeLive()
      connection = online ? 'idle' : 'offline'
      listeners.clear()
    },
  }
}

function safeOnlineState(readOnline: (() => boolean) | undefined): boolean {
  if (!readOnline) return true
  try {
    return readOnline()
  } catch {
    return true
  }
}

function positiveDelay(value: number | undefined, fallback: number): number {
  return typeof value === 'number' && Number.isFinite(value) && value > 0
    ? Math.floor(value)
    : fallback
}
