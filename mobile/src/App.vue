<script setup lang="ts">
import { computed, onMounted, onUnmounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import AppBottomNav from '@/components/AppBottomNav.vue'
import PwaStatus from '@/components/PwaStatus.vue'
import RootHeader from '@/components/RootHeader.vue'
import SignalField from '@/components/SignalField.vue'
import {
  routeDirection,
  routeTransitionName,
  routeTransitionSequence,
  routeTransitionTier,
} from '@/core/navigation'
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
const showSignalField = computed(() => (
  showBottomNav.value
  && ['home', 'markets', 'assets', 'profile'].includes(String(route.name || ''))
))
const routeMotionClasses = computed(() => [
  `route-${routeDirection.value}`,
  `transition-${routeTransitionTier.value}`,
])

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
  <main
    class="app-stage"
    :class="theme.isDark ? 'theme-dark' : 'theme-light'"
    :data-route-direction="routeDirection"
    :data-transition-tier="routeTransitionTier"
    :data-motion-zone="showSignalField ? 'expressive' : 'protected'"
  >
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
      <div v-if="showSignalField" class="ambient-layer" aria-hidden="true">
        <SignalField :light="!theme.isDark" />
      </div>
      <div
        :key="`veil-${routeTransitionSequence}`"
        class="route-veil"
        :class="`route-veil-${routeTransitionTier}`"
        :data-direction="routeDirection"
        aria-hidden="true"
      >
        <span />
        <i />
      </div>
      <PwaStatus />
      <RootHeader v-if="showBottomNav" />
      <div class="app-route-host">
        <RouterView v-slot="{ Component, route: currentRoute }">
          <Transition :name="routeTransitionName">
            <component
              :is="Component"
              :key="currentRoute.fullPath"
              :class="[
                'app-route-layer',
                'view-stack',
                ...routeMotionClasses,
                { 'secondary-stack': !showBottomNav },
              ]"
            />
          </Transition>
        </RouterView>
      </div>
      <AppBottomNav v-if="showBottomNav" />
    </section>
  </main>
</template>
