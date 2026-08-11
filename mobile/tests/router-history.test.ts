import assert from 'node:assert/strict'
import test from 'node:test'
import {
  createMemoryHistory,
  createRouter,
  createWebHistory,
  type RouteLocationRaw,
  type Router,
} from 'vue-router'
import {
  createBottomNavSecondsFallbackTarget,
  createBottomNavSecondsTarget,
  createLoginRedirectTarget,
  goBackOr,
  isBottomNavigationSecondsEntry,
  replaceAuthStep,
  sanitizeInternalRedirect,
} from '../src/core/navigation.ts'

const routes = [
  { path: '/', name: 'home', component: {} },
  { path: '/markets', name: 'markets', component: {} },
  { path: '/news', name: 'news', component: {} },
  { path: '/products', name: 'products', component: {} },
  { path: '/seconds', name: 'seconds', component: {} },
  { path: '/assets', name: 'assets', component: {} },
  { path: '/login', name: 'login', component: {} },
  { path: '/register', name: 'register', component: {} },
  { path: '/forgot-password', name: 'forgot-password', component: {} },
  { path: '/login/two-factor', name: 'login-two-factor', component: {} },
]

function createHistoryRouter(): Router {
  return createRouter({
    history: createMemoryHistory(),
    routes,
  })
}

function createBrowserHistoryRouter(): { restore: () => void, router: Router } {
  const globalNames = ['window', 'document', 'history', 'location'] as const
  const originalDescriptors = new Map(
    globalNames.map((name) => [name, Object.getOwnPropertyDescriptor(globalThis, name)]),
  )
  const listeners = new Map<string, Set<(event: { state: unknown }) => void>>()
  const locationState = {
    protocol: 'https:',
    host: 'mobile.test',
    pathname: '/',
    search: '',
    hash: '',
  }
  const entries: Array<{ state: unknown, url: string }> = [{ state: null, url: '/' }]
  let position = 0

  function applyUrl(value: string | URL | null | undefined): void {
    if (value == null) return
    const currentUrl = `https://${locationState.host}${locationState.pathname}${locationState.search}${locationState.hash}`
    const url = new URL(String(value), currentUrl)
    locationState.protocol = url.protocol
    locationState.host = url.host
    locationState.pathname = url.pathname
    locationState.search = url.search
    locationState.hash = url.hash
  }

  function currentUrl(): string {
    return `${locationState.pathname}${locationState.search}${locationState.hash}`
  }

  const historyState = {
    get length(): number {
      return entries.length
    },
    get state(): unknown {
      return entries[position]?.state ?? null
    },
    replaceState(state: unknown, _title: string, url?: string | URL | null): void {
      applyUrl(url)
      entries[position] = { state, url: currentUrl() }
    },
    pushState(state: unknown, _title: string, url?: string | URL | null): void {
      applyUrl(url)
      position += 1
      entries.splice(position)
      entries.push({ state, url: currentUrl() })
    },
    go(delta: number): void {
      const nextPosition = Math.max(0, Math.min(position + delta, entries.length - 1))
      if (nextPosition === position) return
      position = nextPosition
      applyUrl(entries[position]?.url)
      queueMicrotask(() => {
        for (const listener of listeners.get('popstate') || []) {
          listener({ state: entries[position]?.state })
        }
      })
    },
  }
  const windowState = {
    history: historyState,
    location: locationState,
    scrollX: 0,
    scrollY: 0,
    pageXOffset: 0,
    pageYOffset: 0,
    scrollTo(): void {},
    addEventListener(type: string, listener: (event: { state: unknown }) => void): void {
      const typeListeners = listeners.get(type) || new Set()
      typeListeners.add(listener)
      listeners.set(type, typeListeners)
    },
    removeEventListener(type: string, listener: (event: { state: unknown }) => void): void {
      listeners.get(type)?.delete(listener)
    },
  }
  const documentState = {
    visibilityState: 'visible',
    querySelector(): null {
      return null
    },
    addEventListener(): void {},
    removeEventListener(): void {},
  }

  Object.defineProperties(globalThis, {
    window: { configurable: true, value: windowState },
    document: { configurable: true, value: documentState },
    history: { configurable: true, value: historyState },
    location: { configurable: true, value: locationState },
  })

  const router = createRouter({
    history: createWebHistory('/'),
    routes,
  })

  return {
    router,
    restore() {
      router.options.history.destroy()
      for (const name of globalNames) {
        const descriptor = originalDescriptors.get(name)
        if (descriptor) Object.defineProperty(globalThis, name, descriptor)
        else Reflect.deleteProperty(globalThis, name)
      }
    },
  }
}

