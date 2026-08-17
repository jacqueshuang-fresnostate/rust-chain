const TURNSTILE_SCRIPT_URL = 'https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit'
const TURNSTILE_SCRIPT_SELECTOR = `script[src="${TURNSTILE_SCRIPT_URL}"]`

export type TurnstileWidgetId = string | number

export type TurnstileApi = {
  ready: (callback: () => void) => void
  render: (element: string | HTMLElement, options: Record<string, unknown>) => TurnstileWidgetId
  reset: (widgetId: TurnstileWidgetId) => void
  remove: (widgetId: TurnstileWidgetId) => void
}

declare global {
  interface Window {
    turnstile?: TurnstileApi
  }
}

let turnstileLoadPromise: Promise<TurnstileApi> | null = null

type LoadedTurnstileApi = {
  api: TurnstileApi
  waitForReady: boolean
}

export function getTurnstileApi(): TurnstileApi | undefined {
  return typeof window === 'undefined' ? undefined : window.turnstile
}

function canWaitForTurnstileReady(script: HTMLScriptElement | null): boolean {
  return !script || (!script.async && !script.defer)
}

function loadTurnstileScript(): Promise<LoadedTurnstileApi> {
  const availableApi = getTurnstileApi()
  if (typeof document === 'undefined') {
    if (availableApi) {
      return Promise.resolve({ api: availableApi, waitForReady: true })
    }
    return Promise.reject(new Error('Cloudflare Turnstile requires a browser document'))
  }

  const existingScript = document.querySelector<HTMLScriptElement>(TURNSTILE_SCRIPT_SELECTOR)
  if (availableApi) {
    return Promise.resolve({
      api: availableApi,
      waitForReady: canWaitForTurnstileReady(existingScript),
    })
  }

  return new Promise((resolve, reject) => {
    let script = existingScript
    const shouldAppendScript = !script

    if (!script) {
      script = document.createElement('script')
      // Dynamic classic scripts are force-async by default. The advanced SPA
      // contract requires synchronous execution before turnstile.ready().
      script.async = false
      script.defer = false
      script.src = TURNSTILE_SCRIPT_URL
    }
    const waitForReady = canWaitForTurnstileReady(script)

    const cleanup = () => {
      script.removeEventListener('load', handleLoad)
      script.removeEventListener('error', handleError)
    }
    const fail = (error: Error) => {
      cleanup()
      script.dataset.turnstileLoaderState = 'failed'
      if (!getTurnstileApi()) {
        script.remove()
      }
      reject(error)
    }
    const handleLoad = () => {
      script.dataset.turnstileLoaderState = 'loaded'
      const api = getTurnstileApi()
      if (!api) {
        fail(new Error('Cloudflare Turnstile API is unavailable after script load'))
        return
      }
      cleanup()
      resolve({ api, waitForReady })
    }
    const handleError = () => {
      fail(new Error('Failed to load Cloudflare Turnstile script'))
    }

    script.addEventListener('load', handleLoad)
    script.addEventListener('error', handleError)

    if (script.dataset.turnstileLoaderState === 'loaded') {
      queueMicrotask(handleLoad)
    } else if (shouldAppendScript) {
      document.head.appendChild(script)
    }
  })
}

function waitForTurnstileReady(api: TurnstileApi): Promise<TurnstileApi> {
  return new Promise((resolve, reject) => {
    try {
      api.ready(() => resolve(api))
    } catch (error) {
      reject(error)
    }
  })
}

export function loadTurnstileApi(): Promise<TurnstileApi> {
  if (!turnstileLoadPromise) {
    turnstileLoadPromise = (async () => {
      const loaded = await loadTurnstileScript()
      return loaded.waitForReady ? waitForTurnstileReady(loaded.api) : loaded.api
    })().catch((error: unknown) => {
      turnstileLoadPromise = null
      throw error
    })
  }

  return turnstileLoadPromise
}

type MaybePromise<T> = T | Promise<T>

export type TurnstileLifecycleCallbacks = {
  beforeInteractive?: () => void
  callback?: (token: string) => void
  expired?: () => void
  error?: () => void
  timeout?: () => void
}

