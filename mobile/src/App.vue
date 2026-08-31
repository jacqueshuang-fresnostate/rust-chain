<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import AppBottomNav from '@/components/AppBottomNav.vue'
import LaunchIntro from '@/components/LaunchIntro.vue'
import PwaStatus from '@/components/PwaStatus.vue'
import RootHeader from '@/components/RootHeader.vue'
import SignalField from '@/components/SignalField.vue'
import {
  resolveRouteShellVisibility,
  routeDirection,
  routeTransitionName,
  routeTransitionSequence,
  routeTransitionTier,
} from '@/core/navigation'
import { useRouteAccessibility } from '@/core/navigation/useRouteAccessibility'
import { useSessionStore } from '@/stores/session'
import { useMarketFavoritesStore } from '@/stores/marketFavorites'
import { useThemeStore } from '@/stores/theme'
import stageLogo from '@/assets/brand/hippo-logo-landscape.png'
import stageImage from '@/assets/brand/signal-theatre.webp'

const route = useRoute()
const router = useRouter()
const session = useSessionStore()
const marketFavorites = useMarketFavoritesStore()
const theme = useThemeStore()
const { locale, t } = useI18n()
const showDesktopStageImage = ref(false)
let desktopStageMedia: MediaQueryList | null = null
const routeAccessibility = useRouteAccessibility({
  route,
  locale,
  translate(key, values) {
    return values ? t(key, { ...values }) : t(key)
  },
})
const shellVisibility = computed(() => resolveRouteShellVisibility(
  route.name,
  route.query.mode,
  route.query.purpose,
  route.meta.showBottomNav,
))
const showBottomNav = computed(() => shellVisibility.value.showBottomNav)
const showRootHeader = computed(() => (
  shellVisibility.value.showRootHeader
  && ['home', 'markets'].includes(String(route.name || ''))
))
const showSignalField = computed(() => shellVisibility.value.showSignalField)
const rootSurface = computed(() => (
  ['trade', 'message-center'].includes(String(route.name || '')) ? 'protected' : 'expressive'
))
const routeMotionClasses = computed(() => [
  `route-${routeDirection.value}`,
  `transition-${routeTransitionTier.value}`,
])

theme.initializeTheme()

watch(() => session.token, (token) => {
  marketFavorites.reset()
  if (token) void marketFavorites.load()
}, { immediate: true })

function handleAuthExpired() {
  session.sync()
  if (!['login', 'login-two-factor'].includes(String(route.name || ''))) {
    void router.replace({ name: 'login', query: { redirect: route.fullPath } })
  }
}

function syncDesktopStageImage(event?: MediaQueryListEvent): void {
  showDesktopStageImage.value = event?.matches ?? desktopStageMedia?.matches ?? false
}

watch(() => session.externalLogoutVersion, (current, previous) => {
  if (current > previous) handleAuthExpired()
})

onMounted(() => {
  desktopStageMedia = window.matchMedia('(min-width: 821px)')
  syncDesktopStageImage()
  desktopStageMedia.addEventListener('change', syncDesktopStageImage)
  session.initialize()
  window.addEventListener('hippo-mobile-auth-expired', handleAuthExpired)
})
onUnmounted(() => {
  desktopStageMedia?.removeEventListener('change', syncDesktopStageImage)
  desktopStageMedia = null
  window.removeEventListener('hippo-mobile-auth-expired', handleAuthExpired)
  session.dispose()
})
</script>

<template>
  <div
    class="app-stage"
    :class="theme.isDark ? 'theme-dark' : 'theme-light'"
    :data-route-direction="routeDirection"
    :data-transition-tier="routeTransitionTier"
    :data-motion-zone="showSignalField ? 'expressive' : 'protected'"
  >
    <a
      class="route-skip-link"
      :href="`#${routeAccessibility.mainContentId}`"
      @click.prevent="routeAccessibility.focusMainContent"
    >
      {{ t('routeAccessibility.skipToMain') }}
    </a>
    <p
      class="route-announcer sr-only"
      aria-live="polite"
      aria-atomic="true"
    >
      {{ routeAccessibility.announcement.value }}
    </p>
    <div class="stage-art" aria-hidden="true">
      <div v-if="showDesktopStageImage" class="stage-image" :style="{ backgroundImage: `url(${stageImage})` }" />
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
      <LaunchIntro />
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
      <RootHeader v-if="showRootHeader" />
      <div class="app-route-host">
        <RouterView v-slot="{ Component, route: currentRoute }">
          <Transition
            :name="routeTransitionName"
            @after-enter="routeAccessibility.handleRouteEntered"
          >
            <component
              :is="Component"
              :key="currentRoute.fullPath"
              :data-route-accessibility-key="routeAccessibility.renderKeyFor(currentRoute)"
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
  </div>
</template>
