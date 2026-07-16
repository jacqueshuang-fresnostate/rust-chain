/// <reference types="vite/client" />

import type { RouteLocationRaw } from 'vue-router'

declare module 'vue-router' {
  interface RouteMeta {
    backFallback?: RouteLocationRaw
    depth?: number
    showBottomNav?: boolean
  }
}
