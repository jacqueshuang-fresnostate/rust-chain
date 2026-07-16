<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { formatNumber } from '@/utils/format'
import { stompService } from '@/api/stomp'
import { useToast } from 'vue-toastification'
import { Icon } from '@iconify/vue'
import MarketChart from '@/components/chart/MarketChart.vue'
import AuthRequiredState from '@/components/common/AuthRequiredState.vue'
import PairLogo from '@/components/common/PairLogo.vue'
import numeral from 'numeral'
import { useSecondStore, type SecondOrder } from '@/stores/second'
import { useSettingStore } from '@/stores/setting'
import { useI18n } from 'vue-i18n'
import { useAuthRequired } from '@/composables/useAuthRequired'

const toast = useToast()
const route = useRoute()
const router = useRouter()
const store = useSecondStore()
const settingStore = useSettingStore()
const { t } = useI18n()
const { isLoggedIn, goToLogin } = useAuthRequired()

const symbol = ref(route.params.symbol ? (route.params.symbol as string).replace('_', '/') : 'BTC/USDT')
const amount = ref<number>(100)
const selectedCycleId = ref<number | null>(null)
const loading = ref(false)

// Active/History tab
const orderTab = ref<'active' | 'history'>('active')

// Filter Text for SideBar
const filterText = ref('')

// Computed
const currentTicker = computed(() => store.getTickerBySymbol(symbol.value))
const currentPrice = computed(() => currentTicker.value?.close || 0)
const precision = computed(() => {
    const val = currentPrice.value
    if (val === 0) return 2
    if (val < 0.1) return 6
    if (val < 1) return 4
    return 2
})
const usdtBalance = computed(() => isLoggedIn.value ? store.balance : 0)
const usdtBalanceText = computed(() => isLoggedIn.value ? `${formatNumber(usdtBalance.value)} USDT` : '--')

const selectedCycle = computed(() => store.cycles.find(c => c.id === selectedCycleId.value))
const visibleCycles = computed(() => store.cycles.filter(c => !c.symbol || c.symbol === symbol.value))

const payoutRate = computed(() => {
    const cycle = selectedCycle.value
    return cycle ? (cycle.cycleRate || 0) : 0
})

const payoutPercent = computed(() => {
    return numeral(payoutRate.value * 100).format('0')
})

const coinSymbol = 'USDT-ERC20'

// Market List for Sidebar
const marketList = computed(() => {
    if (!filterText.value) return store.tickers
    const lower = filterText.value.toLowerCase()
    return store.tickers.filter(t => t.symbol.toLowerCase().includes(lower))
})

// Countdown Logic
const orderCountdowns = ref<Record<number, number>>({})
let intervalId: any
let tickerSubs: Array<{ unsubscribe: () => void }> = []
let privateSub: { unsubscribe: () => void } | null = null

const privateEventType = (msg: { body: string }) => {
    try {
        return String(JSON.parse(msg.body)?.type || '')
    } catch {
        return ''
    }
}

const handlePrivateEvent = async (msg: { body: string }) => {
    if (!isLoggedIn.value) return
    const type = privateEventType(msg)
    if (type && !type.startsWith('seconds_contract.') && !type.startsWith('wallet.')) return
    await Promise.all([
        store.loadBalance(),
        store.loadCurrentOrders(symbol.value),
        store.loadHistoryOrders(symbol.value, 0)
    ])
}

const changeSymbol = (newSymbol: string) => {
    symbol.value = newSymbol
    router.replace({ params: { symbol: newSymbol.replace('/', '_') } })
    selectDefaultCycleForSymbol()
    loadOrders()
}

const selectDefaultCycleForSymbol = () => {
    const nextCycle = visibleCycles.value[0]
    selectedCycleId.value = nextCycle?.id ?? null
}

const clearTickerSubscriptions = () => {
    tickerSubs.forEach(sub => sub.unsubscribe())
    tickerSubs = []
}

const subscribeSecondTickers = async () => {
    clearTickerSubscriptions()
    const symbols = [...new Set(store.tickers.map(ticker => ticker.symbol).filter(Boolean))]
    tickerSubs = await Promise.all(symbols.map(symbol =>
        stompService.subscribe('seconds', `seconds:ticker:${symbol}`, (msg) => {
            try {
                store.updateTicker(JSON.parse(msg.body))
            } catch (e) {
                console.error('Failed to parse seconds ticker', e)
            }
        }),
    ))
}

