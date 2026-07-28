import { ref } from 'vue'
import type { RouteLocationRaw, Router } from 'vue-router'

export const DEFAULT_TRADE_SYMBOL = 'BTC_USDT'
export const ROOT_ROUTE_ORDER = [
  'home',
  'markets',
  'spot',
  'contract',
  'assets',
  'profile',
] as const

export type RootRouteKey = typeof ROOT_ROUTE_ORDER[number]
export type RouteDirection = 'forward' | 'back' | 'still'
export type RouteTransitionTier = 'root' | 'secondary'

export const routeTransitionName = ref('route-fade')
export const routeDirection = ref<RouteDirection>('still')
export const routeTransitionTier = ref<RouteTransitionTier>('secondary')
export const routeTransitionSequence = ref(0)

export function normalizeRouteSymbol(value: unknown): string {
  const parts = String(value || '')
    .trim()
    .toUpperCase()
    .split(/[\/_-]/)
    .filter(Boolean)
  if (parts.length < 2) return DEFAULT_TRADE_SYMBOL
  return `${parts[0]}_${parts[1]}`
}

export function sanitizeInternalRedirect(value: unknown, fallback = '/'): string {
  if (typeof value !== 'string') return fallback
  const target = value.trim()
  if (!target.startsWith('/') || target.startsWith('//')) return fallback
  return target
}

export function hasUsableRouterBack(state: unknown): boolean {
  const back = (state as { back?: unknown } | null)?.back
  return typeof back === 'string' && back.startsWith('/') && !back.startsWith('//')
}

export async function goBackOr(router: Router, fallback: RouteLocationRaw = '/'): Promise<void> {
  if (hasUsableRouterBack(router.options.history.state)) {
    router.back()
    return
  }
  await router.replace(fallback)
}

export function resolveRootRouteKey(
  routeName: unknown,
  mode: unknown,
  purpose: unknown = undefined,
): RootRouteKey | null {
  const name = String(routeName || '')
  if (name === 'markets' && purpose === 'trade') return null
  if (name === 'trade') return mode === 'contract' ? 'contract' : 'spot'
  if (ROOT_ROUTE_ORDER.includes(name as RootRouteKey)) return name as RootRouteKey
  return null
}

export function classifyRootRouteDirection(
  from: RootRouteKey | null,
  to: RootRouteKey | null,
): RouteDirection {
  if (!from || !to) return 'still'
  const currentIndex = ROOT_ROUTE_ORDER.indexOf(from)
  const nextIndex = ROOT_ROUTE_ORDER.indexOf(to)
  if (currentIndex < 0 || nextIndex < 0 || currentIndex === nextIndex) return 'still'
  return nextIndex > currentIndex ? 'forward' : 'back'
}

export function updateRouteTransition(
  toDepth: unknown,
  fromDepth: unknown,
  toRoot: RootRouteKey | null = null,
  fromRoot: RootRouteKey | null = null,
  routeChanged = false,
): void {
  const nextDepth = Number(toDepth || 0)
  const previousDepth = Number(fromDepth || 0)
  const rootDirection = classifyRootRouteDirection(fromRoot, toRoot)
  const isRootTransition = rootDirection !== 'still'

  routeTransitionTier.value = isRootTransition ? 'root' : 'secondary'
  if (isRootTransition) routeDirection.value = rootDirection
  else if (fromRoot && !toRoot) {
    routeDirection.value = 'forward'
  } else if (!fromRoot && toRoot) {
    routeDirection.value = 'back'
  } else if (nextDepth > previousDepth || (routeChanged && nextDepth === previousDepth)) {
    routeDirection.value = 'forward'
  } else if (nextDepth < previousDepth) {
    routeDirection.value = 'back'
  } else {
    routeDirection.value = 'still'
  }

  if (routeDirection.value === 'forward') routeTransitionName.value = 'route-forward'
  else if (routeDirection.value === 'back') routeTransitionName.value = 'route-back'
  else routeTransitionName.value = 'route-fade'
  routeTransitionSequence.value += 1
}
