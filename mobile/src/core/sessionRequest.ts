export type SessionRequestLoadResult<T> =
  | { state: 'guest' }
  | { state: 'loaded'; value: T }
  | { state: 'error'; error: unknown }
  | { state: 'stale' }

export interface SessionRequestLifecycle<T> {
  load: () => Promise<SessionRequestLoadResult<T>>
  invalidate: () => void
  stop: () => void
}

export function createSessionRequestLifecycle<T>(input: {
  sessionKey: () => string
  request: () => Promise<T>
}): SessionRequestLifecycle<T> {
  let requestVersion = 0
  let active = true

  return {
    async load(): Promise<SessionRequestLoadResult<T>> {
      const version = ++requestVersion
      if (!active) return { state: 'stale' }
      const sessionKey = input.sessionKey()
      if (!sessionKey) return { state: 'guest' }

      try {
        const value = await input.request()
        if (!active || version !== requestVersion || input.sessionKey() !== sessionKey) {
          return { state: 'stale' }
        }
        return { state: 'loaded', value }
      } catch (error) {
        if (!active || version !== requestVersion || input.sessionKey() !== sessionKey) {
          return { state: 'stale' }
        }
        return { state: 'error', error }
      }
    },
    invalidate(): void {
      requestVersion += 1
    },
    stop(): void {
      active = false
      requestVersion += 1
    },
  }
}
