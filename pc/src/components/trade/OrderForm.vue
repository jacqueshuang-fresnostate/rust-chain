<template>
  <div class="flex flex-col h-full bg-card">
    <!-- Tabs -->
    <div class="flex border-b border-border">
      <button
        @click="direction = 'BUY'"
        class="flex-1 py-3 text-sm font-bold border-b-2 transition-colors"
        :class="direction === 'BUY' ? 'border-up text-up bg-up/5' : 'border-transparent text-muted-foreground hover:text-foreground'"
      >
        {{ $t('trade.buy') }}
      </button>
      <button
        v-if="!buyOnly"
        @click="direction = 'SELL'"
        class="flex-1 py-3 text-sm font-bold border-b-2 transition-colors"
        :class="direction === 'SELL' ? 'border-down text-down bg-down/5' : 'border-transparent text-muted-foreground hover:text-foreground'"
      >
        {{ $t('trade.sell') }}
      </button>
    </div>

    <!-- Form Content -->
    <div class="p-4 flex flex-col gap-4 flex-1">
      <!-- Order Type -->
      <div class="flex gap-2 mb-2">
         <span @click="orderType = 'LIMIT_PRICE'" :class="orderType === 'LIMIT_PRICE' ? 'text-foreground font-bold underline' : 'text-muted-foreground font-medium hover:text-foreground'" class="text-xs cursor-pointer">{{ $t('trade.limit') }}</span>
         <span @click="orderType = 'MARKET_PRICE'" :class="orderType === 'MARKET_PRICE' ? 'text-foreground font-bold underline' : 'text-muted-foreground font-medium hover:text-foreground'" class="text-xs cursor-pointer">{{ $t('trade.market') }}</span>
      </div>

      <!-- Balance -->
      <div class="flex justify-between text-xs items-center">
         <span class="text-muted-foreground flex items-center gap-1">
            <Icon icon="mdi:wallet-outline" class="w-3 h-3" /> {{ $t('trade.avbl') }}
         </span>
         <span class="font-mono font-bold text-foreground">
            {{ direction === 'BUY' ? formatNumber(wallet.quote, 'price') + ' ' + quoteSymbol : formatNumber(wallet.base, 'amount') + ' ' + baseSymbol }}
         </span>
      </div>

      <!-- Price Input -->
      <div class="space-y-1" v-if="orderType !== 'MARKET_PRICE'">
        <div class="flex items-center bg-background border border-input rounded px-3 h-10 focus-within:border-primary transition-colors hover:border-border/80">
          <span class="text-xs text-muted-foreground w-12 shrink-0">{{ $t('trade.price') }}</span>
          <input
            v-model="price"
            type="number"
            class="bg-transparent flex-1 outline-none text-right font-mono text-sm"
            placeholder="0.00"
          />
          <span class="text-xs text-muted-foreground ml-2 w-8 text-right">{{ quoteSymbol }}</span>
        </div>
      </div>
      <div class="h-10 flex items-center px-3 bg-muted/20 border border-transparent rounded text-sm text-muted-foreground" v-else>
          Market Price
      </div>

      <!-- Amount Input -->
      <div class="space-y-1">
        <div class="flex items-center bg-background border border-input rounded px-3 h-10 focus-within:border-primary transition-colors hover:border-border/80">
          <span class="text-xs text-muted-foreground w-12 shrink-0">{{ orderType === 'MARKET_PRICE' && direction === 'BUY' ? $t('trade.total') : $t('trade.amount') }}</span>
          <input
            v-model="amount"
            type="number"
            class="bg-transparent flex-1 outline-none text-right font-mono text-sm"
            placeholder="0.00"
          />
          <span class="text-xs text-muted-foreground ml-2 w-8 text-right">{{ orderType === 'MARKET_PRICE' && direction === 'BUY' ? quoteSymbol : baseSymbol }}</span>
        </div>
      </div>

      <!-- Percent Slider/Buttons -->
      <div class="flex gap-2">
        <button v-for="p in [25, 50, 75, 100]" :key="p"
                @click="setPercent(p)"
                class="flex-1 bg-muted/50 hover:bg-muted text-[10px] py-1 rounded border border-transparent hover:border-border transition-all">
          {{ p }}%
        </button>
      </div>

      <!-- Total -->
      <div class="space-y-1 mt-2" v-if="orderType !== 'MARKET_PRICE'">
        <div class="flex items-center bg-background border border-input rounded px-3 h-10 focus-within:border-primary">
          <span class="text-xs text-muted-foreground w-12 shrink-0">{{ $t('trade.total') }}</span>
          <input
            :value="total"
            readonly
            type="number"
            class="bg-transparent flex-1 outline-none text-right font-mono text-sm text-foreground/70"
            placeholder="0.00"
          />
          <span class="text-xs text-muted-foreground ml-2 w-8 text-right">{{ quoteSymbol }}</span>
        </div>
      </div>

      <!-- Submit Button -->
      <button
        @click="submitOrder"
        :disabled="loading"
        class="w-full py-3 rounded-lg font-bold text-white mt-4 shadow-lg transition-all transform active:scale-[0.98] disabled:opacity-50 disabled:cursor-not-allowed"
        :class="direction === 'BUY' ? 'bg-up hover:bg-up/90 shadow-up/20' : 'bg-down hover:bg-down/90 shadow-down/20'"
      >
        <span v-if="loading">...</span>
        <span v-else>{{ direction === 'BUY' ? $t('trade.buy') + ' ' + baseSymbol : $t('trade.sell') + ' ' + baseSymbol }}</span>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue'
