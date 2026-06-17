<template>
  <header data-pc-header data-tauri-drag-region class="sticky top-0 z-50 shrink-0 select-none border-b border-border/70 bg-background/95 px-4 backdrop-blur-xl md:px-6">
    <div class="mx-auto flex h-16 max-w-[1440px] items-center justify-between gap-4">
      <div class="flex min-w-0 items-center gap-5 lg:gap-7">
        <button
          type="button"
          class="flex h-10 w-28 shrink-0 items-center justify-start rounded-full transition-colors hover:bg-muted/60 focus:outline-none focus-visible:ring-2 focus-visible:ring-primary/50"
          :aria-label="settingStore.platformName"
          @click="goHome"
        >
          <BrandLogo container-class="flex items-center" image-class="h-9 w-24 object-contain" />
        </button>

        <nav class="hidden items-center gap-1 md:flex">
          <router-link v-for="item in primaryNavItems" :key="item.to" :to="item.to" :class="navLinkClass">
            <Icon :icon="item.icon" class="h-4 w-4" />
            <span>{{ item.label }}</span>
          </router-link>

          <div class="group relative" @mouseleave="hoveredItem = null">
            <button
              type="button"
              class="inline-flex h-9 items-center gap-1.5 rounded-full px-3 text-sm font-semibold text-muted-foreground transition-colors hover:bg-muted/70 hover:text-foreground focus:outline-none focus-visible:ring-2 focus-visible:ring-primary/50"
              :aria-label="t('nav.trade')"
              @mouseenter="hoveredItem = 'spot'"
            >
              <Icon icon="mdi:swap-horizontal-bold" class="h-4 w-4" />
              <span>{{ t('nav.trade') }}</span>
              <Icon icon="mdi:chevron-down" class="h-3.5 w-3.5 transition-transform group-hover:rotate-180" />
            </button>

            <div
              data-pc-header-trade-menu
              class="invisible absolute left-0 top-full z-50 mt-3 flex w-[620px] overflow-hidden rounded-2xl border border-border/80 bg-card shadow-2xl opacity-0 ring-1 ring-white/5 transition-all duration-200 group-hover:visible group-hover:translate-y-0 group-hover:opacity-100"
            >
              <div class="w-56 shrink-0 border-r border-border/70 bg-muted/20 p-2">
                <button type="button" :class="productItemClass('spot')" @click="goSpot" @mouseenter="hoveredItem = 'spot'">
                  <span class="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-background text-foreground">
                    <Icon icon="mdi:chart-candlestick" class="h-5 w-5" />
                  </span>
                  <span class="flex min-w-0 flex-col">
                    <span class="truncate text-sm font-semibold">{{ t('nav.spot') }}</span>
                    <span class="truncate text-xs text-muted-foreground">{{ t('nav.spot_desc') }}</span>
                  </span>
                </button>

                <router-link to="/swap" :class="productItemClass('swap')" @mouseenter="hoveredItem = 'swap'">
                  <span class="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-background text-foreground">
                    <Icon icon="mdi:swap-horizontal" class="h-5 w-5" />
                  </span>
                  <span class="flex min-w-0 flex-col">
                    <span class="truncate text-sm font-semibold">{{ t('nav.swap') }}</span>
                    <span class="truncate text-xs text-muted-foreground">{{ t('nav.swap_desc') }}</span>
                  </span>
                </router-link>

                <router-link to="/second" :class="productItemClass('binary')" @mouseenter="hoveredItem = 'binary'">
                  <span class="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-background text-foreground">
                    <Icon icon="mdi:timer-sand" class="h-5 w-5" />
                  </span>
                  <span class="flex min-w-0 flex-col">
                    <span class="truncate text-sm font-semibold">{{ t('nav.binary') }}</span>
                    <span class="truncate text-xs text-muted-foreground">{{ t('nav.binary_desc') }}</span>
                  </span>
                </router-link>

                <router-link to="/contract" :class="productItemClass('contract')" @mouseenter="hoveredItem = 'contract'">
                  <span class="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-background text-foreground">
                    <Icon icon="mdi:finance" class="h-5 w-5" />
                  </span>
                  <span class="flex min-w-0 flex-col">
                    <span class="truncate text-sm font-semibold">{{ t('nav.contract') }}</span>
                    <span class="truncate text-xs text-muted-foreground">{{ t('nav.contract_desc') }}</span>
                  </span>
                </router-link>
              </div>

              <div class="flex min-h-[360px] flex-1 flex-col bg-card">
                <div class="flex items-center justify-between border-b border-border/70 px-4 py-3">
                  <div>
                    <div class="text-sm font-semibold text-foreground">{{ t('nav.top_pairs') }}</div>
                    <div class="text-xs text-muted-foreground">{{ t('nav.market_col') }} / {{ t('nav.price_col') }}</div>
                  </div>
                  <router-link to="/market" class="rounded-full px-3 py-1.5 text-xs font-semibold text-primary transition-colors hover:bg-primary/10">
                    {{ t('nav.view_all') }}
                  </router-link>
                </div>

                <div v-if="dropdownTickers.length" class="custom-scrollbar flex-1 overflow-y-auto p-2">
                  <button
                    v-for="ticker in dropdownTickers"
                    :key="ticker.symbol"
                    type="button"
                    class="group/item flex w-full items-center justify-between rounded-xl px-3 py-2.5 text-left transition-colors hover:bg-muted/60"
                    @click.stop="goToTrade(ticker.symbol)"
                  >
                    <span class="flex min-w-0 items-center gap-3">
                      <PairLogo class="h-8 w-8" :symbol="ticker.symbol" :src="ticker.icon" />
                      <span class="flex min-w-0 flex-col gap-0.5">
                        <span class="flex min-w-0 items-center gap-1 truncate font-mono text-sm font-semibold text-foreground group-hover/item:text-primary">
                          {{ ticker.symbol.split('/')[0] }}
                          <span class="truncate text-[10px] font-normal text-muted-foreground">/{{ ticker.symbol.split('/')[1] }}</span>
                        </span>
                        <span class="font-mono text-[10px] text-muted-foreground">{{ t('nav.vol_col') }} {{ formatVolume(ticker.volume) }}</span>
                      </span>
                    </span>
                    <span class="flex shrink-0 flex-col items-end gap-1">
                      <span class="font-mono text-sm font-semibold tracking-tight" :class="getPriceColor(ticker.chg)">{{ ticker.close }}</span>
                      <span class="rounded-full px-2 py-0.5 font-mono text-[10px] font-semibold" :class="getChangeColorClass(ticker.chg)">
                        {{ ticker.chg >= 0 ? '+' : '' }}{{ formatChange(ticker.chg) }}%
                      </span>
                    </span>
                  </button>
                </div>

                <div v-else class="flex flex-1 items-center justify-center px-6 text-sm text-muted-foreground">
                  {{ t('market.no_markets') }}
                </div>
              </div>
            </div>
          </div>

          <router-link to="/user/assets" :class="navLinkClass">
            <Icon icon="mdi:wallet-outline" class="h-4 w-4" />
            <span>{{ t('nav.assets') }}</span>
          </router-link>
        </nav>
      </div>

      <div class="flex shrink-0 items-center gap-2">
        <button
          type="button"
          class="inline-flex h-9 items-center gap-1.5 rounded-full px-3 text-sm font-semibold text-muted-foreground transition-colors hover:bg-muted/70 hover:text-foreground focus:outline-none focus-visible:ring-2 focus-visible:ring-primary/50"
          @click="showLangModal = true"
        >
          <Icon icon="mdi:web" class="h-4 w-4" />
          <span class="hidden lg:inline">{{ currentLangLabel }}</span>
          <Icon icon="mdi:chevron-down" class="h-3.5 w-3.5" />
        </button>

        <template v-if="!userStore.isLoggedIn">
          <button
            type="button"
            class="hidden h-9 items-center rounded-full px-4 text-sm font-semibold text-muted-foreground transition-colors hover:bg-muted/70 hover:text-foreground sm:inline-flex"
            @click="login"
          >
            {{ t('nav.login') }}
          </button>
          <button
            type="button"
            class="inline-flex h-9 items-center rounded-full bg-primary px-4 text-sm font-semibold text-primary-foreground transition-colors hover:bg-primary/90"
            @click="signup"
          >
            {{ t('nav.signup') }}
          </button>
        </template>

        <template v-else>
          <router-link to="/user" class="flex h-9 items-center gap-2 rounded-full border border-border bg-card px-2.5 pr-3 transition-colors hover:border-primary/60 hover:bg-muted/60">
            <span class="flex h-6 w-6 items-center justify-center overflow-hidden rounded-full bg-primary text-xs font-bold text-primary-foreground">
              <img v-if="userAvatarUrl" :src="userAvatarUrl" :alt="userDisplayName" class="h-full w-full object-cover" />
              <template v-else>{{ userInitial }}</template>
            </span>
            <span class="max-w-28 truncate text-sm font-semibold">{{ userDisplayName }}</span>
          </router-link>
          <button
            type="button"
            class="inline-flex h-9 w-9 items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive"
            :aria-label="t('common.logout')"
            @click="logout"
          >
            <Icon icon="mdi:logout" class="h-5 w-5" />
          </button>
        </template>
      </div>
    </div>

    <!-- Language Modal -->
    <Teleport to="body">
      <div v-if="showLangModal" class="fixed inset-0 z-[9999] flex items-center justify-center bg-black/50 backdrop-blur-sm" @click.self="showLangModal = false">
        <div class="w-80 overflow-hidden rounded-2xl border border-border bg-card shadow-2xl">
          <div class="flex items-center justify-between border-b border-border px-5 py-4">
            <span class="text-sm font-bold text-foreground">{{ t('settings.language') }}</span>
            <button type="button" class="text-muted-foreground transition-colors hover:text-foreground" @click="showLangModal = false">
              <Icon icon="mdi:close" class="w-4 h-4" />
            </button>
          </div>
          <div class="py-2">
            <button
              v-for="lang in availableLanguages" :key="lang.code"
              @click="selectLang(lang.code)"
              class="w-full flex items-center justify-between px-5 py-3 transition-colors"
              :class="locale === lang.code ? 'bg-primary/10' : 'hover:bg-muted/60'"
            >
              <div class="flex items-center gap-3">
                <span class="text-lg">{{ lang.flag }}</span>
                <div class="flex flex-col items-start">
                  <span class="text-sm font-medium" :class="locale === lang.code ? 'text-primary' : 'text-foreground'">{{ lang.native }}</span>
                  <span class="text-[11px] text-muted-foreground">{{ lang.label }}</span>
                </div>
              </div>
              <div v-if="locale === lang.code" class="w-5 h-5 rounded-full bg-primary flex items-center justify-center">
                <Icon icon="mdi:check" class="w-3.5 h-3.5 text-primary-foreground" />
              </div>
            </button>
          </div>
        </div>
      </div>
    </Teleport>
  </header>
