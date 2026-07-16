<script setup lang="ts">
import { computed, onMounted, onUnmounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import AppBottomNav from '@/components/AppBottomNav.vue'
import { routeTransitionName } from '@/core/navigation'
import { useSessionStore } from '@/stores/session'

const route = useRoute()
const router = useRouter()
const session = useSessionStore()
const showBottomNav = computed(() => route.meta.showBottomNav !== false && !(route.name === 'markets' && route.query.purpose === 'trade'))

function handleAuthExpired() {
  session.logout()
  if (!['login', 'login-two-factor'].includes(String(route.name || ''))) {
    void router.replace({ name: 'login', query: { redirect: route.fullPath } })
  }
}

onMounted(() => window.addEventListener('hippo-mobile-auth-expired', handleAuthExpired))
onUnmounted(() => window.removeEventListener('hippo-mobile-auth-expired', handleAuthExpired))
</script>

<template>
  <div class="app-frame">
    <RouterView v-slot="{ Component, route: currentRoute }">
      <Transition :name="routeTransitionName">
        <component :is="Component" :key="currentRoute.fullPath" />
      </Transition>
    </RouterView>
    <AppBottomNav v-if="showBottomNav" />
  </div>
</template>

<style>
.route-forward-enter-active,.route-forward-leave-active,.route-back-enter-active,.route-back-leave-active,.route-fade-enter-active,.route-fade-leave-active { transition: opacity 150ms ease, transform 180ms ease; }
.route-forward-leave-active,.route-back-leave-active,.route-fade-leave-active { inset: 0; pointer-events: none; position: absolute; width: 100%; }
.route-forward-enter-from { opacity: 0; transform: translateX(12px); }
.route-forward-leave-to { opacity: 0; transform: translateX(-8px); }
.route-back-enter-from { opacity: 0; transform: translateX(-12px); }
.route-back-leave-to { opacity: 0; transform: translateX(8px); }
.route-fade-enter-from,.route-fade-leave-to { opacity: 0; }
@media (prefers-reduced-motion: reduce) {
  .route-forward-enter-active,.route-forward-leave-active,.route-back-enter-active,.route-back-leave-active,.route-fade-enter-active,.route-fade-leave-active { transition: none; }
}
</style>
