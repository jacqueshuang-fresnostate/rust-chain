<script setup lang="ts">
import { computed, onMounted, onUnmounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import AppBottomNav from '@/components/AppBottomNav.vue'
import PwaStatus from '@/components/PwaStatus.vue'
import { routeTransitionName } from '@/core/navigation'
import { useSessionStore } from '@/stores/session'
import { useThemeStore } from '@/stores/theme'

const route = useRoute()
const router = useRouter()
const session = useSessionStore()
const theme = useThemeStore()
const showBottomNav = computed(() => route.meta.showBottomNav !== false && !(route.name === 'markets' && route.query.purpose === 'trade'))

theme.initializeTheme()

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
  <div class="app-frame" :class="{ 'app-frame--with-bottom-nav': showBottomNav }">
    <PwaStatus />
    <div class="app-route-host">
      <RouterView v-slot="{ Component, route: currentRoute }">
        <Transition :name="routeTransitionName">
          <component :is="Component" :key="currentRoute.fullPath" class="app-route-layer" />
        </Transition>
      </RouterView>
    </div>
    <AppBottomNav v-if="showBottomNav" />
  </div>
</template>

<style>
.app-route-host {
  min-height: 100dvh;
  position: relative;
}
.app-route-layer {
  min-width: 0;
  width: 100%;
}
.route-forward-enter-active,.route-forward-leave-active,.route-back-enter-active,.route-back-leave-active,.route-fade-enter-active,.route-fade-leave-active {
  transition: opacity var(--motion-medium) var(--motion-ease), transform var(--motion-medium) var(--motion-ease);
}
.route-forward-enter-active,.route-back-enter-active,.route-fade-enter-active { position: relative; z-index: var(--layer-route-transition); }
.route-forward-leave-active,.route-back-leave-active,.route-fade-leave-active {
  inset: 0;
  pointer-events: none;
  position: absolute;
  width: 100%;
  z-index: var(--layer-content);
}
.route-forward-enter-from { opacity: 0; transform: translateX(8px); }
.route-forward-leave-to { opacity: 0; transform: translateX(-6px); }
.route-back-enter-from { opacity: 0; transform: translateX(-8px); }
.route-back-leave-to { opacity: 0; transform: translateX(6px); }
.route-fade-enter-from,.route-fade-leave-to { opacity: 0; }
@media (prefers-reduced-motion: reduce) {
  .route-forward-enter-active,.route-forward-leave-active,.route-back-enter-active,.route-back-leave-active,.route-fade-enter-active,.route-fade-leave-active {
    transition: none;
  }
  .route-forward-enter-from,.route-forward-leave-to,.route-back-enter-from,.route-back-leave-to {
    transform: none;
  }
}
</style>
