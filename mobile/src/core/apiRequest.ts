import axios, { type AxiosInstance, type CreateAxiosDefaults } from 'axios'

export const DEFAULT_API_REQUEST_TIMEOUT_MS = 12_000

export interface ComposedAbortSignal {
  readonly signal: AbortSignal | undefined
  dispose(): void
}

/** Creates the shared HTTP transport with a bounded default timeout. */
export function createApiHttpClient(config: CreateAxiosDefaults = {}): AxiosInstance {
  return axios.create({
    ...config,
    timeout: config.timeout ?? DEFAULT_API_REQUEST_TIMEOUT_MS,
    headers: {
      'Content-Type': 'application/json',
      ...config.headers,
    },
  })
}

/**
 * Composes caller cancellation with the current session-generation signal.
 * The disposer removes listeners after Axios settles or before a replay.
 */
export function composeAbortSignals(
  signals: ReadonlyArray<AbortSignal | null | undefined>,
): ComposedAbortSignal {
  const active = [...new Set(signals.filter((signal): signal is AbortSignal => Boolean(signal)))]
  if (!active.length) return { signal: undefined, dispose: () => undefined }
  if (active.length === 1) return { signal: active[0], dispose: () => undefined }

  const controller = new AbortController()
  let disposed = false
  const removers: Array<() => void> = []
  const abortFrom = (source: AbortSignal): void => {
    if (!controller.signal.aborted) controller.abort(source.reason)
  }

  for (const source of active) {
    if (source.aborted) {
      abortFrom(source)
      break
    }
    const listener = (): void => abortFrom(source)
    source.addEventListener('abort', listener, { once: true })
    removers.push(() => source.removeEventListener('abort', listener))
  }

  return {
    signal: controller.signal,
    dispose(): void {
      if (disposed) return
      disposed = true
      for (const remove of removers) remove()
      removers.length = 0
    },
  }
}
