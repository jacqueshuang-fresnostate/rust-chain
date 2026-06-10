<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { formatNumber } from '@/utils/format'
import { useMarketStore } from '@/stores/market'
import { fetchWallet } from '@/api/exchange'
import { submitSwap } from '@/api/swap'
import { stompService } from '@/api/stomp'
import { useToast } from 'vue-toastification'

const { t } = useI18n()
const toast = useToast()
const marketStore = useMarketStore()

const fromToken = ref('ETH')
const toToken = ref('USDT')
const fromAmount = ref<string>('')
const loading = ref(false)

const wallet = ref<Record<string, number>>({})

const fromBalance = computed(() => wallet.value[fromToken.value] || 0)
const toBalance = computed(() => wallet.value[toToken.value] || 0)

// Find ticker for exchange rate
// Assuming pair format is TOKEN/USDT
const pairSymbol = computed(() => {
    if (toToken.value === 'USDT') return `${fromToken.value}/USDT`
    if (fromToken.value === 'USDT') return `${toToken.value}/USDT`
    return null // Complex cross pairs not supported yet
})

const currentPrice = computed(() => {
    if (!pairSymbol.value) return 0
    const ticker = marketStore.tickers.find(t => t.symbol === pairSymbol.value)
    return ticker?.close || 0
})

const exchangeRate = computed(() => {
    if (toToken.value === 'USDT') return currentPrice.value
    if (fromToken.value === 'USDT' && currentPrice.value) return 1 / currentPrice.value
    return 0
})

const toAmount = computed(() => {
    const amount = parseFloat(fromAmount.value)
    if (!amount || !exchangeRate.value) return 0
    return amount * exchangeRate.value
})

const refreshWallet = async () => {
    // Fetch wallet for both tokens
    // We can use fetchWallet('ETH/USDT') to get both if they are in a pair
    // Or fetch all. For now, let's assume fetchWallet works per pair or use fetchAssets if available
    // Let's try fetching for the active pair
    if (pairSymbol.value) {
        try {
            // eslint-disable-next-line @typescript-eslint/no-explicit-any
            const res: any = await fetchWallet(pairSymbol.value)
            if (res.data && Array.isArray(res.data)) {
                // eslint-disable-next-line @typescript-eslint/no-explicit-any
                res.data.forEach((item: any) => {
                    wallet.value[item.symbol] = item.balance
                })
            }
        } catch (e) {
            console.error(e)
        }
    }
}

const handleSwap = async () => {
    if (!fromAmount.value || parseFloat(fromAmount.value) <= 0) {
        toast.error('Please enter a valid amount')
        return
    }
    if (parseFloat(fromAmount.value) > fromBalance.value) {
        toast.error('Insufficient balance')
        return
    }

    loading.value = true
    try {
        await submitSwap(fromToken.value, toToken.value, parseFloat(fromAmount.value))
        toast.success('Swap Successful')
        fromAmount.value = ''
        refreshWallet()
    } catch (e: any) {
        toast.error(e.message || 'Swap Failed')
    } finally {
        loading.value = false
    }
}

const switchTokens = () => {
    const temp = fromToken.value
    fromToken.value = toToken.value
    toToken.value = temp
    fromAmount.value = '' // Reset amount on switch to avoid confusion
    refreshWallet()
}

onMounted(() => {
    stompService.connect()
    refreshWallet()
})

onUnmounted(() => {
    // stompService.disconnect()
})
</script>

<template>
  <div class="max-w-md mx-auto mt-20 p-6 bg-card border border-border rounded-lg shadow-neon relative overflow-hidden">
    <div class="absolute top-0 left-0 w-full h-1 bg-gradient-to-r from-neon-blue to-neon-pink"></div>

    <h2 class="text-2xl font-bold mb-6 text-center tracking-tight">{{ t('nav.swap') }}</h2>

    <!-- From Input -->
    <div class="bg-muted/50 p-4 rounded-lg mb-2 border border-transparent hover:border-primary/50 transition-colors">
      <div class="flex justify-between mb-2">
        <span class="text-xs text-muted-foreground">From</span>
        <span class="text-xs text-muted-foreground">Balance: {{ formatNumber(fromBalance, 'amount') }} {{ fromToken }}</span>
      </div>
      <div class="flex justify-between items-center">
        <input
            v-model="fromAmount"
            type="number"
            class="bg-transparent text-2xl outline-none w-full font-mono"
            placeholder="0.0"
        />
        <div class="flex items-center gap-2 bg-background px-2 py-1 rounded-full border border-border cursor-pointer hover:border-primary">
          <div class="w-5 h-5 rounded-full" :class="fromToken === 'USDT' ? 'bg-green-500' : 'bg-blue-500'"></div>
          <span class="font-bold">{{ fromToken }}</span>
          <!-- <span>▼</span> -->
        </div>
      </div>
    </div>

    <!-- Swap Arrow -->
    <div class="flex justify-center -my-3 relative z-10">
      <div @click="switchTokens" class="bg-card border border-border p-2 rounded-full cursor-pointer hover:text-primary hover:border-primary transition-all box-shadow-sm">
        ↓
      </div>
    </div>

    <!-- To Input -->
    <div class="bg-muted/50 p-4 rounded-lg mt-2 border border-transparent hover:border-primary/50 transition-colors">
      <div class="flex justify-between mb-2">
        <span class="text-xs text-muted-foreground">To</span>
        <span class="text-xs text-muted-foreground">Balance: {{ formatNumber(toBalance, 'amount') }} {{ toToken }}</span>
      </div>
      <div class="flex justify-between items-center">
        <input
            :value="formatNumber(toAmount, 'amount')"
            readonly
            type="text"
            class="bg-transparent text-2xl outline-none w-full font-mono text-muted-foreground"
            placeholder="0.0"
        />
        <div class="flex items-center gap-2 bg-background px-2 py-1 rounded-full border border-border cursor-pointer hover:border-primary">
          <div class="w-5 h-5 rounded-full" :class="toToken === 'USDT' ? 'bg-green-500' : 'bg-blue-500'"></div>
          <span class="font-bold">{{ toToken }}</span>
          <!-- <span>▼</span> -->
        </div>
      </div>
    </div>

    <!-- Info -->
    <div class="mt-6 space-y-2 text-sm">
      <div class="flex justify-between text-muted-foreground">
        <span>Rate</span>
        <span>1 {{ fromToken }} ≈ {{ formatNumber(exchangeRate, 'price') }} {{ toToken }}</span>
      </div>
      <div class="flex justify-between text-muted-foreground">
        <span>Fee</span>
        <span>${{ formatNumber(0, 'price') }}</span>
      </div>
    </div>

    <!-- Action -->
    <button
        @click="handleSwap"
        :disabled="loading"
        class="w-full mt-6 py-4 bg-primary text-primary-foreground font-bold text-lg rounded-lg hover:bg-primary/90 transition-all box-glow disabled:opacity-50 disabled:cursor-not-allowed">
      <span v-if="loading">Swapping...</span>
      <span v-else>Swap</span>
    </button>
  </div>
</template>
