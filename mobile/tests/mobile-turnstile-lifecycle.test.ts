import assert from 'node:assert/strict'
import test from 'node:test'
import {
  createTurnstileLifecycle,
  loadTurnstileApi,
  type TurnstileApi,
} from '../src/core/turnstile.ts'

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })
  return { promise, reject, resolve }
}

function containerFixture(): HTMLElement {
  return { isConnected: true } as HTMLElement
}

test('module loader reuses one script, waits for ready, and retries after a load failure', async () => {
  const originalWindow = Object.getOwnPropertyDescriptor(globalThis, 'window')
  const originalDocument = Object.getOwnPropertyDescriptor(globalThis, 'document')
  const scripts: FakeScript[] = []

  class FakeScript extends EventTarget {
    async = true
    dataset: Record<string, string> = {}
    defer = true
    src = ''

    remove(): void {
      const index = scripts.indexOf(this)
      if (index >= 0) scripts.splice(index, 1)
    }
  }

  const fakeWindow: { turnstile?: TurnstileApi } = {}
  const fakeDocument = {
    createElement: () => new FakeScript(),
    head: {
      appendChild: (script: FakeScript) => {
        scripts.push(script)
        return script
      },
    },
    querySelector: () => scripts[0] ?? null,
  }

  Object.defineProperty(globalThis, 'window', { configurable: true, value: fakeWindow })
  Object.defineProperty(globalThis, 'document', { configurable: true, value: fakeDocument })

  try {
    const firstLoad = loadTurnstileApi()
    const sharedLoad = loadTurnstileApi()
    assert.equal(firstLoad, sharedLoad)
    assert.equal(scripts.length, 1)
    assert.equal(scripts[0]?.async, false)
    assert.equal(scripts[0]?.defer, false)

    const failedLoads = Promise.allSettled([firstLoad, sharedLoad])
    scripts[0]?.dispatchEvent(new Event('error'))
    assert.deepEqual((await failedLoads).map((result) => result.status), ['rejected', 'rejected'])
    assert.equal(scripts.length, 0)

    const retryLoad = loadTurnstileApi()
    assert.notEqual(retryLoad, firstLoad)
    assert.equal(scripts.length, 1)
    assert.equal(scripts[0]?.async, false)
    assert.equal(scripts[0]?.defer, false)

    let readyCalls = 0
    fakeWindow.turnstile = {
      ready: (callback) => {
        readyCalls += 1
        callback()
      },
      render: () => 'widget',
      reset: () => undefined,
      remove: () => undefined,
    }
    scripts[0]?.dispatchEvent(new Event('load'))

    assert.equal(await retryLoad, fakeWindow.turnstile)
    assert.equal(readyCalls, 1)
    assert.equal(scripts.length, 1)
  } finally {
    if (originalWindow) Object.defineProperty(globalThis, 'window', originalWindow)
    else Reflect.deleteProperty(globalThis, 'window')
    if (originalDocument) Object.defineProperty(globalThis, 'document', originalDocument)
    else Reflect.deleteProperty(globalThis, 'document')
  }
})

test('module loader reuses a completed async/defer script without calling ready', async () => {
  const originalWindow = Object.getOwnPropertyDescriptor(globalThis, 'window')
  const originalDocument = Object.getOwnPropertyDescriptor(globalThis, 'document')
  const script = Object.assign(new EventTarget(), {
    async: true,
    dataset: { turnstileLoaderState: 'loaded' } as Record<string, string>,
    defer: true,
    remove: () => undefined,
    src: 'https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit',
  })
  let readyCalls = 0
  const api: TurnstileApi = {
    ready: () => {
      readyCalls += 1
      throw new Error('ready must not run for an already loaded async/defer script')
    },
    render: () => 'widget',
    reset: () => undefined,
    remove: () => undefined,
  }

  Object.defineProperty(globalThis, 'window', { configurable: true, value: { turnstile: api } })
  Object.defineProperty(globalThis, 'document', {
    configurable: true,
    value: {
      createElement: () => {
        throw new Error('the existing Turnstile script must be reused')
      },
      head: { appendChild: () => undefined },
      querySelector: () => script,
    },
  })

  try {
    const freshModuleUrl = new URL('../src/core/turnstile.ts?existing-async-script', import.meta.url)
    const { loadTurnstileApi: loadExistingTurnstileApi } = await import(freshModuleUrl.href)

    assert.equal(await loadExistingTurnstileApi(), api)
    assert.equal(readyCalls, 0)
    assert.equal(script.async, true)
    assert.equal(script.defer, true)
  } finally {
    if (originalWindow) Object.defineProperty(globalThis, 'window', originalWindow)
    else Reflect.deleteProperty(globalThis, 'window')
    if (originalDocument) Object.defineProperty(globalThis, 'document', originalDocument)
    else Reflect.deleteProperty(globalThis, 'document')
  }
})