import { formatNumber } from '@/utils/format'
import { Icon } from '@iconify/vue'
import { addOrder, fetchWallet, type OrderType, type OrderDirection } from '@/api/exchange'
import { useToast } from 'vue-toastification'
import { useMarketStore } from '@/stores/market'

const props = defineProps<{
  symbol?: string
  currentPrice?: number
  buyOnly?: boolean
}>()

const toast = useToast()
const marketStore = useMarketStore()

const direction = ref<OrderDirection>('BUY')
const orderType = ref<OrderType>('LIMIT_PRICE')
const price = ref<number | null>(null)
const amount = ref<number | null>(null)
const loading = ref(false)

const wallet = ref({ base: 0, quote: 0 })

const baseSymbol = computed(() => props.symbol?.split('/')[0] || 'BTC')
const quoteSymbol = computed(() => props.symbol?.split('/')[1] || 'USDT')

watch(() => props.buyOnly, (val) => {
    if (val) {
        direction.value = 'BUY'
    }
}, { immediate: true })

// Sync price with market price initially or when switching
watch(() => props.currentPrice, (newPrice) => {
    if (newPrice && !price.value && orderType.value === 'LIMIT_PRICE') {
        price.value = newPrice
    }
}, { immediate: true })

const total = computed(() => {
    if (!price.value || !amount.value) return 0
    return price.value * amount.value
})

const getWallet = async () => {
    if (!props.symbol) return
    try {
        const res = await fetchWallet(props.symbol)
        const list = Array.isArray(res.data) ? res.data : []
        const baseItem = list.find((item: any) => item.symbol === baseSymbol.value)
        const quoteItem = list.find((item: any) => item.symbol === quoteSymbol.value)
        wallet.value = {
            base: baseItem?.balance || 0,
            quote: quoteItem?.balance || 0
        }
    } catch {
    }
}

watch(() => props.symbol, () => {
    getWallet()
    // Reset inputs
    price.value = props.currentPrice || null
    amount.value = null
})

onMounted(() => {
    getWallet()
})

const setPercent = (p: number) => {
    if (direction.value === 'BUY') {
        // Buy: Use quote balance (USDT)
        const balance = wallet.value.quote
        if (orderType.value === 'MARKET_PRICE') {
            // Market Buy: amount is USDT turnover
            amount.value = balance * (p / 100)
        } else {
            // Limit Buy: total = price * amount <= balance
            if (price.value) {
                const maxTotal = balance * (p / 100)
                amount.value = maxTotal / price.value
            }
        }
    } else {
        // Sell: Use base balance (BTC)
        const balance = wallet.value.base
        amount.value = balance * (p / 100)
    }
}

const submitOrder = async () => {
    if (!props.symbol) return
    loading.value = true
    try {
        // Validation
        if (orderType.value === 'LIMIT_PRICE' && !price.value) {
            toast.error('Please enter price')
            return
        }
        if (!amount.value) {
            toast.error('Please enter amount')
            return
        }

        const params: any = {
            symbol: props.symbol,
            direction: direction.value,
            type: orderType.value,
            useDiscount: 0
        }

        if (orderType.value === 'LIMIT_PRICE') {
            params.price = price.value
            params.amount = amount.value
        } else if (orderType.value === 'MARKET_PRICE') {
            const referencePrice = props.currentPrice || price.value || 0
            if (!referencePrice) {
                toast.error('Market price unavailable')
                return
            }
            params.price = referencePrice
            params.amount = amount.value
        }

        const response = await addOrder(params)

      const res=response.data
        if (res.code === 0 || res.code === 200) {
             toast.success('Order Placed')
             // Refresh wallet
             getWallet()
             // Refresh Orders
             marketStore.triggerOrderRefresh()

             // Clear inputs
             amount.value = null
        } else {
             toast.error(res.message || 'Order failed')
        }
    } catch (e: any) {
        toast.error(e.message || 'Failed to place order')
    } finally {
        loading.value = false
    }
}
</script>