function setHistoryBack(router: Router, back: string): void {
  const history = router.options.history
  history.replace(router.currentRoute.value.fullPath, {
    ...history.state,
    back,
  })
}

async function goBackOrAndWait(
  router: Router,
  fallback: RouteLocationRaw,
  preferFallback: boolean,
): Promise<void> {
  const navigation = new Promise<void>((resolve) => {
    const removeGuard = router.afterEach(() => {
      removeGuard()
      resolve()
    })
  })
  await goBackOr(router, fallback, { preferFallback })
  await navigation
}

async function backAndWait(router: Router): Promise<void> {
  const navigation = new Promise<void>((resolve) => {
    const removeGuard = router.afterEach(() => {
      removeGuard()
      resolve()
    })
  })
  router.back()
  await navigation
}

test('底部入口 replace 到 Seconds 后强制回首页，产品中心 push 仍返回 Products', async () => {
  const bottomFixture = createBrowserHistoryRouter()
  try {
    const bottomRouter = bottomFixture.router
    await bottomRouter.push('/')
    await bottomRouter.push('/markets')
    await bottomRouter.push('/assets')
    await bottomRouter.replace(createBottomNavSecondsTarget())

    assert.equal(bottomRouter.currentRoute.value.fullPath, '/seconds')
    assert.equal(bottomRouter.options.history.state.back, '/markets')
    assert.equal(isBottomNavigationSecondsEntry(bottomRouter.options.history.state), true)

    await goBackOrAndWait(
      bottomRouter,
      createBottomNavSecondsFallbackTarget(),
      isBottomNavigationSecondsEntry(bottomRouter.options.history.state),
    )
    assert.equal(bottomRouter.currentRoute.value.fullPath, '/')
    assert.equal(isBottomNavigationSecondsEntry(bottomRouter.options.history.state), false)

    await bottomRouter.replace('/assets')
    assert.equal(isBottomNavigationSecondsEntry(bottomRouter.options.history.state), false)
  } finally {
    bottomFixture.restore()
  }

  const productFixture = createBrowserHistoryRouter()
  try {
    const productRouter = productFixture.router
    await productRouter.push('/')
    await productRouter.push('/products')
    await productRouter.push({ name: 'seconds' })

    assert.equal(productRouter.options.history.state.back, '/products')
    assert.equal(isBottomNavigationSecondsEntry(productRouter.options.history.state), false)

    await goBackOrAndWait(
      productRouter,
      createBottomNavSecondsFallbackTarget(),
      isBottomNavigationSecondsEntry(productRouter.options.history.state),
    )
    assert.equal(productRouter.currentRoute.value.fullPath, '/products')

    await productRouter.push('/assets')
    await goBackOrAndWait(productRouter, { name: 'home' }, false)
    assert.equal(productRouter.currentRoute.value.fullPath, '/products')
  } finally {
    productFixture.restore()
  }

  const memoryRouter = createHistoryRouter()
  await memoryRouter.push('/')
  await memoryRouter.push('/products')
  await memoryRouter.push({ name: 'seconds' })
  setHistoryBack(memoryRouter, '/products')
  await goBackOrAndWait(
    memoryRouter,
    createBottomNavSecondsFallbackTarget(),
    isBottomNavigationSecondsEntry(memoryRouter.options.history.state),
  )
  assert.equal(memoryRouter.currentRoute.value.fullPath, '/products')
})