test('slow initialization is cancelled before render when its generation becomes stale', async () => {
  const apiReady = deferred<TurnstileApi>()
  const loadStarted = deferred<void>()
  const renderCalls: Array<Record<string, unknown>> = []
  const api: TurnstileApi = {
    ready: (callback) => callback(),
    render: (_container, options) => {
      renderCalls.push(options)
      return 'slow-widget'
    },
    reset: () => undefined,
    remove: () => undefined,
  }
  const lifecycle = createTurnstileLifecycle({
    loadApi: () => {
      loadStarted.resolve()
      return apiReady.promise
    },
  })
  const container = containerFixture()
  const renderPromise = lifecycle.render({
    resolveContainer: () => container,
    isContainerCurrent: (candidate) => candidate === container,
    options: { sitekey: 'mobile-site-key' },
  })

  await loadStarted.promise
  lifecycle.remove()
  apiReady.resolve(api)

  assert.equal(await renderPromise, null)
  assert.equal(renderCalls.length, 0)
})

test('widget id zero resets in place while disconnected and removed widgets have stale callbacks', async () => {
  const removed: Array<string | number> = []
  const reset: Array<string | number> = []
  const tokens: string[] = []
  let widgetOptions: Record<string, unknown> = {}
  const api: TurnstileApi = {
    ready: (callback) => callback(),
    render: (_container, options) => {
      widgetOptions = options
      return 0
    },
    reset: (widgetId) => reset.push(widgetId),
    remove: (widgetId) => removed.push(widgetId),
  }
  const lifecycle = createTurnstileLifecycle({ loadApi: () => Promise.resolve(api) })
  const container = containerFixture()

  assert.equal(await lifecycle.render({
    resolveContainer: () => container,
    isContainerCurrent: (candidate) => candidate === container,
    options: { sitekey: 'mobile-site-key' },
    callbacks: { callback: (token) => tokens.push(token) },
  }), 0)
  assert.equal(lifecycle.reset(), true)
  assert.equal(lifecycle.getWidgetId(), 0)
  assert.deepEqual(reset, [0])

  const callback = widgetOptions.callback as (token: string) => void
  callback('current-token')
  ;(container as { isConnected: boolean }).isConnected = false
  callback('disconnected-token')
  lifecycle.remove()
  callback('removed-token')

  assert.deepEqual(tokens, ['current-token'])
  assert.deepEqual(removed, [0])
})

test('a render made stale synchronously is removed and a failed reset invalidates callbacks', async () => {
  const removed: Array<string | number> = []
  const tokens: string[] = []
  let lifecycle = createTurnstileLifecycle()
  let staleOptions: Record<string, unknown> = {}
  const staleApi: TurnstileApi = {
    ready: (callback) => callback(),
    render: (_container, options) => {
      staleOptions = options
      lifecycle.remove()
      return 'stale-widget'
    },
    reset: () => undefined,
    remove: (widgetId) => removed.push(widgetId),
  }
  const container = containerFixture()
  lifecycle = createTurnstileLifecycle({ loadApi: () => Promise.resolve(staleApi) })

  assert.equal(await lifecycle.render({
    resolveContainer: () => container,
    isContainerCurrent: (candidate) => candidate === container,
    options: {},
    callbacks: { callback: (token) => tokens.push(token) },
  }), null)
  ;(staleOptions.callback as (token: string) => void)('stale-token')
  assert.deepEqual(removed, ['stale-widget'])
  assert.deepEqual(tokens, [])

  let activeOptions: Record<string, unknown> = {}
  const resetFailureApi: TurnstileApi = {
    ready: (callback) => callback(),
    render: (_container, options) => {
      activeOptions = options
      return 'reset-widget'
    },
    reset: () => {
      throw new Error('reset failed')
    },
    remove: (widgetId) => removed.push(widgetId),
  }
  lifecycle = createTurnstileLifecycle({ loadApi: () => Promise.resolve(resetFailureApi) })
  await lifecycle.render({
    resolveContainer: () => container,
    isContainerCurrent: (candidate) => candidate === container,
    options: {},
    callbacks: { callback: (token) => tokens.push(token) },
  })

  assert.equal(lifecycle.reset(), false)
  ;(activeOptions.callback as (token: string) => void)('after-reset-failure')
  assert.deepEqual(removed, ['stale-widget', 'reset-widget'])
  assert.deepEqual(tokens, [])
})
