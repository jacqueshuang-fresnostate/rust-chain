<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import {
  Activity,
  ArrowLeftRight,
  ChartNoAxesCombined,
  Gauge,
  House,
  UserRound,
  WalletCards,
} from 'lucide-vue-next'
import { useNavigationStore } from '@/stores/navigation'

const route = useRoute()
const navigation = useNavigationStore()
const { t } = useI18n()

const activeName = computed(() => String(route.name || ''))
const contractMode = computed(() => route.query.mode === 'contract')

const items = computed(() => [
  {
    key: 'home',
    label: t('nav.home'),
    to: { name: 'home' },
    icon: House,
    active: activeName.value === 'home',
  },
  {
    key: 'markets',
    label: t('nav.markets'),
    to: { name: 'markets' },
    icon: ChartNoAxesCombined,
    active: activeName.value === 'markets',
  },
  {
    key: 'spot',
    label: t('trade.spot'),
    to: { name: 'trade', params: { symbol: navigation.lastTradeSymbol } },
    icon: ArrowLeftRight,
    active: activeName.value === 'trade' && !contractMode.value,
  },
  {
    key: 'seconds',
    label: t('seconds.title'),
    to: { name: 'seconds' },
    icon: Gauge,
    active: activeName.value === 'seconds',
    primary: true,
  },
  {
    key: 'contract',
    label: t('trade.contract'),
    to: {
      name: 'trade',
      params: { symbol: navigation.lastTradeSymbol },
      query: { mode: 'contract' },
    },
    icon: Activity,
    active: activeName.value === 'trade' && contractMode.value,
  },
  {
    key: 'assets',
    label: t('nav.assets'),
    to: { name: 'assets' },
    icon: WalletCards,
    active: activeName.value === 'assets',
  },
  {
    key: 'profile',
    label: t('nav.profile'),
    to: { name: 'profile' },
    icon: UserRound,
    active: activeName.value === 'profile',
  },
])
</script>

<template>
  <nav class="bottom-nav" :aria-label="t('nav.main')">
    <RouterLink
      v-for="item in items"
      :key="item.key"
      v-slot="{ href, navigate }"
      :to="item.to"
      replace
      custom
    >
      <a
        :href="href"
        class="bottom-nav__item"
        :class="{ 'is-active': item.active, 'is-primary': item.primary }"
        :aria-current="item.active ? 'page' : undefined"
        @click="navigate"
      >
        <span class="bottom-nav__icon" aria-hidden="true">
          <component :is="item.icon" :size="item.primary ? 25 : 21" :stroke-width="item.active ? 2.35 : 2" />
        </span>
        <span class="bottom-nav__label">{{ item.label }}</span>
      </a>
    </RouterLink>
  </nav>
</template>

<style scoped>
.bottom-nav {
  align-items: end;
  background: var(--surface);
  border-top: 1px solid var(--line);
  bottom: 0;
  box-shadow: 0 -10px 28px rgb(15 23 42 / 8%);
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr)) minmax(56px, 1.2fr) repeat(3, minmax(0, 1fr));
  left: 50%;
  max-width: var(--app-max-width);
  min-height: calc(var(--bottom-nav-height) + env(safe-area-inset-bottom));
  padding: 8px 0 calc(7px + env(safe-area-inset-bottom));
  position: fixed;
  transform: translateX(-50%);
  width: 100%;
  z-index: var(--layer-navigation);
}

.bottom-nav::before {
  background: var(--surface);
  border: 1px solid var(--line);
  border-bottom: 0;
  border-radius: 48px 48px 0 0;
  content: '';
  height: 48px;
  left: 50%;
  pointer-events: none;
  position: absolute;
  top: -24px;
  transform: translateX(-50%);
  width: 70px;
}

.bottom-nav__item {
  align-items: center;
  color: var(--muted);
  display: flex;
  flex-direction: column;
  font-size: 9px;
  font-weight: 620;
  gap: 1px;
  justify-content: flex-end;
  line-height: 1.05;
  min-height: 62px;
  min-width: 0;
  position: relative;
  text-align: center;
  text-decoration: none;
  z-index: 1;
}

.bottom-nav__item:focus {
  outline: none;
}

.bottom-nav__item:focus-visible .bottom-nav__icon {
  box-shadow: 0 0 0 3px var(--focus-ring), inset 0 0 0 2px var(--focus);
}

.bottom-nav__icon {
  align-items: center;
  border: 1px solid transparent;
  border-radius: 15px;
  display: flex;
  flex: 0 0 44px;
  height: 44px;
  justify-content: center;
  transition:
    background-color var(--motion-fast) var(--motion-ease),
    box-shadow var(--motion-fast) var(--motion-ease),
    color var(--motion-fast) var(--motion-ease),
    transform var(--motion-fast) var(--motion-ease);
  width: 44px;
}

.bottom-nav__label {
  align-items: flex-start;
  display: flex;
  justify-content: center;
  max-width: 100%;
  min-height: 18px;
  overflow-wrap: anywhere;
  padding: 0 1px;
}

.bottom-nav__item.is-active {
  color: var(--ink);
  font-weight: 760;
}

.bottom-nav__item.is-active:not(.is-primary) .bottom-nav__icon {
  background: var(--soft);
  border-color: var(--line);
}

.bottom-nav__item.is-primary {
  color: var(--ink);
  gap: 2px;
}

.bottom-nav__item.is-primary .bottom-nav__icon {
  background: var(--ink);
  border: 4px solid var(--surface);
  border-radius: 50%;
  box-shadow: 0 5px 18px rgb(15 23 42 / 24%);
  color: var(--surface);
  flex-basis: 56px;
  height: 56px;
  margin-top: -28px;
  width: 56px;
}

.bottom-nav__item.is-primary.is-active .bottom-nav__icon {
  background: var(--accent);
  color: var(--on-accent);
}

@media (max-width: 360px) {
  .bottom-nav__item {
    font-size: 8px;
  }
}

@media (prefers-reduced-motion: reduce) {
  .bottom-nav__icon {
    transition: none;
  }
}
</style>
