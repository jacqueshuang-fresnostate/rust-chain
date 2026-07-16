<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ArrowLeftRight, ChartNoAxesCombined, House, UserRound, WalletCards } from 'lucide-vue-next'
import { useNavigationStore } from '@/stores/navigation'

const route = useRoute()
const navigation = useNavigationStore()
const { t } = useI18n()

const items = computed(() => [
  { key: 'home', label: t('nav.home'), to: '/', icon: House, match: ['home'] },
  { key: 'markets', label: t('nav.markets'), to: '/markets', icon: ChartNoAxesCombined, match: ['markets', 'market-detail'] },
  { key: 'trade', label: t('nav.trade'), to: navigation.lastTradePath, icon: ArrowLeftRight, match: ['trade'], primary: true },
  { key: 'assets', label: t('nav.assets'), to: '/assets', icon: WalletCards, match: ['assets'] },
  { key: 'profile', label: t('nav.profile'), to: '/profile', icon: UserRound, match: ['profile'] },
])

const activeName = computed(() => String(route.name || ''))
</script>

<template>
  <nav class="bottom-nav" :aria-label="t('nav.main')">
    <RouterLink
      v-for="item in items"
      :key="item.key"
      :to="item.to"
      replace
      class="bottom-nav__item"
      :class="{ 'is-active': item.match.includes(activeName), 'is-primary': item.primary }"
    >
      <span class="bottom-nav__icon"><component :is="item.icon" :size="item.primary ? 24 : 21" /></span>
      <span>{{ item.label }}</span>
    </RouterLink>
  </nav>
</template>

<style scoped>
.bottom-nav {
  align-items: flex-end;
  backdrop-filter: blur(16px);
  background: color-mix(in srgb, var(--surface) 94%, transparent);
  border-top: 1px solid rgb(230 233 236 / 88%);
  bottom: 0;
  display: grid;
  grid-template-columns: repeat(5, 1fr);
  left: 50%;
  max-width: var(--app-max-width);
  min-height: calc(66px + env(safe-area-inset-bottom));
  padding: 9px 8px calc(8px + env(safe-area-inset-bottom));
  position: fixed;
  transform: translateX(-50%);
  width: 100%;
  z-index: 40;
}

.bottom-nav__item {
  align-items: center;
  color: var(--muted);
  display: flex;
  flex-direction: column;
  font-size: 11px;
  gap: 4px;
  justify-content: flex-end;
  line-height: 1.1;
  min-height: 50px;
  text-decoration: none;
}

.bottom-nav__item:focus { outline: none; }
.bottom-nav__item:focus-visible { border-radius: 6px; box-shadow: inset 0 0 0 2px color-mix(in srgb, var(--accent) 55%, transparent); }

.bottom-nav__icon {
  align-items: center;
  display: flex;
  height: 24px;
  justify-content: center;
}

.bottom-nav__item.is-active { color: var(--ink); font-weight: 750; }

.bottom-nav__item.is-primary .bottom-nav__icon {
  background: var(--ink);
  border: 4px solid var(--surface);
  border-radius: 50%;
  box-shadow: 0 4px 16px rgb(15 23 42 / 20%);
  color: white;
  height: 52px;
  margin-top: -28px;
  width: 52px;
}

.bottom-nav__item.is-primary { gap: 7px; }
</style>
