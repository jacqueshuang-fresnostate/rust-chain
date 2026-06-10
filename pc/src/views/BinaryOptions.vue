<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed, watch } from 'vue'
import { formatNumber } from '@/utils/format'
import { useMarketStore } from '@/stores/market'
import { fetchWallet } from '@/api/exchange'
import { submitOptionOrder } from '@/api/option'
import { stompService } from '@/api/stomp'
import { useToast } from 'vue-toastification'

const toast = useToast()
const marketStore = useMarketStore()

const symbol = ref('BTC/USDT')
const amount = ref<number>(100)
const period = ref<number>(30) // seconds
const loading = ref(false)
const wallet = ref<Record<string, number>>({})

// Computed
const currentTicker = computed(() => marketStore.tickers.find(t => t.symbol === symbol.value))
const currentPrice = computed(() => currentTicker.value?.close || 0)
const usdtBalance = computed(() => wallet.value['USDT'] || 0)

const countdown = ref('00:00')
let timer: any

// Mock countdown for UI effect (in real app, this binds to active order)
const startCountdown = (seconds: number) => {
    let sec = seconds
    if (timer) clearInterval(timer)
    timer = setInterval(() => {
        sec--
        if (sec < 0) {
            clearInterval(timer)
            countdown.value = '00:00'
            return
        }
        countdown.value = `00:${sec.toString().padStart(2, '0')}`
    }, 1000)
}

const refreshWallet = async () => {
    try {
        // Fetch wallet for USDT (Quote currency for Binary usually)
        const res = await fetchWallet(symbol.value) // or specific wallet endpoint
        if (res.data && Array.isArray(res.data)) {
            res.data.forEach((item: any) => {
                wallet.value[item.symbol] = item.balance
            })
        }
    } catch (e) {
        console.error(e)
    }
}

const handleOrder = async (direction: 'BUY' | 'SELL') => {
    if (!amount.value || amount.value <= 0) {
        toast.error('Invalid amount')
        return
    }
    if (amount.value > usdtBalance.value) {
        toast.error('Insufficient balance')
        return
    }

    loading.value = true
    try {
        await submitOptionOrder(symbol.value, direction, amount.value, period.value)
        toast.success(`${direction === 'BUY' ? 'Call' : 'Put'} Order Placed`)
        refreshWallet()
        startCountdown(period.value)
    } catch (e: any) {
        toast.error(e.message || 'Order Failed')
    } finally {
        loading.value = false
    }
}

watch(period, () => {
    countdown.value = `00:${period.value.toString().padStart(2, '0')}`
}, { immediate: true })

onMounted(() => {
    stompService.connect()
    refreshWallet()
})

onUnmounted(() => {
    // stompService.disconnect()
  if (timer) clearInterval(timer)
})
</script>

<template>
  <div class="h-full flex flex-col md:flex-row gap-4 p-4">
    <!-- Chart Area -->
    <div class="flex-1 bg-card border border-border rounded-lg overflow-hidden flex flex-col relative">
       <div class="p-4 border-b border-border flex justify-between items-center">
         <div class="flex gap-2 items-center">
           <span class="font-bold text-xl">{{ symbol }}</span>
           <span class="px-2 py-0.5 bg-muted text-xs rounded">Binary</span>
         </div>
         <div class="font-mono text-xl animate-pulse" :class="(currentTicker?.chg || 0) >= 0 ? 'text-up' : 'text-down'">
           {{ formatNumber(currentPrice, 'price') }}
         </div>
       </div>
       <div class="flex-1 relative bg-background/50">
          <!-- Placeholder for simplified chart -->
          <div class="absolute inset-0 flex items-center justify-center text-muted-foreground">
             [Line Chart Area - Realtime Price: {{ formatNumber(currentPrice, 'price') }}]
          </div>
          <!-- Countdown Overlay -->
          <div class="absolute top-10 left-1/2 -translate-x-1/2 flex flex-col items-center">
            <div class="text-xs text-muted-foreground uppercase tracking-widest mb-1">Expires In</div>
            <div class="text-4xl font-black font-mono text-primary text-glow">{{ countdown }}</div>
          </div>
       </div>
    </div>

    <!-- Controls -->
    <div class="w-full md:w-80 flex flex-col gap-4">
      <div class="bg-card border border-border rounded-lg p-4 flex-1 flex flex-col">
        <div class="flex justify-between text-xs text-muted-foreground mb-4">
            <span>Balance</span>
            <span class="font-mono text-foreground">{{ formatNumber(usdtBalance, 'amount') }} USDT</span>
        </div>

        <div class="mb-6">
           <label class="text-xs text-muted-foreground block mb-2">Amount</label>
           <div class="flex items-center bg-muted/50 border border-input rounded px-3 h-12">
             <span class="text-muted-foreground mr-2">$</span>
             <input v-model="amount" type="number" class="bg-transparent w-full outline-none text-lg font-bold" />
           </div>
           <div class="flex gap-2 mt-2">
             <button @click="amount = 10" class="flex-1 py-1 text-xs bg-muted hover:bg-muted/80 rounded transition-colors">$10</button>
             <button @click="amount = 50" class="flex-1 py-1 text-xs bg-muted hover:bg-muted/80 rounded transition-colors">$50</button>
             <button @click="amount = 100" class="flex-1 py-1 text-xs bg-muted hover:bg-muted/80 rounded transition-colors">$100</button>
           </div>
        </div>

        <div class="mb-6">
           <label class="text-xs text-muted-foreground block mb-2">Duration</label>
           <div class="grid grid-cols-3 gap-2">
             <button @click="period = 30" :class="period === 30 ? 'border-primary bg-primary/20 text-primary box-glow' : 'border-border hover:border-primary bg-transparent'" class="py-2 text-sm border rounded font-bold transition-all">30s</button>
             <button @click="period = 60" :class="period === 60 ? 'border-primary bg-primary/20 text-primary box-glow' : 'border-border hover:border-primary bg-transparent'" class="py-2 text-sm border rounded font-bold transition-all">1m</button>
             <button @click="period = 300" :class="period === 300 ? 'border-primary bg-primary/20 text-primary box-glow' : 'border-border hover:border-primary bg-transparent'" class="py-2 text-sm border rounded font-bold transition-all">5m</button>
           </div>
        </div>

        <div class="mt-auto space-y-3">
           <div class="flex justify-between text-sm">
             <span class="text-muted-foreground">Profit</span>
             <span class="text-up font-bold">{{ formatNumber(0.85, 'percent') }}</span>
           </div>
           <button @click="handleOrder('BUY')" :disabled="loading" class="w-full py-4 bg-up text-black font-black text-xl rounded hover:opacity-90 transition-all shadow-[0_0_20px_rgba(0,255,159,0.3)] hover:shadow-[0_0_30px_rgba(0,255,159,0.5)] flex items-center justify-center gap-2 disabled:opacity-50">
             <span>CALL</span>
             <span class="text-sm">▲</span>
           </button>
           <button @click="handleOrder('SELL')" :disabled="loading" class="w-full py-4 bg-down text-white font-black text-xl rounded hover:opacity-90 transition-all shadow-[0_0_20px_rgba(255,0,85,0.3)] hover:shadow-[0_0_30px_rgba(255,0,85,0.5)] flex items-center justify-center gap-2 disabled:opacity-50">
             <span>PUT</span>
             <span class="text-sm">▼</span>
           </button>
        </div>
      </div>
    </div>
  </div>
</template>
