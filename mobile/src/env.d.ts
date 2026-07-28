/// <reference types="vite/client" />
/// <reference types="vite-plugin-pwa/client" />

import type { RouteLocationRaw } from 'vue-router'

declare global {
  const __PWA_ENABLED__: boolean
}

declare module 'vue-router' {
  interface RouteMeta {
    backFallback?: RouteLocationRaw
    depth?: number
    showBottomNav?: boolean
  }
}
