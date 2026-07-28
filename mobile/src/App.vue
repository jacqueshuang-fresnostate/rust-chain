<script setup lang="ts">
import { computed, onMounted, onUnmounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import AppBottomNav from '@/components/AppBottomNav.vue'
import PwaStatus from '@/components/PwaStatus.vue'
import RootHeader from '@/components/RootHeader.vue'
import { routeTransitionName } from '@/core/navigation'
import { useSessionStore } from '@/stores/session'
import { useThemeStore } from '@/stores/theme'
import stageLogo from '@/assets/brand/hippo-logo-landscape.png'
import stageImage from '@/assets/brand/signal-theatre.png'

const route = useRoute()
const router = useRouter()
const session = useSessionStore()
const theme = useThemeStore()
const { t } = useI18n()
const showBottomNav = computed(() => route.meta.showBottomNav !== false && !(route.name === 'markets' && route.query.purpose === 'trade'))
const rootSurface = computed(() => ['trade'].includes(String(route.name || '')) ? 'protected' : 'expressive')

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
  <main class="app-stage" :class="theme.isDark ? 'theme-dark' : 'theme-light'">
    <div class="stage-art" aria-hidden="true">
      <div class="stage-image" :style="{ backgroundImage: `url(${stageImage})` }" />
      <span
        class="stage-brand-lockup"
        :style="{ backgroundImage: `url(${stageLogo})` }"
      />
      <span class="stage-index">{{ t('rootPrototype.desktopStageIndex') }}</span>
      <p>
        {{ t('rootPrototype.desktopStageMottoLine1') }}<br />
        {{ t('rootPrototype.desktopStageMottoLine2') }}<br />
        {{ t('rootPrototype.desktopStageMottoLine3') }}
      </p>
      <div class="stage-tape">
        <span>{{ t('rootPrototype.desktopStagePairBtc') }}</span>
        <span>{{ t('rootPrototype.desktopStagePairEth') }}</span>
        <span>{{ t('rootPrototype.desktopStagePairSol') }}</span>
      </div>
      <div class="stage-caption">
        <span>{{ t('rootPrototype.desktopStageInstrument') }}</span>
        <span>{{ t('rootPrototype.desktopStageLocation') }}</span>
      </div>
    </div>
    <section
      class="app-frame mobile-canvas"
      :class="{ 'app-frame--with-bottom-nav': showBottomNav }"
      :data-surface="showBottomNav ? rootSurface : 'protected'"
    >
      <PwaStatus />
      <RootHeader v-if="showBottomNav" />
      <div class="app-route-host view-stack" :class="{ 'secondary-stack': !showBottomNav }">
        <RouterView v-slot="{ Component, route: currentRoute }">
          <Transition :name="routeTransitionName">
            <component :is="Component" :key="currentRoute.fullPath" class="app-route-layer" />
          </Transition>
        </RouterView>
      </div>
      <AppBottomNav v-if="showBottomNav" />
    </section>
  </main>
</template>

<style>
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