const syncSymbolWithSecondsProducts = () => {
    if (store.tickers.length === 0) return
    const exists = store.tickers.some(ticker => ticker.symbol === symbol.value)
    if (exists) return

    symbol.value = store.tickers[0].symbol
    router.replace({ params: { symbol: symbol.value.replace('/', '_') } })
}

const loadOrders = async () => {
    if (!isLoggedIn.value) {
        store.currentOrders = []
        store.historyOrders = []
        return
    }
    if (orderTab.value === 'active') {
        await store.loadCurrentOrders(symbol.value)
    } else {
        await store.loadHistoryOrders(symbol.value, 0)
    }
}

const handleHistoryScroll = async (e: Event) => {
    const target = e.target as HTMLElement
    // trigger when within 10px of bottom
    if (target.scrollHeight - target.scrollTop - target.clientHeight < 10) {
        if (!store.loadingHistory && store.historyHasMore) {
            await store.loadHistoryOrders(symbol.value, store.historyPage + 1)
        }
    }
}

const handleOrder = async (direction: 0 | 1) => {
    if (!isLoggedIn.value) {
        goToLogin()
        return
    }
    if (!selectedCycleId.value) {
        toast.error(t('seconds.select_cycle_error'))
        return
    }
    if (!amount.value || amount.value <= 0) {
        toast.error(t('seconds.invalid_amount'))
        return
    }
    const cycle = selectedCycle.value
    if (cycle) {
        if (amount.value < cycle.minAmount) {
            toast.error(t('seconds.min_amount_error', { amount: cycle.minAmount }))
            return
        }
        if (cycle.maxAmount > 0 && amount.value > cycle.maxAmount) {
            toast.error(t('seconds.max_amount_error', { amount: cycle.maxAmount }))
            return
        }
    }

    loading.value = true
    try {
        await store.placeOrder({
            symbol: symbol.value,
            coinSymbol: coinSymbol,
            direction,
            cycleId: selectedCycleId.value,
            productId: cycle?.productId,
            durationSeconds: cycle?.cycleLength,
            amount: amount.value
        })
        toast.success(direction === 0 ? t('seconds.call_success') : t('seconds.put_success'))
        orderTab.value = 'active'
        await loadOrders()
    } catch (e: any) {
        toast.error(e?.response?.data?.message || e.message || t('seconds.order_failed'))
    } finally {
        loading.value = false
    }
}

const showResultModal = ref(false)
interface SettlementResult {
    id: number
    symbol: string
    direction: 'BUY' | 'SELL' | number | string
    amount: number
    profit: number
    isWin: boolean
    isLose: boolean
}
const resultData = ref<SettlementResult | null>(null)

const checkingResults = ref<Set<number>>(new Set())

const tick = () => {
    const now = Date.now()
    // Update countdowns for active orders
    store.currentOrders.forEach(order => {
        const remaining = Math.max(0, Math.ceil((order.endTime - now) / 1000))
        orderCountdowns.value[order.id] = remaining
    })

    // Check for settled orders (remaining <= 0)
    const pendingSettlement = store.currentOrders.filter(order => {
        const remaining = orderCountdowns.value[order.id]
        return remaining !== undefined && remaining <= 0
    })

    if (pendingSettlement.length > 0) {
        pendingSettlement.forEach(async order => {
            if (checkingResults.value.has(order.id)) return
            checkingResults.value.add(order.id)

            try {
                const result = await store.checkOrderResult(order.id, order.symbol)
                // If the backend indicates it's settled (CLOSE status or WIN/LOSE result)
                if (result && (result.status === 'CLOSE' || result.status === 1 || result.result === 'WIN' || result.result === 'LOSE' || result.result === 1 || result.result === 2)) {

                    const isWin = result.result === 'WIN' || result.result === 1
                    const isLose = result.result === 'LOSE' || result.result === 2
                    const profitAmount = result.winAmount ?? result.profit ?? result.rewardAmount ?? 0

                    resultData.value = {
                        id: order.id,
                        symbol: order.symbol,
                        direction: order.direction,
                        amount: order.betAmount || order.amount,
                        profit: profitAmount,
                        isWin,
                        isLose
                    }
                    showResultModal.value = true

                    // Remove the finished order
                    store.currentOrders = store.currentOrders.filter(o => o.id !== order.id)
                    store.loadBalance()
                    if (orderTab.value === 'history') {
                        store.loadHistoryOrders(symbol.value, 0)
                    }
                }
            } catch (e) {
                console.error(t('seconds.result_query_failed'), e)
            } finally {
                // Cooldown before retrying if it hasn't actually settled yet
                setTimeout(() => {
                    checkingResults.value.delete(order.id)
                }, 3000)
            }
        })
    }
}

