 <template>
  <div class="p-4 md:p-8 max-w-7xl mx-auto space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-700">
    <div class="flex items-center justify-between border-b border-border/50 pb-6">
      <h1 class="text-4xl font-black flex items-center gap-3 text-transparent bg-clip-text bg-gradient-to-r from-primary to-neon-blue tracking-tighter">
        <span class="i-mdi-wallet text-primary text-4xl"></span>
        {{ t('nav.assets') }}
      </h1>
      <button @click="hideBalance = !hideBalance" class="flex items-center gap-2 text-sm text-primary hover:text-neon-blue transition-colors px-4 py-2 rounded-lg bg-primary/10 hover:bg-primary/20 border border-primary/20 backdrop-blur-sm">
         <span :class="hideBalance ? 'i-mdi-eye-off-outline' : 'i-mdi-eye-outline'" class="text-lg"></span>
         {{ hideBalance ? t('assets.show_balance') : t('assets.hide_balance') }}
      </button>
    </div>

    <!-- Account Type Tabs -->
    <div class="flex gap-2 bg-muted/20 p-1.5 rounded-xl w-fit border border-border/50 backdrop-blur-sm">
      <button
        @click="activeTab = 'spot'"
        :class="activeTab === 'spot' ? 'bg-primary text-primary-foreground shadow-[0_0_15px_rgba(var(--primary),0.4)] scale-100' : 'text-muted-foreground hover:text-foreground scale-95 hover:scale-100 hover:bg-muted/50'"
        class="px-6 py-2.5 text-sm font-bold rounded-lg transition-all duration-300 transform"
      >
        {{ t('assets.spot') }}
      </button>
      <button
        @click="activeTab = 'margin'"
        :class="activeTab === 'margin' ? 'bg-primary text-primary-foreground shadow-[0_0_15px_rgba(var(--primary),0.4)] scale-100' : 'text-muted-foreground hover:text-foreground scale-95 hover:scale-100 hover:bg-muted/50'"
        class="px-6 py-2.5 text-sm font-bold rounded-lg transition-all duration-300 transform"
      >
        {{ t('assets.margin') }}
      </button>
    </div>

    <!-- Overview Card -->
    <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
       <!-- Main Balance -->
       <div class="lg:col-span-2 bg-card/60 backdrop-blur-md border border-primary/20 rounded-2xl p-8 relative overflow-hidden group hover:border-primary/50 transition-colors duration-500 shadow-[0_8px_32px_rgba(0,0,0,0.4)] hover:shadow-[0_8px_32px_rgba(var(--primary),0.15)]">
         <!-- Ornamental glow -->
         <div class="absolute -top-32 -right-32 w-96 h-96 bg-primary/10 rounded-full blur-[100px] group-hover:bg-primary/20 transition-colors duration-700 pointer-events-none"></div>
         <div class="absolute -bottom-20 -left-20 w-64 h-64 bg-neon-blue/10 rounded-full blur-[80px] group-hover:bg-neon-blue/20 transition-colors duration-700 pointer-events-none"></div>

         <div class="relative z-10">
           <div class="text-sm text-primary/80 font-mono tracking-widest uppercase mb-2 flex items-center gap-2">
             <span class="i-mdi-chart-donut text-lg"></span>
             {{ activeTab === 'spot' ? t('assets.estimated_balance') : t('assets.margin_balance') }}
           </div>

           <div class="text-3xl md:text-4xl font-black font-mono flex flex-wrap items-baseline gap-2 md:gap-3 my-4">
             <span class="text-2xl text-muted-foreground/60">$</span>
             <span class="text-glow tracking-tight text-white break-all">{{ hideBalance ? '********' : totalBalanceUSD }}</span>
             <span class="text-base text-muted-foreground font-sans font-normal ml-1 shrink-0">≈ {{ hideBalance ? '***' : totalBalanceBTC }} BTC</span>
           </div>

           <!-- Margin Account Extra Info -->
           <div v-if="activeTab === 'margin'" class="flex flex-wrap gap-8 mt-6 p-4 bg-background/40 backdrop-blur rounded-xl border border-border/50">
             <div>
               <span class="text-muted-foreground text-xs uppercase tracking-wider block mb-1">{{ t('assets.risk_rate') }}</span>
               <div class="font-bold text-lg text-up flex items-center gap-1">
                  {{ marginRiskRateText }}
               </div>
             </div>
             <div class="w-px bg-border/50"></div>
             <div>
               <span class="text-muted-foreground text-xs uppercase tracking-wider block mb-1">{{ t('assets.margin_level') }}</span>
               <div class="font-bold text-lg text-primary flex items-center gap-1">
                  <span class="i-mdi-lightning-bolt"></span> {{ marginLeverageText }}
               </div>
             </div>
           </div>

           <!-- Actions -->
           <div class="flex flex-wrap gap-4 mt-8">
             <router-link to="/user/recharge" class="px-8 py-3 bg-primary hover:bg-primary/90 text-primary-foreground font-black tracking-wide rounded-xl transition-all shadow-[0_0_20px_rgba(var(--primary),0.4)] hover:shadow-[0_0_30px_rgba(var(--primary),0.6)] hover:-translate-y-0.5 flex items-center gap-2">
                <span class="i-mdi-arrow-down-circle-outline text-xl"></span> {{ t('assets.deposit') }}
             </router-link>
             <router-link to="/user/withdraw" class="px-8 py-3 bg-muted/50 hover:bg-muted text-foreground font-bold tracking-wide rounded-xl border border-border hover:border-border/80 transition-all backdrop-blur hover:-translate-y-0.5 flex items-center gap-2">
                <span class="i-mdi-arrow-up-circle-outline text-xl"></span> {{ t('assets.withdraw') }}
             </router-link>
           </div>
         </div>
       </div>

       <!-- PnL Card -->
       <div class="bg-card/40 backdrop-blur-md border border-border/50 rounded-2xl p-8 flex flex-col justify-center relative overflow-hidden group hover:border-up/30 transition-colors">
         <div class="absolute inset-0 bg-gradient-to-br from-up/5 to-transparent opacity-0 group-hover:opacity-100 transition-opacity duration-500"></div>
         <div class="relative z-10">
            <div class="w-12 h-12 rounded-xl bg-up/10 text-up flex items-center justify-center mb-6 border border-up/20">
               <span class="i-mdi-trending-up text-2xl"></span>
            </div>
            <div class="text-sm text-muted-foreground font-mono tracking-widest uppercase mb-2">
              {{ activeTab === 'spot' ? t('assets.today_pnl') : t('assets.unrealized_pnl') }}
            </div>
            <div class="text-3xl font-black font-mono mb-2" :class="pnlClass">
              {{ hideBalance ? '****' : pnlDisplay }}
            </div>
            <div class="inline-flex items-center gap-1.5 px-3 py-1 rounded-full text-sm font-bold border" :class="pnlBadgeClass">
              <span :class="pnlValue >= 0 ? 'i-mdi-arrow-top-right' : 'i-mdi-arrow-bottom-right'"></span> {{ pnlRatioDisplay }}
            </div>
         </div>
       </div>
    </div>

    <!-- Assets List -->
    <div class="bg-card/40 backdrop-blur-md border border-border/50 rounded-2xl overflow-hidden shadow-lg">
      <div class="p-6 border-b border-border/50 flex justify-between items-center bg-muted/10">
        <h3 class="font-black text-lg tracking-wide flex items-center gap-2">
           <span class="i-mdi-view-list text-primary"></span>
           {{ activeTab === 'spot' ? t('assets.crypto_assets') : t('assets.margin_assets') }}
        </h3>
        <div class="relative">
           <span class="i-mdi-magnify absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground w-5 h-5"></span>
           <input type="text" :placeholder="t('assets.search_placeholder')" class="pl-10 pr-4 py-2 bg-background/50 border border-border rounded-lg text-sm focus:outline-none focus:border-primary focus:ring-1 focus:ring-primary/50 transition-all font-mono" />
        </div>
      </div>

      <div class="divide-y divide-border/50">
        <div v-for="(asset, index) in displayAssets" :key="asset.coin"
             class="p-4 sm:p-6 flex flex-col sm:flex-row sm:items-center justify-between hover:bg-muted/40 transition-all duration-300 group cursor-pointer animate-in fade-in slide-in-from-right-4 fill-mode-both"
             :style="{ animationDelay: `${index * 100}ms` }"
        >
           <!-- Asset Info -->
           <div class="flex items-center gap-5 mb-4 sm:mb-0">
             <div class="w-12 h-12 rounded-xl bg-background border border-border/50 flex items-center justify-center text-2xl group-hover:scale-110 group-hover:border-primary/50 group-hover:shadow-[0_0_15px_rgba(var(--primary),0.3)] transition-all duration-300 relative overflow-hidden">
               <div class="absolute inset-0 bg-gradient-to-tr from-primary/10 to-transparent opacity-0 group-hover:opacity-100 transition-opacity"></div>
               <img
                 v-if="asset.logoUrl"
                 :src="asset.logoUrl"
                 :alt="t('assets.logo_alt', { symbol: asset.coin })"
                 class="relative z-10 w-8 h-8 rounded-full object-cover"
                 @error="markLogoFailed(asset.coin)"
               />
               <span
                 v-else
                 :class="asset.fallbackIcon"
                 class="relative z-10 text-foreground group-hover:text-primary transition-colors"
               ></span>
             </div>
             <div>
               <div class="font-black text-lg flex items-center gap-2">
                  {{ asset.coin }}
                  <span class="text-xs px-2 py-0.5 rounded-full bg-muted text-muted-foreground font-normal border border-border">{{ asset.name }}</span>
               </div>
               <div class="text-sm text-muted-foreground font-mono mt-0.5">{{ t('assets.price') }}: ${{ formatNumber(asset.price, 'price') }}</div>
             </div>
           </div>

           <!-- Balance Info -->
           <div class="sm:text-right flex sm:flex-col justify-between items-end border-t sm:border-0 border-border/50 pt-4 sm:pt-0">
             <div class="font-black font-mono text-xl group-hover:text-primary transition-colors">
                 {{ hideBalance ? '********' : formatNumber(asset.balance, 'amount') }}
             </div>
             <div class="text-sm text-muted-foreground font-mono">
                 ≈ ${{ hideBalance ? '****' : formatNumber(asset.balance * asset.price, 'price') }}
             </div>
           </div>
        </div>

        <div v-if="displayAssets.length === 0" class="p-12 text-center text-muted-foreground flex flex-col items-center justify-center">
             <span class="i-mdi-cube-off-outline text-6xl mb-4 opacity-20"></span>
             <p>{{ t('assets.no_assets') }}</p>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { useUserStore } from '@/stores/user'
