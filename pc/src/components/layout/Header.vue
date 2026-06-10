<template>
  <header data-tauri-drag-region class="h-16 bg-card/80 backdrop-blur border-b border-border flex items-center px-6 justify-between select-none sticky top-0 z-50">
    <!-- Logo & Nav -->
    <div class="flex items-center gap-8">
      <div class="flex items-center gap-2 text-primary font-bold text-2xl cursor-pointer tracking-tighter" @click="$router.push('/')">
        <img src="@/assets/logo/logo.png" class="w-28 h-11 object-contain drop-shadow-neon" alt="Hippo" />
      </div>

      <nav class="hidden md:flex gap-6 text-sm font-medium items-center ml-2">
        <router-link to="/market" class="text-muted-foreground hover:text-primary transition-colors hover:text-glow py-4">
          {{ t('nav.markets') }}
        </router-link>

        <router-link to="/launchpad" class="text-muted-foreground hover:text-primary transition-colors hover:text-glow py-4">
          {{ t('nav.launchpad') }}
        </router-link>

        <router-link to="/finance" class="text-muted-foreground hover:text-primary transition-colors hover:text-glow py-4">
          {{ t('nav.finance') }}
        </router-link>


        <!-- Trade Dropdown -->
        <div class="relative group">
          <button class="flex items-center gap-1 text-muted-foreground hover:text-primary transition-colors hover:text-glow focus:outline-none py-4">
            {{ t('nav.trade') }}
            <Icon icon="mdi:chevron-down" class="w-3 h-3 transition-transform group-hover:rotate-180" />
          </button>

          <!-- Dropdown Content -->
          <div class="absolute top-full left-0 bg-card border border-border rounded shadow-xl opacity-0 invisible group-hover:opacity-100 group-hover:visible transition-all duration-200 transform translate-y-2 group-hover:translate-y-0 flex overflow-hidden">
            <!-- Main Actions -->
            <div class="w-40 py-2 border-r border-border bg-muted/20 flex flex-col shrink-0">
              <div
                 class="block px-4 py-3 text-sm text-muted-foreground hover:bg-muted hover:text-primary hover:text-glow transition-colors cursor-pointer relative"
                 @mouseenter="hoveredItem = 'spot'"
              >
                <div class="flex items-center font-bold">
                  {{ t('nav.spot') }}
                </div>
                <!-- Active Indicator -->
                <div v-if="hoveredItem === 'spot'" class="absolute left-0 top-0 bottom-0 w-1 bg-primary"></div>
              </div>

              <router-link
                 to="/swap"
                 class="block px-4 py-3 text-sm text-muted-foreground hover:bg-muted hover:text-primary hover:text-glow transition-colors"
                 @mouseenter="hoveredItem = 'swap'"
              >
                <div class="flex items-center font-bold">
                  {{ t('nav.swap') }}
                </div>
              </router-link>

              <router-link
                 to="/second"
                 class="block px-4 py-3 text-sm text-muted-foreground hover:bg-muted hover:text-primary hover:text-glow transition-colors"
                 @mouseenter="hoveredItem = 'binary'"
              >
                <div class="flex items-center font-bold">
                   {{ t('nav.binary') }}
                </div>
              </router-link>

              <router-link
                 to="/contract"
                 class="block px-4 py-3 text-sm text-muted-foreground hover:bg-muted hover:text-primary hover:text-glow transition-colors"
                 @mouseenter="hoveredItem = 'contract'"
              >
                <div class="flex items-center font-bold">
                   {{ t('nav.contract') }}
                </div>
              </router-link>
            </div>

            <!-- Ticker List (Only visible when hoveredItem is spot) -->
            <div v-if="hoveredItem === 'spot'" class="w-[280px] flex flex-col bg-card animate-in fade-in slide-in-from-left-2 duration-200">
               <div class="px-4 py-3 text-xs font-bold text-muted-foreground uppercase tracking-wider border-b border-border bg-muted/10 flex justify-between shrink-0">
                   <span>{{ t('nav.market_col') }}</span>
                   <span>{{ t('nav.price_col') }}</span>
               </div>
               <div class="flex-1 overflow-y-auto custom-scrollbar p-1 max-h-[360px]">
                  <div v-for="ticker in allTickers" :key="ticker.symbol"
                       class="px-3 py-2.5 hover:bg-muted/50 rounded-md cursor-pointer transition-colors flex justify-between items-center group/item mb-0.5 shrink-0"
                       @click.stop="goToTrade(ticker.symbol)">
                      <div class="flex flex-col gap-0.5">
                          <span class="text-sm font-bold font-mono group-hover/item:text-primary flex items-center gap-1">
                            {{ ticker.symbol.split('/')[0] }}
                            <span class="text-[10px] text-muted-foreground font-normal">/{{ ticker.symbol.split('/')[1] }}</span>
                          </span>
                          <span class="text-[10px] text-muted-foreground font-mono">{{ t('nav.vol_col') }} {{ formatVolume(ticker.volume) }}</span>
                      </div>
                      <div class="flex flex-col items-end gap-0.5">
                          <span class="text-sm font-mono font-bold tracking-tight" :class="getPriceColor(ticker.chg)">{{ ticker.close }}</span>
                          <span class="text-[10px] font-mono font-medium px-1.5 py-0.5 rounded-sm bg-muted" :class="getChangeColorClass(ticker.chg)">
                              {{ ticker.chg >= 0 ? '+' : '' }}{{ formatChange(ticker.chg) }}%
                          </span>
                      </div>
                  </div>
               </div>
            </div>
          </div>
        </div>

        <router-link to="/user/assets" class="text-muted-foreground hover:text-primary transition-colors hover:text-glow py-4">
          {{ t('nav.assets') }}
        </router-link>
      </nav>
    </div>

    <!-- Right Actions -->
    <div class="flex items-center gap-4">
      <!-- Theme Switcher -->
      <!-- <button @click="toggleTheme" class="p-2 rounded-full hover:bg-muted transition-colors">
        <Icon v-if="settingStore.theme === 'dark'" icon="mdi:white-balance-sunny" class="w-5 h-5" />
        <Icon v-else icon="mdi:moon-waning-crescent" class="w-5 h-5" />
      </button> -->

      <!-- Language Switcher -->
      <button @click="showLangModal = true" class="flex items-center gap-1.5 text-sm font-medium text-muted-foreground hover:text-foreground transition-colors">
        <Icon icon="mdi:web" class="w-4 h-4" />
        {{ currentLangLabel }}
        <Icon icon="mdi:chevron-down" class="w-3 h-3" />
      </button>

      <div class="h-4 w-[1px] bg-border mx-2"></div>

      <!-- Auth Buttons / User Profile -->
      <template v-if="!userStore.isLoggedIn">
        <button @click="login" class="px-4 py-2 text-sm font-bold border border-primary text-primary bg-transparent rounded hover:bg-primary/10 transition-all">
          {{ t('nav.login') }}
        </button>
        <button @click="signup" class="px-4 py-2 text-sm font-bold bg-primary text-primary-foreground rounded hover:bg-primary/90 transition-all box-glow">
          {{ t('nav.signup') }}
        </button>
      </template>

      <template v-else>
         <router-link to="/user" class="flex items-center gap-2 px-3 py-1.5 bg-muted rounded border border-border hover:border-primary transition-colors cursor-pointer">
           <div class="w-6 h-6 rounded-full bg-primary flex items-center justify-center text-primary-foreground font-bold text-xs">
             {{ userStore.user?.username?.charAt(0).toUpperCase() || 'U' }}
           </div>
           <span class="text-sm font-medium">{{ userStore.user?.username || 'User' }}</span>
         </router-link>
         <button @click="logout" class="ml-2 text-muted-foreground hover:text-destructive transition-colors">
            <Icon icon="mdi:logout" class="w-5 h-5" />
         </button>
      </template>
    </div>

    <!-- Language Modal -->
    <Teleport to="body">
      <div v-if="showLangModal" class="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-[9999]" @click.self="showLangModal = false">
        <div class="w-80 rounded-xl overflow-hidden" style="background: rgba(10, 10, 18, 0.95); border: 1px solid rgba(255,255,255,0.08); box-shadow: 0 20px 60px rgba(0,0,0,0.8);">
          <div class="flex items-center justify-between px-5 py-4 border-b" style="border-color: rgba(255,255,255,0.06);">
            <span class="text-sm font-bold text-white">{{ t('settings.language') }}</span>
            <button @click="showLangModal = false" class="text-muted-foreground hover:text-white transition-colors">
              <Icon icon="mdi:close" class="w-4 h-4" />
            </button>
          </div>
          <div class="py-2">
            <button
              v-for="lang in languages" :key="lang.code"
              @click="selectLang(lang.code)"
              class="w-full flex items-center justify-between px-5 py-3 transition-colors"
              :class="locale === lang.code ? 'bg-primary/8' : 'hover:bg-white/[0.03]'"
            >
              <div class="flex items-center gap-3">
                <span class="text-lg">{{ lang.flag }}</span>
                <div class="flex flex-col items-start">
                  <span class="text-sm font-medium" :class="locale === lang.code ? 'text-primary' : 'text-white'">{{ lang.native }}</span>
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
import { Icon } from '@iconify/vue'
import numeral from 'numeral'

const { t, locale } = useI18n()
const router = useRouter()
const settingStore = useSettingStore()
const userStore = useUserStore()
const marketStore = useMarketStore()

const hoveredItem = ref<string | null>(null)
const showLangModal = ref(false)

const languages = [
  { code: 'en', label: 'English', native: 'English', flag: '🇺🇸' },
  { code: 'zh', label: 'Chinese', native: '简体中文', flag: '🇨🇳' },
]

const currentLangLabel = computed(() => {
  const lang = languages.find(l => l.code === locale.value)
  return lang?.native || locale.value.toUpperCase()
})

// All Tickers for Dropdown
const allTickers = computed(() => {
    return marketStore.tickers
})

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

function goToTrade(symbol: string) {
    marketStore.setActiveSymbol(symbol)
    const urlSymbol = symbol.replace('/', '_')
    router.push({ name: 'Trade', params: { symbol: urlSymbol } })
}



function selectLang(code: string) {
  locale.value = code as 'en' | 'zh'
  settingStore.setLocale(code as 'en' | 'zh')
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
  text-shadow: 0 0 10px hsl(var(--primary));
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