</template>

<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { computed, ref } from 'vue'
import { useSettingStore } from '@/stores/setting'
import { useUserStore } from '@/stores/user'
import { useMarketStore } from '@/stores/market'
import BrandLogo from '@/components/common/BrandLogo.vue'
import PairLogo from '@/components/common/PairLogo.vue'
import { Icon } from '@iconify/vue'
import numeral from 'numeral'

const { t, locale } = useI18n()
const router = useRouter()
const settingStore = useSettingStore()
const userStore = useUserStore()
const marketStore = useMarketStore()

type HeaderProductKey = 'spot' | 'swap' | 'binary' | 'contract'

const hoveredItem = ref<HeaderProductKey | null>(null)
const showLangModal = ref(false)
const navLinkClass = 'inline-flex h-9 items-center gap-1.5 rounded-full px-3 text-sm font-semibold text-muted-foreground transition-colors hover:bg-muted/70 hover:text-foreground focus:outline-none focus-visible:ring-2 focus-visible:ring-primary/50'

const primaryNavItems = computed(() => [
  { to: '/market', icon: 'mdi:chart-line', label: t('nav.markets') },
  { to: '/launchpad', icon: 'mdi:rocket-launch-outline', label: t('nav.launchpad') },
  { to: '/finance', icon: 'mdi:percent-outline', label: t('nav.finance') },
  { to: '/loan', icon: 'mdi:cash-fast', label: t('nav.loan') },
  { to: '/prediction', icon: 'mdi:chart-timeline-variant-shimmer', label: t('nav.prediction') },
])