import { useMarketStore } from '@/stores/market'
import { useContractStore } from '@/stores/contract'
import { useI18n } from 'vue-i18n'
import { formatNumber } from '@/utils/format'
import { getWallets, type MemberWallet } from '@/api/asset'

const { t } = useI18n()
const userStore = useUserStore()
const marketStore = useMarketStore()
const contractStore = useContractStore()

const activeTab = ref<'spot' | 'margin'>('spot')
const hideBalance = ref(false)
const wallets = ref<MemberWallet[]>([])
const failedLogoSymbols = ref<Set<string>>(new Set())

const getAssetPrice = (unit: string) => {
    const symbol = unit.toUpperCase()
    if (symbol === 'USDT') return 1
    const ticker = marketStore.tickers.find(t => t.symbol === `${symbol}/USDT`)
    return ticker ? ticker.close : 0
}
const getAssetIcon = (unit: string): string => {
  const symbol = unit.toUpperCase()
  if (symbol === 'USDT') return ''
  const ticker = marketStore.tickers.find(t => t.symbol === `${symbol}/USDT`)
  return ticker?.icon || ''
}

const btcPrice = computed(() => getAssetPrice('BTC') || 40000)

const normalizeLogoUrl = (value?: string | null) => value?.trim() || ''

const isImageUrl = (value: string) => /^(https?:|data:image|\/)/i.test(value.trim())

