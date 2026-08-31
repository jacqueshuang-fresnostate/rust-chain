export interface OrdersRequestSnapshot {
  sessionGeneration: number
  market: 'spot' | 'margin'
  state: 'current' | 'history' | 'positions'
}

export type LatestOrdersRequestResult<T> =
  | { state: 'loaded'; snapshot: OrdersRequestSnapshot; value: T }
  | { state: 'error'; snapshot: OrdersRequestSnapshot; error: unknown }
  | { state: 'stale' }

export interface OrdersRequestLifecycle {
  load: <T>(
    snapshot: OrdersRequestSnapshot,
    request: (signal: AbortSignal) => Promise<T>,
  ) => Promise<LatestOrdersRequestResult<T>>
  invalidate: () => void
  stop: () => void
}

/** Owns one latest-only request generation and actively aborts superseded tab requests. */
export function createOrdersRequestLifecycle(): OrdersRequestLifecycle {
  let active = true
  let generation = 0
  let controller: AbortController | null = null

  return {
    async load<T>(
      snapshot: OrdersRequestSnapshot,
      request: (signal: AbortSignal) => Promise<T>,
    ): Promise<LatestOrdersRequestResult<T>> {
      const requestGeneration = ++generation
      controller?.abort()
      controller = new AbortController()
      const requestController = controller
      if (!active) {
        requestController.abort()
        return { state: 'stale' }
      }

      try {
        const value = await request(requestController.signal)
        if (!active || requestGeneration !== generation || requestController.signal.aborted) {
          return { state: 'stale' }
        }
        return { state: 'loaded', snapshot: { ...snapshot }, value }
      } catch (error) {
        if (!active || requestGeneration !== generation || requestController.signal.aborted) {
          return { state: 'stale' }
        }
        return { state: 'error', snapshot: { ...snapshot }, error }
      } finally {
        if (controller === requestController) controller = null
      }
    },
    invalidate(): void {
      generation += 1
      controller?.abort()
      controller = null
    },
    stop(): void {
      active = false
      generation += 1
      controller?.abort()
      controller = null
    },
  }
}

export interface SpotCancelAllOutcome {
  kind: 'success' | 'partial' | 'failure'
  succeeded: number
  failed: number
  failureDetails: string[]
}

export function spotCancelAllOutcome(result: {
  orders: readonly { id: string }[]
  failures: readonly { id: string; code: string; message: string }[]
}): SpotCancelAllOutcome {
  const succeeded = result.orders.length
  const failed = result.failures.length
  return {
    kind: failed === 0 ? 'success' : succeeded === 0 ? 'failure' : 'partial',
    succeeded,
    failed,
    failureDetails: result.failures.map((failure) => {
      const identity = failure.id.trim() || failure.code.trim()
      const message = failure.message.trim() || failure.code.trim()
      return [identity, message].filter(Boolean).join(': ')
    }).filter(Boolean),
  }
}

export function commitSpotCancelAllResult<T extends { id: string }>(
  currentOrders: readonly T[],
  result: {
    orders: readonly { id: string }[]
    failures: readonly { id: string; code: string; message: string }[]
  },
): { remainingOrders: T[]; outcome: SpotCancelAllOutcome } {
  const canceledIds = new Set(result.orders.map((order) => order.id))
  return {
    remainingOrders: currentOrders.filter((order) => !canceledIds.has(order.id)),
    outcome: spotCancelAllOutcome(result),
  }
}