test('新闻页优先返回产品中心历史，直开时 replace 到产品中心', async () => {
  const historyFixture = createBrowserHistoryRouter()
  try {
    const router = historyFixture.router
    await router.push('/')
    await router.push('/products')
    await router.push('/news')

    assert.equal(router.options.history.state.back, '/products')
    await goBackOrAndWait(router, '/products', false)
    assert.equal(router.currentRoute.value.fullPath, '/products')
  } finally {
    historyFixture.restore()
  }

  const directRouter = createHistoryRouter()
  await directRouter.replace('/news')
  assert.equal(directRouter.options.history.state.back, undefined)

  const replacements: RouteLocationRaw[] = []
  const originalReplace = directRouter.replace.bind(directRouter)
  directRouter.replace = ((target: RouteLocationRaw) => {
    replacements.push(target)
    return originalReplace(target)
  }) as Router['replace']

  await goBackOrAndWait(directRouter, '/products', false)
  assert.deepEqual(replacements, ['/products'])
  assert.equal(directRouter.currentRoute.value.fullPath, '/products')
})

test('登录 replace 到注册、忘记密码和 2FA 后，完成流程不会把登录页留在历史栈', async () => {
  const authFlows: Array<{
    step: RouteLocationRaw
    completion: RouteLocationRaw[]
  }> = [
    {
      step: { name: 'register', query: { redirect: '/assets' } },
      completion: ['/assets'],
    },
    {
      step: { name: 'forgot-password', query: { redirect: '/assets' } },
      completion: [createLoginRedirectTarget('/assets'), '/assets'],
    },
    {
      step: { name: 'login-two-factor', query: { challenge: 'challenge-1', redirect: '/assets' } },
      completion: ['/assets'],
    },
    {
      step: { name: 'login-two-factor', query: { setup: 'setup-1', redirect: '/assets' } },
      completion: ['/assets'],
    },
  ]

  for (const authFlow of authFlows) {
    const router = createHistoryRouter()
    await router.push('/')
    await router.push(createLoginRedirectTarget('/assets'))
    await replaceAuthStep(router, authFlow.step)
    for (const target of authFlow.completion) {
      await replaceAuthStep(router, target)
    }

    assert.equal(router.currentRoute.value.fullPath, '/assets')
    await backAndWait(router)
    assert.equal(router.currentRoute.value.fullPath, '/')
  }
})

test('注册、忘记密码和 2FA 显式返回始终 replace 到保留 redirect 的登录页', async () => {
  for (const step of [
    { name: 'register', query: {} },
    { name: 'forgot-password', query: {} },
    { name: 'login-two-factor', query: { challenge: 'challenge-1' } },
  ] as const) {
    const router = createHistoryRouter()
    await router.push('/')
    await router.push('/markets')
    await router.push(createLoginRedirectTarget('/assets?tab=funding'))
    await replaceAuthStep(router, {
      name: step.name,
      query: { ...step.query, redirect: '/assets?tab=funding' },
    })
    setHistoryBack(router, '/markets')

    await goBackOrAndWait(
      router,
      createLoginRedirectTarget(router.currentRoute.value.query.redirect),
      true,
    )

    assert.equal(router.currentRoute.value.name, 'login')
    assert.equal(router.currentRoute.value.query.redirect, '/assets?tab=funding')
  }
})

test('2FA 重置和挑战失效回登录时保留清洗后的 redirect', async () => {
  for (const redirect of ['/assets?tab=funding', '//outside.example/path']) {
    const returnRouter = createHistoryRouter()
    await returnRouter.push('/')
    await returnRouter.push({
      name: 'login-two-factor',
      query: { challenge: 'challenge-1', redirect },
    })
    await replaceAuthStep(
      returnRouter,
      createLoginRedirectTarget(returnRouter.currentRoute.value.query.redirect),
    )

    assert.equal(returnRouter.currentRoute.value.name, 'login')
    assert.equal(
      returnRouter.currentRoute.value.query.redirect,
      redirect.startsWith('//') ? '/' : redirect,
    )

    const completionRouter = createHistoryRouter()
    await completionRouter.push('/')
    await completionRouter.push({
      name: 'login-two-factor',
      query: { challenge: 'challenge-1', redirect },
    })
    await replaceAuthStep(
      completionRouter,
      sanitizeInternalRedirect(completionRouter.currentRoute.value.query.redirect),
    )
    assert.equal(
      completionRouter.currentRoute.value.fullPath,
      redirect.startsWith('//') ? '/' : redirect,
    )
  }
})
