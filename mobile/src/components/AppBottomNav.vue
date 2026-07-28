<script setup lang="ts">
import { computed } from 'vue'
import { useRoute, useRouter, type RouteLocationRaw } from 'vue-router'
import { useI18n } from 'vue-i18n'
import {
  Activity,
  ArrowLeftRight,
  ChartNoAxesCombined,
  House,
  UserRound,
  WalletCards,
  Zap,
} from 'lucide-vue-next'
import { useNavigationStore } from '@/stores/navigation'

const route = useRoute()
const router = useRouter()
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
    icon: Zap,
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

function selectRoot(to: RouteLocationRaw): void {
  void router.replace(to)
}
</script>

<template>
  <nav class="bottom-nav" :aria-label="t('nav.main')">
    <button
      v-for="item in items"
      :key="item.key"
      type="button"
      :class="{ active: item.active, 'seconds-nav-action': item.primary }"
      :aria-current="item.active ? 'page' : undefined"
      @click="selectRoot(item.to)"
    >
      <span aria-hidden="true">
        <component :is="item.icon" :size="item.primary ? 22 : 19" :stroke-width="item.active ? 2.35 : 2" />
      </span>
      <small>{{ item.label }}</small>
    </button>
  </nav>
</template>
