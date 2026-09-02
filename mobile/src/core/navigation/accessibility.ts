export const ROUTE_MAIN_CONTENT_ID = 'main-content'
export const ROUTE_ACCESSIBILITY_KEY_ATTRIBUTE = 'data-route-accessibility-key'
export const ROUTE_FOCUS_TARGET_ATTRIBUTE = 'data-route-focus-target'

const ROUTE_ADDED_TABINDEX_ATTRIBUTE = 'data-route-added-tabindex'
const ROUTE_ORIGINAL_ID_ATTRIBUTE = 'data-route-original-id'
const ACTIVE_DIALOG_SELECTOR = [
  'dialog[open]',
  '[role="dialog"][aria-modal="true"]:not([aria-hidden="true"])',
].join(', ')

export const ROUTE_ACCESSIBILITY_TITLE_KEYS = Object.freeze({
  home: 'routeAccessibility.titles.home',
  markets: 'routeAccessibility.titles.markets',
  'market-detail': 'routeAccessibility.titles.marketDetail',
  news: 'routeAccessibility.titles.news',
  'news-detail': 'routeAccessibility.titles.newsDetail',
  'message-center': 'routeAccessibility.titles.messageCenter',
  trade: 'routeAccessibility.titles.trade',
  swap: 'routeAccessibility.titles.swap',
  products: 'routeAccessibility.titles.products',
  earn: 'routeAccessibility.titles.earn',
  loan: 'routeAccessibility.titles.loan',
  'new-coins': 'routeAccessibility.titles.newCoins',
  'new-coin-records': 'routeAccessibility.titles.newCoinRecords',
  'new-coin-detail': 'routeAccessibility.titles.newCoinDetail',
  prediction: 'routeAccessibility.titles.prediction',
  seconds: 'routeAccessibility.titles.seconds',
  'seconds-history': 'routeAccessibility.titles.secondsHistory',
  orders: 'routeAccessibility.titles.orders',
  'position-associated-orders': 'routeAccessibility.titles.positionAssociatedOrders',
  profile: 'routeAccessibility.titles.profile',
  'help-support': 'routeAccessibility.titles.helpSupport',
  'support-chat': 'routeAccessibility.titles.supportChat',
  language: 'routeAccessibility.titles.language',
  kyc: 'routeAccessibility.titles.kyc',
  security: 'routeAccessibility.titles.security',
  'account-bindings': 'routeAccessibility.titles.accountBindings',
  referrals: 'routeAccessibility.titles.referrals',
  assets: 'routeAccessibility.titles.assets',
  'deposit-asset': 'routeAccessibility.titles.depositAsset',
  'deposit-network': 'routeAccessibility.titles.depositNetwork',
  'deposit-detail': 'routeAccessibility.titles.depositDetail',
  'withdraw-asset': 'routeAccessibility.titles.withdrawAsset',
  withdraw: 'routeAccessibility.titles.withdraw',
  'wallet-ledger': 'routeAccessibility.titles.walletLedger',
  'withdrawal-records': 'routeAccessibility.titles.withdrawalRecords',
  'quick-recharge': 'routeAccessibility.titles.quickRecharge',
  login: 'routeAccessibility.titles.login',
  'login-two-factor': 'routeAccessibility.titles.loginTwoFactor',
  register: 'routeAccessibility.titles.register',
  'forgot-password': 'routeAccessibility.titles.forgotPassword',
} as const)

export interface RouteAccessibilityLocation {
  readonly name?: unknown
  readonly path?: unknown
  readonly fullPath?: unknown
}

export type RouteAccessibilityTranslator = (
  key: string,
  values?: Readonly<Record<string, string>>,
) => string

export interface RouteAccessibilityElement {
  matches(selector: string): boolean
  querySelector(selector: string): RouteAccessibilityElement | null
  getAttribute(name: string): string | null
  setAttribute(name: string, value: string): void
  removeAttribute(name: string): void
  focus(options?: FocusOptions): void
}