const defaultAssetIcon = (unit: string) => {
  const symbol = unit.toUpperCase()
  if (symbol === 'BTC') return 'i-mdi-bitcoin'
  if (symbol === 'USDT' || symbol === 'USD') return 'i-mdi-currency-usd'
  return 'i-mdi-cube-outline'
}

const resolveAssetVisual = (unit: string, walletLogoUrl?: string | null) => {
  const symbol = unit.toUpperCase()
  const walletLogo = normalizeLogoUrl(walletLogoUrl)
  const marketIcon = normalizeLogoUrl(getAssetIcon(symbol))
  const logoCandidate = walletLogo || (isImageUrl(marketIcon) ? marketIcon : '')
  return {
    logoUrl: logoCandidate && !failedLogoSymbols.value.has(symbol) ? logoCandidate : '',
    fallbackIcon: marketIcon && !isImageUrl(marketIcon) ? marketIcon : defaultAssetIcon(symbol),
  }
}

const markLogoFailed = (unit: string) => {
  const nextFailedLogos = new Set(failedLogoSymbols.value)
  nextFailedLogos.add(unit.toUpperCase())
  failedLogoSymbols.value = nextFailedLogos
}



// Spot Assets
const spotAssets = computed(() => {
    return wallets.value.map(w => {
      const coin = (w.coin.coinGroup || w.coin.unit || w.coin.name).toUpperCase()
      return {
        coin,
        name: w.coin.name || coin,
        ...resolveAssetVisual(coin, w.coin.logoUrl),
        balance: w.balance,
        price: getAssetPrice(coin)
      }
    })
})

const marginAssets = computed(() => {
  const grouped = new Map<string, { coin: string; name: string; logoUrl: string; fallbackIcon: string; balance: number; price: number }>()
  for (const wallet of contractStore.wallets) {
    const coin = (wallet.baseSymbol || 'USDT').toUpperCase()
    const existing = grouped.get(coin)
    const marginBalance = wallet.usdtBuyPrincipalAmount + wallet.usdtSellPrincipalAmount || wallet.usdtBalance
    if (existing) {
      existing.balance += marginBalance
      continue
    }
    grouped.set(coin, {
      coin,
      name: coin,
      ...resolveAssetVisual(coin),
      balance: marginBalance,
      price: getAssetPrice(coin),
    })
  }
  return Array.from(grouped.values())
})

