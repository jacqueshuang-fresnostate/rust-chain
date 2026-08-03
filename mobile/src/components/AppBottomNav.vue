<script setup lang="ts">
import { computed } from 'vue'
import { useRoute, useRouter, type RouteLocationRaw } from 'vue-router'
import { useI18n } from 'vue-i18n'
import {
  ArrowLeftRight,
  ChartNoAxesCombined,
  House,
  UserRound,
  WalletCards,
} from 'lucide-vue-next'
import { useNavigationStore } from '@/stores/navigation'

const route = useRoute()
const router = useRouter()
const navigation = useNavigationStore()
const { t } = useI18n()

const activeName = computed(() => String(route.name || ''))
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
    key: 'trade',
    label: t('nav.trade'),
    to: {
      name: 'trade',
      params: { symbol: navigation.lastTradeSymbol },
      query: navigation.lastTradeMode === 'contract' ? { mode: 'contract' } : undefined,
    },
    icon: ArrowLeftRight,
    active: activeName.value === 'trade' || activeName.value === 'seconds',
    primary: true,
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

function selectRoot(to: RouteLocationRaw): void {
  void router.replace(to)
}
</script>

<template>
  <nav class="bottom-nav" :aria-label="t('nav.main')">
    <div class="bottom-nav__dock">
      <button
        v-for="item in items"
        :key="item.key"
        type="button"
        class="bottom-nav__item"
        :class="{ active: item.active, 'trade-nav-action': item.primary }"
        :data-nav-key="item.key"
        :aria-label="item.label"
        :aria-current="item.active ? 'page' : undefined"
        @click="selectRoot(item.to)"
      >
        <span class="bottom-nav__icon" aria-hidden="true">
          <component :is="item.icon" :size="item.primary ? 24 : 21" :stroke-width="item.active ? 2.35 : 2" />
        </span>
        <small class="bottom-nav__label">{{ item.label }}</small>
      </button>
    </div>
  </nav>
</template>
