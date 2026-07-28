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
  background: transparent;
  border-top: 0;
  bottom: 0;
  box-shadow: none;
  display: grid;
  grid-template-columns: repeat(7, minmax(44px, 1fr));
  height: calc(var(--bottom-nav-height) + env(safe-area-inset-bottom));
  isolation: isolate;
  left: 50%;
  max-width: var(--app-max-width);
  min-height: calc(var(--bottom-nav-height) + env(safe-area-inset-bottom));
  overflow: visible;
  padding: 18px 2px env(safe-area-inset-bottom);
  position: fixed;
  transform: translateX(-50%);
  width: 100%;
  z-index: var(--layer-navigation);
}

.bottom-nav::before {
  background: var(--surface);
  clip-path: polygon(
    0 14px,
    34% 14px,
    38% 9px,
    42% 0,
    58% 0,
    62% 9px,
    66% 14px,
    100% 14px,
    100% 100%,
    0 100%
  );
  content: '';
  inset: 0;
  pointer-events: none;
  position: absolute;
  filter: drop-shadow(0 -1px 0 var(--line-strong));
  z-index: -2;
}

.bottom-nav::after {
  background: linear-gradient(
    90deg,
    var(--line-strong) 0 34%,
    transparent 34% 66%,
    var(--line-strong) 66% 100%
  );
  content: '';
  height: 1px;
  left: 0;
  pointer-events: none;
  position: absolute;
  right: 0;
  top: 14px;
  z-index: -1;
}

.bottom-nav__item {
  align-items: center;
  color: var(--muted);
  display: flex;
  flex-direction: column;
  font-size: 9px;
  font-weight: 650;
  gap: 1px;
  justify-content: flex-end;
  line-height: 1.05;
  min-height: 66px;
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
  border-radius: 4px;
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
  display: -webkit-box;
  line-height: 1.05;
  max-width: 100%;
  min-height: 16px;
  overflow: hidden;
  overflow-wrap: anywhere;
  padding: 0 1px;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}

.bottom-nav__item.is-active {
  color: var(--ink);
  font-weight: 760;
}

.bottom-nav__item.is-active:not(.is-primary) .bottom-nav__icon {
  background: linear-gradient(var(--signal-green), var(--signal-green)) center / 28px 28px no-repeat;
  border-color: transparent;
  color: var(--on-positive);
}

.bottom-nav__item.is-primary {
  color: var(--ink);
  gap: 2px;
}

.bottom-nav__item.is-primary .bottom-nav__icon {
  background: var(--signal-green);
  border: 4px solid var(--surface);
  border-radius: 50%;
  box-shadow: 0 0 0 1px var(--line-strong);
  color: var(--on-positive);
  flex-basis: 48px;
  height: 48px;
  margin-top: -24px;
  width: 48px;
}

.bottom-nav__item.is-primary.is-active .bottom-nav__icon {
  background: var(--signal-green);
  box-shadow: 0 0 0 1px var(--line-strong), 0 0 0 5px color-mix(in srgb, var(--signal-green) 12%, transparent);
  color: var(--on-positive);
}

@media (max-width: 360px) {
  .bottom-nav {
    padding-inline: 0;
  }

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