const formatTime = (ts: number) => {
    if (!ts) return '--'
    return new Date(ts).toLocaleString()
}

const formatCountdown = (seconds: number) => {
    if (seconds <= 0) return t('seconds.settling')
    const m = Math.floor(seconds / 60)
    const s = seconds % 60
    return m > 0 ? `${m}:${String(s).padStart(2, '0')}` : `${s}s`
}

const getResultText = (order: SecondOrder) => {
    if (order.status === 'ENTRUST') return t('seconds.status_entrust')
    if (order.status === 'OPEN') return t('seconds.status_open')
    if (order.status === 'CANCELED') return t('seconds.status_canceled')

    if (order.result === 1 || order.result === 'WIN') return t('seconds.status_win')
    if (order.result === 2 || order.result === 'LOSE') return t('seconds.status_lose')

    if (order.status === 'CLOSE') return t('seconds.status_completed')
    return t('seconds.status_pending')
}

const getResultClass = (order: SecondOrder) => {
    if (order.status === 'ENTRUST' || order.status === 'OPEN' || order.status === 'CANCELED') return 'text-muted-foreground'

    if (order.result === 1 || order.result === 'WIN') return 'text-up'
    if (order.result === 2 || order.result === 'LOSE') return 'text-down'
    return 'text-muted-foreground'
}

const chartData = ref<any[]>([])

onMounted(async () => {
    if (!isLoggedIn.value) return

    stompService.connect('seconds')

    // Load seconds products first so the pair selector cannot show normal spot markets.
    await Promise.all([
        store.loadTickers(),
        store.loadCycles(),
    ])
    await subscribeSecondTickers()
    syncSymbolWithSecondsProducts()
    selectDefaultCycleForSymbol()

    if (isLoggedIn.value) {
        await Promise.all([
            store.loadBalance(),
            store.loadCurrentOrders(symbol.value)
        ])
        privateSub = await stompService.subscribePrivate((msg) => {
            void handlePrivateEvent(msg)
        })
    } else {
        store.currentOrders = []
        store.historyOrders = []
        store.balance = 0
    }

    intervalId = setInterval(tick, 1000)
})

onUnmounted(() => {
    if (intervalId) clearInterval(intervalId)
    clearTickerSubscriptions()
    privateSub?.unsubscribe()
})
</script>

