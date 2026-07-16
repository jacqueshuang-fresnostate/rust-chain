import { ref } from 'vue'
import type { RouteLocationRaw, Router } from 'vue-router'

export const DEFAULT_TRADE_SYMBOL = 'BTC_USDT'
export const routeTransitionName = ref('route-fade')

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

export function updateRouteTransition(toDepth: unknown, fromDepth: unknown): void {
  const nextDepth = Number(toDepth || 0)
  const previousDepth = Number(fromDepth || 0)
  if (nextDepth > previousDepth) routeTransitionName.value = 'route-forward'
  else if (nextDepth < previousDepth) routeTransitionName.value = 'route-back'
  else routeTransitionName.value = 'route-fade'
}
