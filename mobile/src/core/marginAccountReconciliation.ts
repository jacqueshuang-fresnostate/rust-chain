export const MARGIN_ACCOUNT_RECONCILIATION_INTERVAL_MS = 5_000

export type MarginAccountReconciliationKind = 'foreground' | 'background'

export type MarginAccountReconciliationSkipReason =
  | 'inactive'
  | 'guest'
  | 'spot'
  | 'hidden'
  | 'foreground'
  | 'single-flight'

export type MarginAccountReconciliationResult =
  | { state: 'completed' }
  | { state: 'error'; error: unknown }
  | { state: 'stale' }
  | { state: 'skipped'; reason: MarginAccountReconciliationSkipReason }

export interface MarginAccountReconciliationRequest {
  readonly kind: MarginAccountReconciliationKind
  isCurrent(): boolean
  commit(update: () => void): boolean
}

export interface MarginAccountReconciliationScheduler {
  setInterval(callback: () => void, delay: number): unknown
  clearInterval(handle: unknown): void
}

export interface MarginAccountReconciliationLifecycle {
  refreshForeground(): Promise<MarginAccountReconciliationResult>
  refreshBackground(options?: { queueIfBusy?: boolean }): Promise<MarginAccountReconciliationResult>
  invalidate(): void
  startPolling(): void
  stop(): void
  isPolling(): boolean
  isBackgroundInFlight(): boolean
}

const defaultScheduler: MarginAccountReconciliationScheduler = {
  setInterval: (callback, delay) => globalThis.setInterval(callback, delay),
  clearInterval: (handle) => globalThis.clearInterval(
    handle as ReturnType<typeof globalThis.setInterval>,
  ),
}

/**
 * Owns the latest-request and polling boundary for the private margin account.
 * A foreground refresh may supersede an older poll, while background runs never
 * overlap one another or a current foreground refresh.
 */