<template>
  <div v-if="!isLoggedIn" class="h-full bg-background p-4 text-foreground md:p-8">
    <AuthRequiredState />
  </div>
  <div v-else class="flex flex-col h-full overflow-hidden bg-background text-foreground">
    <!-- Header: Ticker Info -->
    <div class="h-16 min-h-[4rem] border-b border-border flex items-center px-4 bg-card justify-between z-10 shrink-0">
      <div class="flex items-center space-x-6 overflow-x-auto no-scrollbar w-full">
          <div class="flex items-center shrink-0">
             <h1 class="text-xl font-bold font-mono tracking-tight mr-2 flex items-center gap-2">
               <PairLogo class="h-8 w-8" :symbol="symbol" :src="currentTicker?.icon" />
               {{ symbol }}
             </h1>
             <span class="text-[10px] font-bold px-1.5 py-0.5 rounded bg-primary/10 text-primary border border-primary/20">{{ t('seconds.badge') }}</span>
          </div>
          <div class="h-8 w-px bg-border mx-2 shrink-0"></div>
          <div class="flex items-center space-x-3 shrink-0">
              <span :class="['text-2xl font-bold font-mono', (currentTicker?.chg || 0) >= 0 ? 'text-up' : 'text-down']">
                 {{ formatNumber(currentPrice, 'price') }}
              </span>
              <span class="text-sm text-muted-foreground font-medium">≈ ${{ formatNumber(currentPrice, 'price') }}</span>
          </div>
             <div class="flex flex-col shrink-0">
                 <span class="text-xs text-muted-foreground">24h {{ t('market.change') }}</span>

                 <span :class="['text-sm font-bold font-mono', (currentTicker?.chg||0) >= 0 ? 'text-up' : 'text-down']">
                    {{ (currentTicker?.chg||0 ) >= 0 ? '+' : '' }}{{ numeral(currentTicker?.chg||0).format("0.00")  }}%
                 </span>
             </div>
             <div class="flex flex-col shrink-0">
                 <span class="text-xs text-muted-foreground">24h {{ t('market.high') }}</span>
                 <span class="text-sm font-bold font-mono">{{ formatNumber(currentTicker?.high || 0, 'price') }}</span>
             </div>
             <div class="flex flex-col shrink-0">
                 <span class="text-xs text-muted-foreground">24h {{ t('market.low') }}</span>
                 <span class="text-sm font-bold font-mono">{{ formatNumber(currentTicker?.low || 0, 'price') }}</span>
             </div>
         </div>
    </div>

    <!-- Main Content -->
    <div class="flex-1 flex flex-col lg:flex-row overflow-y-auto lg:overflow-hidden">
        <!-- Left: Market List Sidebar -->
        <div class="w-full lg:w-[280px] border-b lg:border-b-0 lg:border-r border-border flex flex-col bg-card shrink-0 order-3 lg:order-1 h-[300px] lg:h-full">
            <div class="p-2 border-b border-border">
                <div class="relative">
                    <Icon icon="lucide:search" class="absolute left-2.5 top-2.5 w-4 h-4 text-muted-foreground" />
                    <input
                        v-model="filterText"
                        type="text"
                        :placeholder="t('seconds.search_symbol')"
                        class="w-full bg-muted/20 border border-border rounded h-9 pl-9 pr-3 text-xs focus:outline-none focus:border-primary transition-colors"
                    />
                </div>
            </div>
            <!-- Header -->
            <div class="flex items-center px-3 py-2 text-xs text-muted-foreground border-b border-border/50">
                <div class="flex-1">{{ t('seconds.symbol') }}</div>
                <div class="w-20 text-right">{{ t('seconds.price') }}</div>
                <div class="w-16 text-right">{{ t('seconds.change') }}</div>
            </div>
            <!-- List -->
            <div class="flex-1 overflow-y-auto no-scrollbar">
                <div
                    v-for="ticker in marketList"
                    :key="ticker.symbol"
                    @click="changeSymbol(ticker.symbol)"
                    class="flex items-center px-3 py-2.5 cursor-pointer hover:bg-muted/30 transition-colors border-b border-border/20 last:border-0 group"
                    :class="{'bg-primary/5': ticker.symbol === symbol}"
                >
                    <div class="flex min-w-0 flex-1 items-center gap-2" :class="ticker.symbol === symbol ? 'text-primary' : ''">
                        <PairLogo class="h-7 w-7" :symbol="ticker.symbol" :src="ticker.icon" />
                        <div class="truncate font-mono text-sm font-bold group-hover:text-primary transition-colors">
                            {{ ticker.symbol.split('/')[0] }}<span class="text-xs font-normal text-muted-foreground">/{{ ticker.symbol.split('/')[1] }}</span>
                        </div>
                    </div>
                    <div class="w-20 text-right font-mono text-sm" :class="ticker.chg >= 0 ? 'text-up' : 'text-down'">
                        {{ formatNumber(ticker.close, 'price') }}
                    </div>
                    <div class="w-16 text-right">
                        <span class="text-[10px] px-1 py-0.5 rounded font-bold font-mono min-w-[50px] inline-block text-center"
                            :class="ticker.chg >= 0 ? 'bg-up/10 text-up' : 'bg-down/10 text-down'">
                            {{ ticker.chg >= 0 ? '+' : '' }}{{ numeral(ticker.chg).format('0.00') }}%
                        </span>
                    </div>
                </div>
                <div v-if="marketList.length === 0" class="p-8 text-center text-muted-foreground text-xs">
                    {{ t('seconds.no_symbols') }}
                </div>
            </div>
        </div>

        <!-- Center: Chart & History -->
        <div class="w-full lg:flex-1 flex flex-col min-h-[500px] lg:h-full bg-background relative order-1 lg:order-2">
             <!-- Chart -->
             <div class="flex-1 border-b border-border relative flex flex-col min-h-[320px]">
                 <!-- Chart Toolbar -->
                 <div class="h-10 border-b border-border bg-card flex items-center px-4 gap-4 overflow-x-auto no-scrollbar shrink-0">
                    <span :class="settingStore.chartProvider === 'klinecharts' ? 'text-sm font-bold text-primary border-b-2 border-primary h-full flex items-center px-2 whitespace-nowrap' : 'text-sm font-medium text-muted-foreground h-full flex items-center px-2 whitespace-nowrap'">{{ t('trade.original') }}</span>
                    <span :class="settingStore.chartProvider === 'tradingview' ? 'text-sm font-bold text-primary border-b-2 border-primary h-full flex items-center px-2 whitespace-nowrap' : 'text-sm font-medium text-muted-foreground h-full flex items-center px-2 whitespace-nowrap'">{{ t('trade.tradingview') }}</span>
                 </div>
                <MarketChart v-if="symbol" :dataList="chartData" :symbol="symbol" :precision="precision" module="seconds" period="1m" class="flex-1" :key="`${symbol}-${precision}`" />
             </div>

             <!-- Active Orders & History -->
             <div class="h-[240px] bg-card border-t-4 border-background shrink-0 flex flex-col z-20 relative overflow-hidden">
                <div class="flex border-b border-border">
                    <button @click="orderTab = 'active'; loadOrders()"
                      :class="['px-4 py-2 text-sm font-bold border-b-2 transition-colors', orderTab === 'active' ? 'border-primary text-primary' : 'border-transparent text-muted-foreground hover:text-foreground']">
                      {{ t('seconds.active_positions') }}
                    </button>
                    <button @click="orderTab = 'history'; loadOrders()"
                      :class="['px-4 py-2 text-sm font-bold border-b-2 transition-colors', orderTab === 'history' ? 'border-primary text-primary' : 'border-transparent text-muted-foreground hover:text-foreground']">
                      {{ t('seconds.history_records') }}
                    </button>
                </div>

                <AuthRequiredState v-if="!isLoggedIn" compact class="m-4" />

                <!-- Active Orders -->
                <div v-else-if="orderTab === 'active'" class="flex-1 overflow-auto p-0">
                     <table class="w-full text-xs text-left">
                        <thead class="bg-muted/20 text-muted-foreground sticky top-0">
                            <tr>
                                <th class="px-4 py-2">{{ t('seconds.direction') }}</th>
                                <th class="px-4 py-2 text-right">{{ t('seconds.amount') }}</th>
                                <th class="px-4 py-2 text-right">{{ t('seconds.open_price') }}</th>
                                <th class="px-4 py-2 text-right">{{ t('seconds.current_price') }}</th>
                                <th class="px-4 py-2 text-right">{{ t('seconds.yield_rate') }}</th>
                                <th class="px-4 py-2 text-center">{{ t('seconds.countdown') }}</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-border/50">
                            <tr v-for="order in store.currentOrders" :key="order.id" class="hover:bg-muted/10">
                                <td class="px-4 py-2">
                                    <span :class="order.direction === 'BUY' ? 'text-up' : 'text-down'" class="font-bold">
                                        {{ order.direction === 'BUY' ? t('seconds.call') : t('seconds.put') }}
                                    </span>
                                </td>