export interface RouteAccessibilityDocument {
  title: string
  getElementById(id: string): RouteAccessibilityElement | null
  querySelector(selector: string): RouteAccessibilityElement | null
}

export interface RouteAccessibilityPolicy {
  readonly identity: string
  readonly renderKey: string
  readonly title: string
  readonly announcement: string
  readonly transferFocus: boolean
}

export type RouteTransitionCompletion =
  | 'stale'
  | 'missing-main'
  | 'prepared'
  | 'announced-dialog'
  | 'focused'

export interface RouteAccessibilityCoordinator {
  beginNavigation(
    to: RouteAccessibilityLocation,
    from: RouteAccessibilityLocation | null,
    translate: RouteAccessibilityTranslator,
  ): RouteAccessibilityPolicy
  updateDocumentTitle(
    route: RouteAccessibilityLocation,
    translate: RouteAccessibilityTranslator,
  ): string
  completeTransition(routeRoot: RouteAccessibilityElement | null): RouteTransitionCompletion
}

function normalizedRouteName(route: RouteAccessibilityLocation): string {
  return String(route.name || '').trim()
}

function normalizedRoutePath(route: RouteAccessibilityLocation): string {
  const path = String(route.path || '').trim()
  if (path) return path
  return String(route.fullPath || '/').split(/[?#]/, 1)[0] || '/'
}

export function routeAccessibilityIdentity(route: RouteAccessibilityLocation): string {
  return `${normalizedRouteName(route) || 'anonymous'}:${normalizedRoutePath(route)}`
}

export function routeAccessibilityRenderKey(route: RouteAccessibilityLocation): string {
  const fullPath = String(route.fullPath || normalizedRoutePath(route))
  return `${routeAccessibilityIdentity(route)}:${fullPath}`
}

export function shouldTransferRouteFocus(
  from: RouteAccessibilityLocation | null,
  to: RouteAccessibilityLocation,
): boolean {
  return from === null || routeAccessibilityIdentity(from) !== routeAccessibilityIdentity(to)
}

export function routeAccessibilityTitleKey(routeName: unknown): string {
  const name = String(routeName || '') as keyof typeof ROUTE_ACCESSIBILITY_TITLE_KEYS
  return ROUTE_ACCESSIBILITY_TITLE_KEYS[name] || 'routeAccessibility.titles.default'
}

export function localizedRouteDestination(
  route: RouteAccessibilityLocation,
  translate: RouteAccessibilityTranslator,
): string {
  return translate(routeAccessibilityTitleKey(route.name))
}

export function localizedRouteDocumentTitle(
  route: RouteAccessibilityLocation,
  translate: RouteAccessibilityTranslator,
): string {
  const app = translate('routeAccessibility.appName')
  if (routeAccessibilityTitleKey(route.name) === 'routeAccessibility.titles.default') return app
  return translate('routeAccessibility.documentTitle', {
    app,
    destination: localizedRouteDestination(route, translate),
  })
}

export function createRouteAccessibilityPolicy(
  to: RouteAccessibilityLocation,
  from: RouteAccessibilityLocation | null,
  translate: RouteAccessibilityTranslator,
): RouteAccessibilityPolicy {
  const destination = localizedRouteDestination(to, translate)
  const title = localizedRouteDocumentTitle(to, translate)
  return Object.freeze({
    identity: routeAccessibilityIdentity(to),
    renderKey: routeAccessibilityRenderKey(to),
    title,
    announcement: translate('routeAccessibility.destinationAnnouncement', { destination }),
    transferFocus: shouldTransferRouteFocus(from, to),
  })
}

function restorePreviousRouteMain(element: RouteAccessibilityElement): void {
  if (element.getAttribute(ROUTE_FOCUS_TARGET_ATTRIBUTE) !== 'true') return
  const originalId = element.getAttribute(ROUTE_ORIGINAL_ID_ATTRIBUTE)
  if (originalId === null) element.removeAttribute('id')
  else element.setAttribute('id', originalId)
  if (element.getAttribute(ROUTE_ADDED_TABINDEX_ATTRIBUTE) === 'true') {
    element.removeAttribute('tabindex')
  }
  element.removeAttribute(ROUTE_ADDED_TABINDEX_ATTRIBUTE)
  element.removeAttribute(ROUTE_ORIGINAL_ID_ATTRIBUTE)
  element.removeAttribute(ROUTE_FOCUS_TARGET_ATTRIBUTE)
}

function resolveRouteMain(routeRoot: RouteAccessibilityElement): RouteAccessibilityElement | null {
  return routeRoot.matches('main') ? routeRoot : routeRoot.querySelector('main')
}

function prepareRouteMain(
  documentTarget: RouteAccessibilityDocument,
  routeRoot: RouteAccessibilityElement,
): RouteAccessibilityElement | null {
  const main = resolveRouteMain(routeRoot)
  if (!main) return null

  const previous = documentTarget.getElementById(ROUTE_MAIN_CONTENT_ID)
  if (previous && previous !== main) restorePreviousRouteMain(previous)

  if (main.getAttribute(ROUTE_FOCUS_TARGET_ATTRIBUTE) !== 'true') {
    const originalId = main.getAttribute('id')
    if (originalId !== null && originalId !== ROUTE_MAIN_CONTENT_ID) {
      main.setAttribute(ROUTE_ORIGINAL_ID_ATTRIBUTE, originalId)
    }
    if (main.getAttribute('tabindex') === null) {
      main.setAttribute(ROUTE_ADDED_TABINDEX_ATTRIBUTE, 'true')
      main.setAttribute('tabindex', '-1')
    }
  }
  main.setAttribute('id', ROUTE_MAIN_CONTENT_ID)
  main.setAttribute(ROUTE_FOCUS_TARGET_ATTRIBUTE, 'true')
  return main
}

function hasActiveDialog(documentTarget: RouteAccessibilityDocument): boolean {
  const dialog = documentTarget.querySelector(ACTIVE_DIALOG_SELECTOR)
  return dialog !== null && dialog.getAttribute('hidden') === null
}

function focusRouteMain(main: RouteAccessibilityElement): void {
  try {
    main.focus({ preventScroll: true })
  } catch {
    main.focus()
  }
}

export function focusPreparedRouteMain(
  documentTarget: RouteAccessibilityDocument,
): boolean {
  const main = documentTarget.getElementById(ROUTE_MAIN_CONTENT_ID)
  if (!main) return false
  focusRouteMain(main)
  return true
}

export function createRouteAccessibilityCoordinator(input: {
  readonly document: RouteAccessibilityDocument
  readonly announce: (message: string) => void
}): RouteAccessibilityCoordinator {
  let pending: RouteAccessibilityPolicy | null = null

  return {
    beginNavigation(to, from, translate) {
      pending = createRouteAccessibilityPolicy(to, from, translate)
      input.document.title = pending.title
      return pending
    },
    updateDocumentTitle(route, translate) {
      const title = localizedRouteDocumentTitle(route, translate)
      input.document.title = title
      if (pending?.renderKey === routeAccessibilityRenderKey(route)) {
        const localized = createRouteAccessibilityPolicy(route, null, translate)
        pending = Object.freeze({
          ...localized,
          transferFocus: pending.transferFocus,
        })
      }
      return title
    },
    completeTransition(routeRoot) {
      if (!routeRoot || !pending) return 'stale'
      if (routeRoot.getAttribute(ROUTE_ACCESSIBILITY_KEY_ATTRIBUTE) !== pending.renderKey) {
        return 'stale'
      }

      const policy = pending
      pending = null
      const main = prepareRouteMain(input.document, routeRoot)
      if (!main) return 'missing-main'
      if (!policy.transferFocus) return 'prepared'

      input.announce(policy.announcement)
      if (hasActiveDialog(input.document)) return 'announced-dialog'
      focusPreparedRouteMain(input.document)
      return 'focused'
    },
  }
}