export type TurnstileRenderRequest = {
  resolveContainer: () => MaybePromise<HTMLElement | null>
  isContainerCurrent: (container: HTMLElement) => boolean
  options: Record<string, unknown>
  callbacks?: TurnstileLifecycleCallbacks
  onError?: (error: unknown) => void
}

export type TurnstileLifecycle = {
  render: (request: TurnstileRenderRequest) => Promise<TurnstileWidgetId | null>
  reset: () => boolean
  remove: () => void
  getWidgetId: () => TurnstileWidgetId | null
}

type TurnstileLifecycleOptions = {
  loadApi?: () => Promise<TurnstileApi>
  onWidgetIdChange?: (widgetId: TurnstileWidgetId | null) => void
}

export function createTurnstileLifecycle(options: TurnstileLifecycleOptions = {}): TurnstileLifecycle {
  const loadApi = options.loadApi ?? loadTurnstileApi
  let generation = 0
  let widgetId: TurnstileWidgetId | null = null
  let widgetApi: TurnstileApi | null = null

  const notifyWidgetId = (nextWidgetId: TurnstileWidgetId | null) => {
    options.onWidgetIdChange?.(nextWidgetId)
  }

  const removeRenderedWidget = () => {
    const currentWidgetId = widgetId
    const currentApi = widgetApi ?? getTurnstileApi()
    widgetId = null
    widgetApi = null
    notifyWidgetId(null)

    if (currentWidgetId !== null && currentApi) {
      try {
        currentApi.remove(currentWidgetId)
      } catch {
        // Cleanup is best effort; the generation guard already blocks stale callbacks.
      }
    }
  }

  const isCurrent = (
    renderGeneration: number,
    container: HTMLElement,
    isContainerCurrent: TurnstileRenderRequest['isContainerCurrent'],
  ) => generation === renderGeneration && container.isConnected && isContainerCurrent(container)

  const remove = () => {
    generation += 1
    removeRenderedWidget()
  }

  const render = async (request: TurnstileRenderRequest): Promise<TurnstileWidgetId | null> => {
    generation += 1
    const renderGeneration = generation
    removeRenderedWidget()

    let container: HTMLElement | null = null
    try {
      container = await request.resolveContainer()
      if (!container || !isCurrent(renderGeneration, container, request.isContainerCurrent)) {
        return null
      }
      const currentContainer = container

      const api = await loadApi()
      if (!isCurrent(renderGeneration, currentContainer, request.isContainerCurrent)) {
        return null
      }

      const runIfCurrent = (callback: (() => void) | undefined) => () => {
        if (isCurrent(renderGeneration, currentContainer, request.isContainerCurrent)) {
          callback?.()
        }
      }
      const receiveTokenIfCurrent = (callback: ((token: string) => void) | undefined) => (token: string) => {
        if (isCurrent(renderGeneration, currentContainer, request.isContainerCurrent)) {
          callback?.(token)
        }
      }

      const nextWidgetId = api.render(currentContainer, {
        ...request.options,
        'before-interactive-callback': runIfCurrent(request.callbacks?.beforeInteractive),
        callback: receiveTokenIfCurrent(request.callbacks?.callback),
        'expired-callback': runIfCurrent(request.callbacks?.expired),
        'error-callback': runIfCurrent(request.callbacks?.error),
        'timeout-callback': runIfCurrent(request.callbacks?.timeout),
      })

      if (!isCurrent(renderGeneration, currentContainer, request.isContainerCurrent)) {
        try {
          api.remove(nextWidgetId)
        } catch {
          // The stale widget is already detached from application state.
        }
        return null
      }

      widgetId = nextWidgetId
      widgetApi = api
      notifyWidgetId(nextWidgetId)
      return nextWidgetId
    } catch (error) {
      const shouldReport = generation === renderGeneration
        && (!container || isCurrent(renderGeneration, container, request.isContainerCurrent))
      if (shouldReport) {
        request.onError?.(error)
      }
      return null
    }
  }

  const reset = () => {
    const currentWidgetId = widgetId
    const currentApi = widgetApi ?? getTurnstileApi()
    if (currentWidgetId === null || !currentApi) {
      return false
    }

    try {
      currentApi.reset(currentWidgetId)
      return true
    } catch {
      remove()
      return false
    }
  }

  return {
    render,
    reset,
    remove,
    getWidgetId: () => widgetId,
  }
}
