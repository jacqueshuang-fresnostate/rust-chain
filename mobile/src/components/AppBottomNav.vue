<script setup lang="ts">
import { computed, ref, type Component } from 'vue'
import { useRoute, useRouter, type RouteLocationRaw } from 'vue-router'
import { useI18n } from 'vue-i18n'
import {
  ArrowDownUp,
  ArrowLeftRight,
  CandlestickChart,
  ChartNoAxesCombined,
  House,
  RefreshCw,
  UserRound,
  WalletCards,
  X,
  Zap,
} from 'lucide-vue-next'
import { createBottomNavSecondsTarget } from '@/core/navigation'
import { useModalDialog } from '@/core/modalDialog'
import { useNavigationStore } from '@/stores/navigation'

type TradeDestination = 'spot' | 'contract' | 'seconds' | 'swap'
type RootNavigationKey = 'home' | 'markets' | 'trade' | 'assets' | 'profile'

interface RootNavigationItem {
  key: RootNavigationKey
  label: string
  to?: RouteLocationRaw
  icon: Component
  active: boolean
  primary?: boolean
}

interface TradePickerOption {
  value: TradeDestination
  label: string
  icon: Component
}

const route = useRoute()
const router = useRouter()
const navigation = useNavigationStore()
const { t } = useI18n()

const tradePickerOpen = ref(false)
const tradePickerDialog = ref<HTMLElement | null>(null)
const tradeTrigger = ref<HTMLButtonElement | null>(null)
const {
  trapFocus: trapTradePickerFocus,
  setReturnFocus,
} = useModalDialog(tradePickerOpen, tradePickerDialog, '[data-trade-picker-close]')

const activeName = computed(() => String(route.name || ''))

const items = computed<RootNavigationItem[]>(() => [
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
    icon: ArrowLeftRight,
    active: ['trade', 'seconds', 'swap'].includes(activeName.value),
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

const tradeOptions = computed<TradePickerOption[]>(() => [
  {
    value: 'spot',
    label: t('trade.spot'),
    icon: RefreshCw,
  },
  {
    value: 'contract',
    label: t('trade.contract'),
    icon: CandlestickChart,
  },
  {
    value: 'seconds',
    label: t('home.secondsShortcut'),
    icon: Zap,
  },
  {
    value: 'swap',
    label: t('trade.swap'),
    icon: ArrowDownUp,
  },
])

function createTradeTarget(mode: 'spot' | 'contract'): RouteLocationRaw {
  return {
    name: 'trade',
    params: { symbol: navigation.lastTradeSymbol },
    query: mode === 'contract' ? { mode: 'contract' } : undefined,
  }
}

function openTradePicker(trigger: HTMLButtonElement): void {
  tradeTrigger.value = trigger
  setReturnFocus(tradeTrigger.value)
  tradePickerOpen.value = true
}

function closeTradePicker(): void {
  tradePickerOpen.value = false
}

function selectRoot(item: RootNavigationItem, event: MouseEvent): void {
  if (item.primary) {
    if (event.currentTarget instanceof HTMLButtonElement) openTradePicker(event.currentTarget)
    return
  }
  if (item.to) void router.replace(item.to)
}

function handleTradePickerKeydown(event: KeyboardEvent): void {
  trapTradePickerFocus(event, closeTradePicker)
}

async function selectTradeDestination(destination: TradeDestination): Promise<void> {
  closeTradePicker()

  if (destination === 'spot') {
    navigation.rememberTradeMode('spot')
    await router.replace(createTradeTarget('spot'))
    return
  }
  if (destination === 'contract') {
    navigation.rememberTradeMode('contract')
    await router.replace(createTradeTarget('contract'))
    return
  }
  if (destination === 'seconds') {
    await router.replace(createBottomNavSecondsTarget())
    return
  }
  await router.push({ name: 'swap' })
}
</script>

<template>
  <nav
    class="bottom-nav"
    :class="{ 'trade-picker-open': tradePickerOpen }"
    :aria-label="t('nav.main')"
  >
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
        :aria-haspopup="item.primary ? 'dialog' : undefined"
        :aria-expanded="item.primary ? tradePickerOpen : undefined"
        :aria-controls="item.primary ? 'root-trade-navigation-picker' : undefined"
        @click="selectRoot(item, $event)"
      >
        <span class="bottom-nav__icon" aria-hidden="true">
          <component :is="item.icon" :size="item.primary ? 24 : 21" :stroke-width="item.active ? 2.35 : 2" />
        </span>
        <small class="bottom-nav__label">{{ item.label }}</small>
      </button>
    </div>
  </nav>

  <Teleport to="body">
    <Transition name="trade-picker">
      <div
        v-if="tradePickerOpen"
        class="trade-navigation-picker"
        data-pencil-source="X0ux9F RtubA U99rP n1BXc eLvdo QrlAB"
        @click.self="closeTradePicker"
      >
        <section
          id="root-trade-navigation-picker"
          ref="tradePickerDialog"
          class="trade-navigation-picker__dialog"
          role="dialog"
          aria-modal="true"
          aria-labelledby="root-trade-navigation-picker-title"
          tabindex="-1"
          @keydown="handleTradePickerKeydown"
        >
          <h2 id="root-trade-navigation-picker-title" class="trade-navigation-picker__title">
            {{ t('nav.tradePickerTitle') }}
          </h2>
          <svg
            class="trade-navigation-picker__shape"
            viewBox="0 0 358 300"
            preserveAspectRatio="none"
            aria-hidden="true"
          >
            <path d="M26 0h306c14.36 0 26 11.64 26 26v252c0 12.15-9.85 22-22 22h-112c-6 0-10.5-4-11.5-10-3-18.5-17-32-33.5-32-16.5 0-30.5 13.5-33.5 32-1 6-5.5 10-11.5 10h-112c-12.15 0-22-9.85-22-22v-252c0-14.36 11.64-26 26-26z" />
          </svg>
          <div class="trade-navigation-picker__options" :aria-label="t('nav.tradePickerTitle')">
            <button
              v-for="option in tradeOptions"
              :key="option.value"
              type="button"
              class="trade-navigation-picker__option"
              @click="selectTradeDestination(option.value)"
            >
              <component :is="option.icon" :size="25" :stroke-width="2" aria-hidden="true" />
              <span>{{ option.label }}</span>
            </button>
          </div>
          <button
            class="trade-navigation-picker__close"
            type="button"
            data-trade-picker-close
            :aria-label="t('common.close')"
            @click="closeTradePicker"
          >
            <X :size="28" :stroke-width="2" aria-hidden="true" />
          </button>
        </section>
      </div>
    </Transition>
  </Teleport>
</template>