const activeProduct = computed<HeaderProductKey>(() => hoveredItem.value || 'spot')

const languageOptions = [
  { code: 'en' as const, label: 'English', native: 'English', flag: '🇺🇸' },
  { code: 'zh' as const, label: 'Chinese', native: '简体中文', flag: '🇨🇳' },
]

const availableLanguages = computed(() => {
  const supportedLocales = userStore.user?.supportedLocales
  if (!Array.isArray(supportedLocales) || supportedLocales.length === 0) return languageOptions
  return languageOptions.filter((language) => supportedLocales.includes(language.code))
})

const currentLangLabel = computed(() => {
  const lang = availableLanguages.value.find(l => l.code === locale.value)
  return lang?.native || locale.value.toUpperCase()
})

// All Tickers for Dropdown
const allTickers = computed(() => {
    return marketStore.tickers
})

const dropdownTickers = computed(() => {
    return allTickers.value.slice(0, 8)
})

const userDisplayName = computed(() => userStore.user?.username || userStore.user?.email || userStore.user?.phone || t('common.user'))
const userAvatarUrl = computed(() => userStore.user?.avatar || userStore.user?.avatarString || '')
const userInitial = computed(() => userDisplayName.value.trim().charAt(0).toUpperCase() || 'U')

function formatVolume(val: number) {
    return numeral(val).format('0.0a').toUpperCase()
}

