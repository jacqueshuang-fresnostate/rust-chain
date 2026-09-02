import assert from 'node:assert/strict'
import test from 'node:test'
import en from '../src/i18n/messages/en.ts'
import zhCN from '../src/i18n/messages/zh-CN.ts'
import {
  ROUTE_ACCESSIBILITY_KEY_ATTRIBUTE,
  ROUTE_ACCESSIBILITY_TITLE_KEYS,
  ROUTE_FOCUS_TARGET_ATTRIBUTE,
  ROUTE_MAIN_CONTENT_ID,
  createRouteAccessibilityCoordinator,
  focusPreparedRouteMain,
  localizedRouteDocumentTitle,
  routeAccessibilityRenderKey,
  shouldTransferRouteFocus,
  type RouteAccessibilityDocument,
  type RouteAccessibilityElement,
  type RouteAccessibilityLocation,
  type RouteAccessibilityTranslator,
} from '../src/core/navigation/accessibility.ts'

const EXPECTED_ROUTE_NAMES = [
  'home',
  'markets',
  'market-detail',
  'news',
  'news-detail',
  'message-center',
  'trade',
  'swap',
  'products',
  'earn',
  'loan',
  'new-coins',
  'new-coin-records',
  'new-coin-detail',
  'prediction',
  'seconds',
  'seconds-history',
  'orders',
  'position-associated-orders',
  'profile',
  'help-support',
  'support-chat',
  'language',
  'kyc',
  'security',
  'account-bindings',
  'referrals',
  'assets',
  'deposit-asset',
  'deposit-network',
  'deposit-detail',
  'withdraw-asset',
  'withdraw',
  'wallet-ledger',
  'withdrawal-records',
  'quick-recharge',
  'login',
  'login-two-factor',
  'register',
  'forgot-password',
] as const

class FakeElement implements RouteAccessibilityElement {
  readonly attributes = new Map<string, string>()
  readonly children: FakeElement[] = []
  readonly tagName: string
  focusCalls: Array<FocusOptions | undefined> = []

  constructor(tagName: string, children: FakeElement[] = []) {
    this.tagName = tagName
    this.children.push(...children)
  }

  matches(selector: string): boolean {
    return selector === this.tagName
  }

  querySelector(selector: string): FakeElement | null {
    for (const child of this.children) {
      if (child.matches(selector)) return child
      const nested = child.querySelector(selector)
      if (nested) return nested
    }
    return null
  }

  getAttribute(name: string): string | null {
    return this.attributes.get(name) ?? null
  }

  setAttribute(name: string, value: string): void {
    this.attributes.set(name, value)
  }

  removeAttribute(name: string): void {
    this.attributes.delete(name)
  }

  focus(options?: FocusOptions): void {
    this.focusCalls.push(options)
  }
}

class FakeDocument implements RouteAccessibilityDocument {
  title = ''
  roots: FakeElement[] = []
  activeDialog: FakeElement | null = null

  getElementById(id: string): FakeElement | null {
    return this.roots.map((root) => findById(root, id)).find(Boolean) || null
  }

  querySelector(selector: string): FakeElement | null {
    return selector.includes('[role="dialog"]') || selector.includes('dialog[open]')
      ? this.activeDialog
      : null
  }
}

function findById(element: FakeElement, id: string): FakeElement | null {
  if (element.getAttribute('id') === id) return element
  return element.children.map((child) => findById(child, id)).find(Boolean) || null
}

function route(
  name: string,
  path: string,
  fullPath = path,
): RouteAccessibilityLocation {
  return { name, path, fullPath }
}

function routeRoot(location: RouteAccessibilityLocation, main = new FakeElement('main')): {
  main: FakeElement
  root: FakeElement
} {
  const root = new FakeElement('section', [main])
  root.setAttribute(ROUTE_ACCESSIBILITY_KEY_ATTRIBUTE, routeAccessibilityRenderKey(location))
  return { main, root }
}

function translator(messages: unknown): RouteAccessibilityTranslator {
  return (key, values) => {
    const value = key.split('.').reduce<unknown>((current, part) => (
      current && typeof current === 'object'
        ? (current as Record<string, unknown>)[part]
        : undefined
    ), messages)
    assert.equal(typeof value, 'string', `missing locale key ${key}`)
    return Object.entries(values || {}).reduce(
      (text, [name, replacement]) => text.replaceAll(`{${name}}`, replacement),
      value as string,
    )
  }
}

test('every named mobile destination resolves a symmetric localized document title', () => {
  assert.deepEqual(Object.keys(ROUTE_ACCESSIBILITY_TITLE_KEYS), EXPECTED_ROUTE_NAMES)
  const zh = translator(zhCN)
  const english = translator(en)

  assert.equal(localizedRouteDocumentTitle(route('home', '/'), zh), '首页 · HIPPO')
  assert.equal(localizedRouteDocumentTitle(route('seconds-history', '/seconds/history'), english), 'Seconds order history · HIPPO')
  assert.equal(localizedRouteDocumentTitle(route('future-route', '/future'), zh), 'HIPPO')

  for (const name of EXPECTED_ROUTE_NAMES) {
    const location = route(name, `/${name}`)
    assert.doesNotMatch(localizedRouteDocumentTitle(location, zh), /routeAccessibility\./)
    assert.doesNotMatch(localizedRouteDocumentTitle(location, english), /routeAccessibility\./)
  }
})