export function createMarginAccountReconciliationLifecycle(options: {
  sessionKey: () => string
  isContractMode: () => boolean
  isVisible: () => boolean
  reconcile: (request: MarginAccountReconciliationRequest) => Promise<void>
  intervalMs?: number
  scheduler?: MarginAccountReconciliationScheduler
}): MarginAccountReconciliationLifecycle {
  const scheduler = options.scheduler ?? defaultScheduler
  const intervalMs = options.intervalMs ?? MARGIN_ACCOUNT_RECONCILIATION_INTERVAL_MS
  let active = true
  let generation = 0
  let pollingHandle: unknown = null
  let backgroundInFlight = false
  let backgroundGeneration: number | null = null
  let queuedBackgroundRefresh = false
  let foregroundGeneration: number | null = null
  let queuedReplayLease: object | null = null

  function skipReason(
    kind: MarginAccountReconciliationKind,
  ): MarginAccountReconciliationSkipReason | null {
    if (!active) return 'inactive'
    if (!options.sessionKey()) return 'guest'
    if (!options.isContractMode()) return 'spot'
    if (kind === 'background' && !options.isVisible()) return 'hidden'
    if (kind === 'background' && foregroundGeneration !== null) return 'foreground'
    if (kind === 'background' && backgroundInFlight) return 'single-flight'
    return null
  }

  async function refresh(
    kind: MarginAccountReconciliationKind,
  ): Promise<MarginAccountReconciliationResult> {
    const reason = skipReason(kind)
    if (reason) return { state: 'skipped', reason }

    const requestGeneration = ++generation
    const requestSessionKey = options.sessionKey()
    if (kind === 'background') {
      backgroundInFlight = true
      backgroundGeneration = requestGeneration
    } else {
      foregroundGeneration = requestGeneration
    }

    const isCurrent = (): boolean => (
      active
      && requestGeneration === generation
      && options.sessionKey() === requestSessionKey
      && options.isContractMode()
      && (kind === 'foreground' || options.isVisible())
    )
    const request: MarginAccountReconciliationRequest = Object.freeze({
      kind,
      isCurrent,
      commit(update: () => void): boolean {
        if (!isCurrent()) return false
        update()
        return true
      },
    })

    let result: MarginAccountReconciliationResult
    try {
      await options.reconcile(request)
      result = isCurrent() ? { state: 'completed' } : { state: 'stale' }
    } catch (error) {
      result = isCurrent() ? { state: 'error', error } : { state: 'stale' }
    } finally {
      if (kind === 'background') {
        backgroundInFlight = false
        backgroundGeneration = null
      } else if (foregroundGeneration === requestGeneration) {
        foregroundGeneration = null
      }
      scheduleQueuedBackgroundRefresh()
    }
    return result
  }

  function backgroundContextIsEligible(): boolean {
    return active
      && Boolean(options.sessionKey())
      && options.isContractMode()
      && options.isVisible()
  }

  function cancelQueuedBackgroundReplay(): void {
    queuedReplayLease = null
  }

  function scheduleQueuedBackgroundRefresh(): void {
    if (
      !queuedBackgroundRefresh
      || backgroundInFlight
      || foregroundGeneration !== null
      || queuedReplayLease !== null
    ) return

    const replayLease = {}
    queuedReplayLease = replayLease

    // Let the initiating foreground caller settle its loading/error state before
    // a silent queued refresh can complete and race that state back out of order.
    void Promise.resolve().then(() => {
      void Promise.resolve().then(() => {
        if (queuedReplayLease !== replayLease) return
        queuedReplayLease = null
        if (
          !queuedBackgroundRefresh
          || backgroundInFlight
          || foregroundGeneration !== null
        ) return
        queuedBackgroundRefresh = false
        void refresh('background')
      })
    })
  }

  function refreshBackground(
    refreshOptions: { queueIfBusy?: boolean } = {},
  ): Promise<MarginAccountReconciliationResult> {
    const busy = backgroundInFlight || foregroundGeneration !== null
    if (
      refreshOptions.queueIfBusy === true
      && backgroundInFlight
      && foregroundGeneration === null
      && backgroundGeneration === generation
      && backgroundContextIsEligible()
    ) {
      // A socket/open/visibility hint happened after this poll began. Keep the
      // transport single-flight, but prevent its older snapshot from committing.
      generation += 1
    }
    const supersededBackground = backgroundInFlight && backgroundGeneration !== generation
    if (
      busy
      && backgroundContextIsEligible()
      && (refreshOptions.queueIfBusy === true || supersededBackground)
    ) {
      queuedBackgroundRefresh = true
    }
    return refresh('background')
  }

  return {
    refreshForeground: () => refresh('foreground'),
    refreshBackground,
    invalidate(): void {
      generation += 1
      foregroundGeneration = null
      queuedBackgroundRefresh = false
      cancelQueuedBackgroundReplay()
    },
    startPolling(): void {
      if (!active || pollingHandle !== null) return
      pollingHandle = scheduler.setInterval(() => {
        void refreshBackground()
      }, intervalMs)
    },
    stop(): void {
      if (!active) return
      active = false
      generation += 1
      foregroundGeneration = null
      queuedBackgroundRefresh = false
      cancelQueuedBackgroundReplay()
      if (pollingHandle !== null) scheduler.clearInterval(pollingHandle)
      pollingHandle = null
    },
    isPolling: () => pollingHandle !== null,
    isBackgroundInFlight: () => backgroundInFlight,
  }
}

/**
 * Retains cached risks only for the latest eligible position IDs and replaces
 * each fulfilled request by its expected ID. Failed requests keep the last
 * successful eligible snapshot.
 */
export function reconcileMarginRiskSnapshots<T>(
  current: Readonly<Record<string, T>>,
  eligiblePositionIds: readonly string[],
  settled: readonly PromiseSettledResult<T>[] = [],
): Record<string, T> {
  const next: Record<string, T> = {}
  const uniqueIds = [...new Set(eligiblePositionIds)]

  uniqueIds.forEach((positionId) => {
    if (Object.prototype.hasOwnProperty.call(current, positionId)) {
      next[positionId] = current[positionId] as T
    }
  })
  settled.forEach((result, index) => {
    const positionId = uniqueIds[index]
    if (positionId !== undefined && result.status === 'fulfilled') {
      next[positionId] = result.value
    }
  })
  return next
}