function formatChange(val: number) {
    return numeral(val).format('0.00')
}

function getPriceColor(chg: number) {
    return chg >= 0 ? 'text-green-500' : 'text-red-500'
}

function getChangeColorClass(chg: number) {
    return chg >= 0 ? 'text-green-500 bg-green-500/10' : 'text-red-500 bg-red-500/10'
}

function productItemClass(key: HeaderProductKey) {
    return [
        'flex w-full items-center gap-3 rounded-xl px-3 py-3 text-left transition-colors',
        activeProduct.value === key
            ? 'bg-background text-foreground shadow-sm'
            : 'text-muted-foreground hover:bg-background/80 hover:text-foreground',
    ]
}

function goHome() {
    router.push('/')
}

function goSpot() {
    router.push('/spot')
}

function goToTrade(symbol: string) {
    marketStore.setActiveSymbol(symbol)
    const urlSymbol = symbol.replace('/', '_')
    router.push({ name: 'Trade', params: { symbol: urlSymbol } })
}



function selectLang(code: 'en' | 'zh') {
  locale.value = code
  settingStore.setManualLocale(code)
  showLangModal.value = false
}

function login() {
  router.push('/login')
}

function signup() {
  router.push('/register')
}

function logout() {
  userStore.logout()
  router.push('/login')
}
</script>

<style scoped>
.router-link-active {
  color: hsl(var(--primary));
  background: hsl(var(--muted) / 0.7);
}
/* Custom Scrollbar for dropdown */
.custom-scrollbar::-webkit-scrollbar {
  width: 4px;
}
.custom-scrollbar::-webkit-scrollbar-track {
  background: transparent;
}
.custom-scrollbar::-webkit-scrollbar-thumb {
  background: hsl(var(--muted-foreground) / 0.3);
  border-radius: 4px;
}
.custom-scrollbar::-webkit-scrollbar-thumb:hover {
  background: hsl(var(--muted-foreground) / 0.5);
}
</style>