test('route completion owns one main target, announces, and focuses only after the entered DOM is ready', () => {
  const documentTarget = new FakeDocument()
  const announcements: string[] = []
  const coordinator = createRouteAccessibilityCoordinator({
    document: documentTarget,
    announce: (message) => announcements.push(message),
  })
  const home = route('home', '/')
  const rendered = routeRoot(home)
  documentTarget.roots = [rendered.root]

  coordinator.beginNavigation(home, null, translator(en))
  assert.equal(documentTarget.title, 'Home · HIPPO')
  assert.equal(rendered.main.focusCalls.length, 0, 'navigation start must not focus stale DOM')
  assert.equal(coordinator.completeTransition(rendered.root), 'focused')
  assert.deepEqual(rendered.main.focusCalls, [{ preventScroll: true }])
  assert.equal(rendered.main.getAttribute('id'), ROUTE_MAIN_CONTENT_ID)
  assert.equal(rendered.main.getAttribute('tabindex'), '-1')
  assert.equal(rendered.main.getAttribute(ROUTE_FOCUS_TARGET_ATTRIBUTE), 'true')
  assert.deepEqual(announcements, ['Now viewing Home'])
})

test('same-route query changes move the skip target without stealing focus or repeating the live announcement', () => {
  const documentTarget = new FakeDocument()
  const announcements: string[] = []
  const coordinator = createRouteAccessibilityCoordinator({
    document: documentTarget,
    announce: (message) => announcements.push(message),
  })
  const initial = route('orders', '/orders', '/orders?tab=spot')
  const queried = route('orders', '/orders', '/orders?tab=history')
  const first = routeRoot(initial)
  documentTarget.roots = [first.root]

  coordinator.beginNavigation(initial, null, translator(zhCN))
  assert.equal(coordinator.completeTransition(first.root), 'focused')

  const second = routeRoot(queried)
  documentTarget.roots.push(second.root)
  assert.equal(shouldTransferRouteFocus(initial, queried), false)
  coordinator.beginNavigation(queried, initial, translator(zhCN))
  assert.equal(coordinator.completeTransition(second.root), 'prepared')
  assert.equal(first.main.getAttribute('id'), null)
  assert.equal(first.main.getAttribute('tabindex'), null)
  assert.equal(second.main.getAttribute('id'), ROUTE_MAIN_CONTENT_ID)
  assert.equal(second.main.focusCalls.length, 0)
  assert.deepEqual(announcements, ['已进入订单中心'])
  assert.equal(focusPreparedRouteMain(documentTarget), true)
  assert.deepEqual(second.main.focusCalls, [{ preventScroll: true }])
})

test('an active dialog keeps focus while a real destination still updates title and the one live channel', () => {
  const documentTarget = new FakeDocument()
  const announcements: string[] = []
  const coordinator = createRouteAccessibilityCoordinator({
    document: documentTarget,
    announce: (message) => announcements.push(message),
  })
  const from = route('home', '/')
  const to = route('markets', '/markets')
  const rendered = routeRoot(to)
  documentTarget.roots = [rendered.root]
  documentTarget.activeDialog = new FakeElement('div')

  coordinator.beginNavigation(to, from, translator(en))
  assert.equal(coordinator.completeTransition(rendered.root), 'announced-dialog')
  assert.equal(documentTarget.title, 'Markets · HIPPO')
  assert.equal(rendered.main.focusCalls.length, 0)
  assert.equal(rendered.main.getAttribute('id'), ROUTE_MAIN_CONTENT_ID)
  assert.deepEqual(announcements, ['Now viewing Markets'])
})

test('a superseded transition cannot focus or claim the main-content anchor', () => {
  const documentTarget = new FakeDocument()
  const coordinator = createRouteAccessibilityCoordinator({
    document: documentTarget,
    announce: () => undefined,
  })
  const oldRoute = route('markets', '/markets')
  const currentRoute = route('assets', '/assets')
  const oldRendered = routeRoot(oldRoute)
  const currentRendered = routeRoot(currentRoute)
  documentTarget.roots = [oldRendered.root, currentRendered.root]

  coordinator.beginNavigation(currentRoute, oldRoute, translator(zhCN))
  assert.equal(coordinator.completeTransition(oldRendered.root), 'stale')
  assert.equal(oldRendered.main.getAttribute('id'), null)
  assert.equal(oldRendered.main.focusCalls.length, 0)
  assert.equal(coordinator.completeTransition(currentRendered.root), 'focused')
  assert.equal(currentRendered.main.getAttribute('id'), ROUTE_MAIN_CONTENT_ID)
})

test('a locale switch during an entering transition refreshes title and pending announcement without changing focus policy', () => {
  const documentTarget = new FakeDocument()
  const announcements: string[] = []
  const coordinator = createRouteAccessibilityCoordinator({
    document: documentTarget,
    announce: (message) => announcements.push(message),
  })
  const from = route('home', '/')
  const to = route('assets', '/assets')
  const rendered = routeRoot(to)
  documentTarget.roots = [rendered.root]

  coordinator.beginNavigation(to, from, translator(en))
  assert.equal(documentTarget.title, 'Assets · HIPPO')
  coordinator.updateDocumentTitle(to, translator(zhCN))
  assert.equal(documentTarget.title, '资产 · HIPPO')
  assert.equal(coordinator.completeTransition(rendered.root), 'focused')
  assert.deepEqual(announcements, ['已进入资产'])
  assert.equal(rendered.main.focusCalls.length, 1)
})