const displayAssets = computed(() => activeTab.value === 'spot' ? spotAssets.value : marginAssets.value)

const totalUsdValue = computed(() => {
  const total = displayAssets.value.reduce((sum, asset) => sum + (asset.balance * asset.price), 0)
  return total
})

const totalBalanceUSD = computed(() => {
  return formatNumber(totalUsdValue.value, 'price')
})

const totalBalanceBTC = computed(() => {
  return formatNumber(totalUsdValue.value / btcPrice.value, 'amount')
})

const marginPositionCount = computed(() => {
  return contractStore.wallets.reduce((total, wallet) => {
    const longCount = wallet.usdtBuyPosition + wallet.usdtFrozenBuyPosition > 0 ? 1 : 0
    const shortCount = wallet.usdtSellPosition + wallet.usdtFrozenSellPosition > 0 ? 1 : 0
    return total + longCount + shortCount
  }, 0)
})

const marginUnrealizedPnl = computed(() => {
  return contractStore.wallets.reduce((sum, wallet) => {
    const currentPrice = contractStore.getThumbBySymbol(wallet.symbol)?.last || wallet.currentPrice || 0
    let pnl = 0
    if (currentPrice > 0 && wallet.usdtBuyPosition > 0 && wallet.usdtBuyPrice > 0) {
      pnl += (currentPrice / wallet.usdtBuyPrice - 1) * wallet.usdtBuyPosition * wallet.usdtShareNumber
    }
    if (currentPrice > 0 && wallet.usdtSellPosition > 0 && wallet.usdtSellPrice > 0) {
      pnl += (1 - currentPrice / wallet.usdtSellPrice) * wallet.usdtSellPosition * wallet.usdtShareNumber
    }
    return sum + pnl
  }, 0)
})

const marginRiskRate = computed(() => {
  const margin = marginAssets.value.reduce((sum, asset) => sum + asset.balance, 0)
  if (margin <= 0) return null
  return (margin + marginUnrealizedPnl.value) / margin
})

const marginRiskRateText = computed(() => {
  if (marginRiskRate.value === null) return '--'
  return `${formatNumber(marginRiskRate.value * 100, 'amount')}%`
})

const marginLeverageText = computed(() => {
  const maxLeverage = Math.max(
    0,
    ...contractStore.wallets.map((wallet) => Math.max(wallet.usdtBuyLeverage, wallet.usdtSellLeverage))
  )
  return maxLeverage > 0 && marginPositionCount.value > 0 ? `${maxLeverage}x` : '--'
})

const pnlValue = computed(() => activeTab.value === 'margin' ? marginUnrealizedPnl.value : 0)
const pnlDisplay = computed(() => `${pnlValue.value >= 0 ? '+' : '-'}$${formatNumber(Math.abs(pnlValue.value), 'price')}`)
const pnlRatioDisplay = computed(() => {
  const principal = activeTab.value === 'margin'
    ? marginAssets.value.reduce((sum, asset) => sum + asset.balance, 0)
    : totalUsdValue.value
  if (principal <= 0) return '0.00%'
  const ratio = pnlValue.value / principal * 100
  return `${ratio >= 0 ? '+' : '-'}${formatNumber(Math.abs(ratio), 'amount')}%`
})
const pnlClass = computed(() => pnlValue.value >= 0 ? 'text-up' : 'text-down')
const pnlBadgeClass = computed(() => pnlValue.value >= 0 ? 'bg-up/10 text-up border-up/20' : 'bg-down/10 text-down border-down/20')

const loadWallets = async () => {
    try {
        const res = await getWallets()
        if (res.data?.code === 0) {
            wallets.value = res.data.data
        }
    } catch (error) {
        console.error('Failed to load wallets:', error)
    }
}

const loadMarginWallets = async () => {
    try {
        await Promise.all([contractStore.loadWallets(), contractStore.loadThumbs()])
    } catch (error) {
        console.error('Failed to load margin wallets:', error)
    }
}

onMounted(() => {
    if (userStore.isLoggedIn) {
        loadWallets()
        loadMarginWallets()
    }

    // Trigger market store fetch if tickers are empty
    if (marketStore.tickers.length === 0) {
        import('@/api/market').then(({ fetchMarketSnapshot }) => {
            fetchMarketSnapshot().then(res => {
                const data = Array.isArray(res.data) ? res.data : (res.data?.data || [])
                marketStore.setTickers(data)
            })
        }).catch(err => console.error(err))
    }
})

watch(activeTab, (tab) => {
    if (tab === 'margin' && userStore.isLoggedIn) {
        loadMarginWallets()
    }
})
</script>
