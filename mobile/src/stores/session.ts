import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import {
  clearAuthTokens,
  readAuthSessionSnapshot,
  registerSessionInvalidationHook,
  startAuthSessionSynchronization,
  stopAuthSessionSynchronization,
  subscribeAuthSession,
  synchronizeAuthSession,
} from '@/api/client'
import { referenceRequestRegistry } from '@/api/requestCache'
import type { SessionSnapshot, SessionTransition } from '@/core/sessionOwner'

export const useSessionStore = defineStore('mobile-session', () => {
  const initial = readAuthSessionSnapshot()
  const token = ref(initial.accessToken)
  const epoch = ref(initial.epoch)
  const identityScope = ref(initial.scope)
  const persistence = ref(initial.persistence)
  const externalLogoutVersion = ref(0)
  const isAuthenticated = computed(() => Boolean(token.value))
  const generation = computed(() => epoch.value)
  let unsubscribe: (() => void) | null = null
  let unregisterCacheInvalidation: (() => void) | null = null

  function applySnapshot(snapshot: SessionSnapshot): void {
    token.value = snapshot.accessToken
    epoch.value = snapshot.epoch
    identityScope.value = snapshot.scope
    persistence.value = snapshot.persistence
  }

  function applyTransition(transition: SessionTransition): void {
    applySnapshot(transition.current)
    if (
      transition.external
      && Boolean(transition.previous.accessToken)
      && !transition.current.accessToken
    ) {
      externalLogoutVersion.value += 1
    }
  }

  function ensureSubscription(): void {
    if (!unsubscribe) unsubscribe = subscribeAuthSession(applyTransition)
    if (!unregisterCacheInvalidation) {
      unregisterCacheInvalidation = registerSessionInvalidationHook(() => {
        // Global generation invalidation also prevents old in-flight loaders
        // from repopulating identity-scoped reference caches after logout.
        referenceRequestRegistry.invalidate()
      })
    }
  }

  function initialize(): void {
    ensureSubscription()
    startAuthSessionSynchronization()
    sync()
  }

  function sync(): void {
    ensureSubscription()
    applySnapshot(synchronizeAuthSession())
  }

  function logout(): void {
    clearAuthTokens()
  }

  function dispose(): void {
    stopAuthSessionSynchronization()
    unsubscribe?.()
    unsubscribe = null
    unregisterCacheInvalidation?.()
    unregisterCacheInvalidation = null
  }

  ensureSubscription()

  return {
    token,
    epoch,
    generation,
    identityScope,
    persistence,
    externalLogoutVersion,
    isAuthenticated,
    initialize,
    sync,
    logout,
    dispose,
  }
})