<!--                                {{JSON.stringify(order)}}-->
                                <td class="px-4 py-2 text-right font-mono">{{ order.betAmount }} USDT</td>
                                <td class="px-4 py-2 text-right font-mono">{{ formatNumber(order.openPrice, 'price') }}</td>
                                <td class="px-4 py-2 text-right font-mono" :class="currentPrice > order.openPrice ? 'text-up' : 'text-down'">
                                    {{ formatNumber(currentPrice, 'price') }}
                                </td>
                                <td class="px-4 py-2 text-right font-mono">{{ numeral(order.cycleRate * 100).format('0') }}%</td>
                                <td class="px-4 py-2 align-middle">
                                    <div class="relative w-32 mx-auto h-6 bg-muted/20 rounded overflow-hidden flex items-center justify-center border border-border/50">
                                        <div class="absolute left-0 top-0 h-full bg-primary/20 transition-all duration-1000 ease-linear origin-left"
                                             :style="{ width: `${Math.max(0, Math.min(100, ((orderCountdowns[order.id] ?? order.cycleLength) / order.cycleLength) * 100))}%` }">
                                        </div>
                                        <span class="relative z-10 text-xs font-bold font-mono text-primary">
                                            {{ formatCountdown(orderCountdowns[order.id] ?? order.cycleLength) }}
                                        </span>
                                    </div>
                                </td>
                            </tr>
                             <tr v-if="store.currentOrders.length === 0">
                                <td colspan="6" class="text-center py-12 text-muted-foreground">{{ t('seconds.no_positions') }}</td>
                            </tr>
                        </tbody>
                    </table>
                </div>

                <!-- History Orders -->
                <div v-else class="flex-1 overflow-auto p-0" @scroll="handleHistoryScroll">
                     <table class="w-full text-xs text-left">
                        <thead class="bg-muted/20 text-muted-foreground sticky top-0 z-10">
                            <tr>
                                <th class="px-4 py-2">{{ t('trade.time') }}</th>
                                <th class="px-4 py-2">{{ t('seconds.direction') }}</th>
                                <th class="px-4 py-2 text-right">{{ t('seconds.amount') }}</th>
                                <th class="px-4 py-2 text-right">{{ t('seconds.open_price') }}</th>
                                <th class="px-4 py-2 text-right">{{ t('seconds.close_price') }}</th>
                                <th class="px-4 py-2 text-right">{{ t('seconds.pnl') }}</th>
                                <th class="px-4 py-2 text-center">{{ t('seconds.result') }}</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-border/50">
                            <tr v-for="order in store.historyOrders" :key="order.id" class="hover:bg-muted/10">
                                <td class="px-4 py-2 text-muted-foreground">{{ formatTime(order.createTime) }}</td>
                                <td class="px-4 py-2">
                                    <span :class="order.direction === 'BUY' ? 'text-up' : 'text-down'" class="font-bold">
                                        {{ order.direction === 'BUY' ? t('seconds.call') : t('seconds.put') }}
                                    </span>
                                </td>
                                <td class="px-4 py-2 text-right font-mono">{{ order.betAmount }}</td>
                                <td class="px-4 py-2 text-right font-mono">{{ formatNumber(order.openPrice, 'price') }}</td>
                                <td class="px-4 py-2 text-right font-mono">{{ formatNumber(order.closePrice, 'price') }}</td>
                                <td class="px-4 py-2 text-right font-mono font-bold" :class="getResultClass(order)">
                                    {{ order.profit >= 0 ? '+' : '' }}{{ numeral(order.profit).format('0,0.00') }}
                                </td>
                                <td class="px-4 py-2 text-center">
                                    <span class="text-[10px] font-bold px-1.5 py-0.5 rounded" :class="getResultClass(order)">
                                        {{ getResultText(order) }}
                                    </span>
                                </td>
                            </tr>
                             <tr v-if="store.historyOrders.length === 0 && !store.loadingHistory">
                                <td colspan="7" class="text-center py-12 text-muted-foreground">{{ t('seconds.no_records') }}</td>
                            </tr>
                        </tbody>
                    </table>

                    <!-- Pagination Loading State -->
                    <div v-if="store.historyOrders.length > 0" class="py-4 text-center pb-8">
                        <div v-if="store.loadingHistory" class="text-xs flex items-center justify-center gap-2 text-muted-foreground">
                            <Icon icon="mdi:loading" class="animate-spin text-primary" /> {{ t('seconds.loading') }}
                        </div>
                        <div v-else-if="!store.historyHasMore" class="text-[10px] text-muted-foreground">
                            {{ t('seconds.no_more') }}
                        </div>
                    </div>
                </div>
             </div>
        </div>

        <!-- Right: Trade Form -->
        <div class="w-full lg:w-[320px] border-t lg:border-t-0 lg:border-l border-border flex flex-col bg-card shrink-0 order-2 lg:order-3">
             <div class="p-4 flex flex-col gap-4 flex-1">
                 <div class="flex justify-between items-center text-sm font-bold mb-2">
                     <span>{{ t('seconds.trading') }}</span>
                     <span class="text-xs font-normal text-muted-foreground">{{ usdtBalanceText }}</span>
                 </div>

                 <!-- Cycle Selection -->
                 <div>
                   <label class="text-xs text-muted-foreground block mb-2">{{ t('seconds.select_cycle') }}</label>
                   <div class="grid grid-cols-3 gap-2">
                     <button
                       v-for="cycle in visibleCycles"
                       :key="cycle.id"
                       @click="selectedCycleId = cycle.id"
                       :class="selectedCycleId === cycle.id
                         ? 'bg-primary text-primary-foreground font-bold shadow-lg shadow-primary/20 border-primary'
                         : 'bg-muted hover:bg-muted/80 border-border'"
                       class="py-2.5 text-xs rounded transition-all border flex flex-col items-center gap-0.5"
                     >
                       <span class="font-mono font-bold">{{ cycle.cycleLength }}s</span>
                       <span class="text-[10px] opacity-70">{{ numeral(cycle.cycleRate * 100).format('0') }}%</span>
                     </button>
                   </div>
                 </div>

                 <!-- Amount -->
                 <div>
                   <label class="text-xs text-muted-foreground block mb-2">{{ t('seconds.stake_amount') }}</label>
                   <div class="flex items-center bg-background border border-input rounded px-3 h-10 focus-within:border-primary transition-colors">
                     <input v-model="amount" type="number" class="bg-transparent w-full outline-none font-bold font-mono" placeholder="0.00" />
                     <span class="text-xs text-muted-foreground ml-2">USDT</span>
                   </div>
                   <div class="grid grid-cols-4 gap-2 mt-2">
                     <button @click="amount = 10" class="py-1 text-[10px] bg-muted hover:bg-muted/80 rounded transition-colors border border-transparent hover:border-border">10</button>
                     <button @click="amount = 50" class="py-1 text-[10px] bg-muted hover:bg-muted/80 rounded transition-colors border border-transparent hover:border-border">50</button>
                     <button @click="amount = 100" class="py-1 text-[10px] bg-muted hover:bg-muted/80 rounded transition-colors border border-transparent hover:border-border">100</button>
                     <button @click="amount = usdtBalance" class="py-1 text-[10px] bg-muted hover:bg-muted/80 rounded transition-colors border border-transparent hover:border-border">{{ t('seconds.all') }}</button>
                   </div>
                   <!-- Min/Max hint -->
                   <div v-if="selectedCycle" class="text-[10px] text-muted-foreground mt-1">
                     {{ t('seconds.range') }}: {{ selectedCycle.minAmount }} - {{ selectedCycle.maxAmount }} USDT
                   </div>
                 </div>

                 <AuthRequiredState v-if="!isLoggedIn" compact />

                 <div class="mt-auto pt-6 flex flex-col gap-4">
                    <!-- Call Button -->
                    <button
                        @click="handleOrder(0)"
                        :disabled="loading || !selectedCycleId || !isLoggedIn"
                        class="w-full relative overflow-hidden rounded-xl border border-[#0ecb81]/30 bg-gradient-to-r from-[#0ecb81]/10 to-transparent p-[1px] transition-all hover:shadow-[0_4px_20px_rgba(14,203,129,0.2)] hover:-translate-y-0.5 active:scale-[0.98] disabled:opacity-50 disabled:cursor-not-allowed group"
                    >
                        <div class="bg-card w-full h-full rounded-xl py-4 px-6 flex items-center justify-between group-hover:bg-[#0ecb81]/5 transition-colors">
                             <div class="flex items-center gap-3">
                                 <div class="w-10 h-10 rounded-full bg-[#0ecb81]/20 flex items-center justify-center text-[#0ecb81] group-hover:bg-[#0ecb81] group-hover:text-white transition-colors duration-300">
                                     <Icon icon="mdi:trending-up" class="w-6 h-6" />
                                 </div>
                                 <div class="flex flex-col items-start leading-none">
                                     <span class="text-foreground font-bold text-lg">{{ t('seconds.buy_call') }}</span>
                                     <span class="text-xs text-muted-foreground mt-1 tracking-wide uppercase">{{ t('seconds.call') }}</span>
                                 </div>
                             </div>
                             <div class="flex flex-col items-end leading-none">
                                 <span class="text-[10px] text-muted-foreground mb-1">{{ t('seconds.expected_return') }}</span>
                                 <span class="text-lg font-black font-mono text-[#0ecb81]">{{ payoutPercent }}%</span>
                             </div>
                        </div>
                    </button>

                    <!-- Put Button -->
                    <button
                        @click="handleOrder(1)"
                        :disabled="loading || !selectedCycleId || !isLoggedIn"
                        class="w-full relative overflow-hidden rounded-xl border border-[#f6465d]/30 bg-gradient-to-r from-[#f6465d]/10 to-transparent p-[1px] transition-all hover:shadow-[0_4px_20px_rgba(246,70,93,0.2)] hover:-translate-y-0.5 active:scale-[0.98] disabled:opacity-50 disabled:cursor-not-allowed group"
                    >
                        <div class="bg-card w-full h-full rounded-xl py-4 px-6 flex items-center justify-between group-hover:bg-[#f6465d]/5 transition-colors">
                             <div class="flex items-center gap-3">
                                 <div class="w-10 h-10 rounded-full bg-[#f6465d]/20 flex items-center justify-center text-[#f6465d] group-hover:bg-[#f6465d] group-hover:text-white transition-colors duration-300">
                                     <Icon icon="mdi:trending-down" class="w-6 h-6" />
                                 </div>
                                 <div class="flex flex-col items-start leading-none">
                                     <span class="text-foreground font-bold text-lg">{{ t('seconds.sell_put') }}</span>
                                     <span class="text-xs text-muted-foreground mt-1 tracking-wide uppercase">{{ t('seconds.put') }}</span>
                                 </div>
                             </div>
                             <div class="flex flex-col items-end leading-none">
                                 <span class="text-[10px] text-muted-foreground mb-1">{{ t('seconds.expected_return') }}</span>
                                 <span class="text-lg font-black font-mono text-[#f6465d]">{{ payoutPercent }}%</span>
                             </div>
                        </div>
                    </button>
                 </div>
             </div>
        </div>
    </div>

    <!-- Settlement Result Modal -->
    <div v-if="showResultModal && resultData" class="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50 transition-opacity" @click.self="showResultModal = false">
      <div
        class="bg-card w-80 rounded-2xl p-6 shadow-2xl border relative overflow-hidden transform transition-all duration-300 scale-100"
        :class="resultData.isWin ? 'border-[#0ecb81]/50 shadow-[#0ecb81]/20' : resultData.isLose ? 'border-[#f6465d]/50 shadow-[#f6465d]/20' : 'border-border'"
      >
        <!-- Modal Background Decoration -->
        <div class="absolute -top-24 -right-24 w-48 h-48 rounded-full blur-3xl opacity-20 pointer-events-none"
             :class="resultData.isWin ? 'bg-[#0ecb81]' : resultData.isLose ? 'bg-[#f6465d]' : 'bg-primary'">
        </div>

        <div class="flex items-center justify-between mb-6 relative z-10">
          <div class="text-lg font-bold">{{ t('seconds.settlement_record') }}</div>
          <button @click="showResultModal = false" class="text-muted-foreground hover:text-foreground p-1 rounded-full hover:bg-muted transition-colors">
              <Icon icon="lucide:x" class="w-5 h-5" />
          </button>
        </div>

        <div class="flex flex-col items-center justify-center mb-8 relative z-10">
            <div
                class="w-16 h-16 rounded-full flex items-center justify-center mb-3 shadow-lg"
                :class="resultData.isWin ? 'bg-[#0ecb81]/20 text-[#0ecb81] shadow-[#0ecb81]/30' : resultData.isLose ? 'bg-[#f6465d]/20 text-[#f6465d] shadow-[#f6465d]/30' : 'bg-muted text-muted-foreground'"
            >
                <Icon :icon="resultData.isWin ? 'mdi:emoticon-happy' : resultData.isLose ? 'mdi:emoticon-sad' : 'mdi:check-circle'" class="w-10 h-10" />
            </div>
            <div class="text-3xl font-black font-mono tracking-tight" :class="resultData.isWin ? 'text-[#0ecb81]' : resultData.isLose ? 'text-[#f6465d]' : ''">
                {{ resultData.isWin ? '+' : resultData.isLose ? '-' : '' }}{{ numeral(resultData.profit).format('0,0.00') }}
            </div>
            <div class="text-xs text-muted-foreground mt-1 uppercase font-medium">USDT</div>
        </div>

        <div class="space-y-3 relative z-10 bg-muted/30 p-4 rounded-xl border border-border/50">
            <div class="flex justify-between text-sm">
                <span class="text-muted-foreground">{{ t('seconds.pair') }}</span>
                <span class="font-bold">{{ resultData.symbol }}</span>
            </div>
            <div class="flex justify-between text-sm">
                <span class="text-muted-foreground">{{ t('seconds.direction') }}</span>
                <span class="font-bold" :class="resultData.direction === 'BUY' || resultData.direction === 0 ? 'text-[#0ecb81]' : 'text-[#f6465d]'">
                    {{ resultData.direction === 'BUY' || resultData.direction === 0 ? t('seconds.call') : t('seconds.put') }}
                </span>
            </div>
            <div class="flex justify-between text-sm">
                <span class="text-muted-foreground">{{ t('seconds.invested_amount') }}</span>
                <span class="font-mono">{{ resultData.amount }} USDT</span>
            </div>
            <div class="flex justify-between text-sm">
                <span class="text-muted-foreground">{{ t('seconds.final_result') }}</span>
                <span class="font-bold" :class="resultData.isWin ? 'text-[#0ecb81]' : resultData.isLose ? 'text-[#f6465d]' : ''">
                    {{ resultData.isWin ? t('seconds.status_win') : resultData.isLose ? t('seconds.status_lose') : t('seconds.status_draw') }}
                </span>
            </div>
        </div>

        <button @click="showResultModal = false" class="w-full mt-6 py-3 bg-primary text-primary-foreground font-bold rounded-xl hover:bg-primary/90 transition-colors shadow-lg shadow-primary/20 relative z-10">
            {{ t('seconds.done') }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.no-scrollbar::-webkit-scrollbar {
    display: none;
}
.no-scrollbar {
    -ms-overflow-style: none;
    scrollbar-width: none;
}
</style>
