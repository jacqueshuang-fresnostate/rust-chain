<template>
  <div class="flex flex-col h-full bg-card">
    <!-- Margin & Leverage Header -->
    <div class="flex items-center justify-between p-2 border-b border-border">
        <div class="flex items-center gap-2">
             <button
               @click="toggleMarginMode"
               :class="['text-xs px-2 py-1 rounded border transition-colors', marginMode === 'cross' ? 'bg-primary/10 text-primary border-primary/30' : 'bg-muted/40 hover:bg-muted border-transparent hover:border-border']"
             >
                {{ marginMode === 'cross' ? $t('trade.cross') : $t('trade.isolated') }}
             </button>
             <button @click="openLeverageModal" class="text-xs bg-muted/40 hover:bg-muted px-2 py-1 rounded border border-transparent hover:border-border transition-colors font-mono">
                {{ leverage }}x
             </button>
        </div>
        <div class="flex items-center gap-2">
            <Icon icon="lucide:calculator" class="w-4 h-4 text-muted-foreground hover:text-foreground cursor-pointer" />
        </div>
    </div>

    <!-- Tabs (Open/Close) -->
    <div class="flex border-b border-border">
      <button
        @click="tab = 'OPEN'"
        class="flex-1 py-2 text-sm font-bold border-b-2 transition-colors"
        :class="tab === 'OPEN' ? 'border-primary text-primary bg-primary/5' : 'border-transparent text-muted-foreground hover:text-foreground'"
      >
        {{ $t('trade.open') }}
      </button>
      <button
        @click="tab = 'CLOSE'"
        class="flex-1 py-2 text-sm font-bold border-b-2 transition-colors"
        :class="tab === 'CLOSE' ? 'border-primary text-primary bg-primary/5' : 'border-transparent text-muted-foreground hover:text-foreground'"
      >
        {{ $t('trade.close') }}
      </button>
    </div>

    <!-- Form Content -->
    <div class="p-4 flex flex-col gap-4 flex-1 overflow-y-auto">
      <!-- Order Type -->
      <div class="flex gap-2 mb-1">
         <span @click="orderType = 1" :class="orderType === 1 ? 'text-foreground font-bold underline' : 'text-muted-foreground font-medium hover:text-foreground'" class="text-xs cursor-pointer">{{ $t('trade.limit') }}</span>
         <span @click="orderType = 0" :class="orderType === 0 ? 'text-foreground font-bold underline' : 'text-muted-foreground font-medium hover:text-foreground'" class="text-xs cursor-pointer">{{ $t('trade.market') }}</span>
      </div>

      <!-- Balance -->
      <div class="flex justify-between text-xs items-center">
         <span class="text-muted-foreground flex items-center gap-1">
            <Icon icon="mdi:wallet-outline" class="w-3 h-3" /> {{ $t('trade.avbl') }}
         </span>
         <div class="flex items-center gap-2">
           <span class="font-mono font-bold text-foreground">
              {{ formatNumber(availableBalance) }} USDT
           </span>
           <button @click="showTransferModal = true" class="text-[10px] bg-primary/10 text-primary hover:bg-primary/20 px-1.5 py-0.5 rounded transition-colors" title="Transfer Funds">
             <Icon icon="lucide:arrow-right-left" class="w-3 h-3" />
           </button>
         </div>
      </div>

      <!-- Price Input -->
      <div class="space-y-1" v-if="orderType === 1">
        <div class="flex items-center bg-background border border-input rounded px-3 h-10 focus-within:border-primary transition-colors hover:border-border/80">
          <span class="text-xs text-muted-foreground w-12 shrink-0">{{ $t('trade.price') }}</span>
          <input
            v-model="price"
            type="number"
            class="bg-transparent flex-1 outline-none text-right font-mono text-sm"
            placeholder="0.00"
          />
          <span class="text-xs text-muted-foreground ml-2 w-8 text-right">USDT</span>
        </div>
      </div>
      <div class="h-10 flex items-center px-3 bg-muted/20 border border-transparent rounded text-sm text-muted-foreground" v-else>
          {{ $t('trade.market_price') }}
      </div>

      <!-- Amount Input -->
      <div class="space-y-1">
        <div class="flex items-center bg-background border border-input rounded px-3 h-10 focus-within:border-primary transition-colors hover:border-border/80">
          <span class="text-xs text-muted-foreground w-12 shrink-0">{{ $t('trade.amount') }}</span>
          <input
            v-model="amount"
            type="number"
            class="bg-transparent flex-1 outline-none text-right font-mono text-sm"
            placeholder="0.00"
          />
          <span class="text-xs text-muted-foreground ml-2 w-8 text-right">{{ baseSymbol }}</span>
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

      <!-- Cost Info -->
      <div class="flex justify-between text-[10px] text-muted-foreground mt-1">
        <span>{{ $t('trade.cost') }}</span>
        <span>{{ formatNumber(cost) }} USDT</span>
      </div>

      <!-- Action Buttons -->
      <div class="grid grid-cols-2 gap-3 mt-2">
          <button
            @click="submitOrder(0)"
            :disabled="loading || !canSubmit"
            class="py-3 rounded-lg font-bold text-white shadow-lg transition-all transform active:scale-[0.98] disabled:opacity-50 disabled:cursor-not-allowed bg-up hover:bg-up/90 shadow-up/20"
          >
            <span v-if="loading">...</span>
            <span v-else>{{ tab === 'OPEN' ? $t('trade.open_long') : $t('trade.close_long') }}</span>
          </button>

          <button
            @click="submitOrder(1)"
            :disabled="loading || !canSubmit"
            class="py-3 rounded-lg font-bold text-white shadow-lg transition-all transform active:scale-[0.98] disabled:opacity-50 disabled:cursor-not-allowed bg-down hover:bg-down/90 shadow-down/20"
          >
            <span v-if="loading">...</span>
            <span v-else>{{ tab === 'OPEN' ? $t('trade.open_short') : $t('trade.close_short') }}</span>
          </button>
      </div>
    </div>

    <!-- Leverage Modal -->
    <div v-if="showLeverageModal" class="fixed inset-0 bg-black/50 flex items-center justify-center z-50" @click.self="showLeverageModal = false">
      <div class="bg-card border border-border rounded-lg p-6 w-80">
        <div class="text-base font-bold mb-4">{{ $t('trade.set_leverage') }}</div>
        <div class="grid grid-cols-4 gap-2 mb-4">
            <button v-for="lv in leverageOptions" :key="lv"
                    @click="tempLeverage = lv"
                    :class="['text-sm py-2 rounded border transition-colors', tempLeverage === lv ? 'bg-primary text-primary-foreground border-primary' : 'bg-muted/40 hover:bg-muted border-border']">
                {{ lv }}x
            </button>
        </div>
        <div class="flex gap-2">
          <button @click="showLeverageModal = false" class="flex-1 py-2 text-sm border border-border rounded hover:bg-muted">{{ $t('common.cancel') }}</button>
          <button @click="confirmLeverage" class="flex-1 py-2 text-sm bg-primary text-primary-foreground rounded hover:bg-primary/90">{{ $t('common.confirm') }}</button>
        </div>
      </div>
    </div>

    <!-- Margin Mode Modal -->
    <div v-if="showMarginModal" class="fixed inset-0 bg-black/50 flex items-center justify-center z-50" @click.self="showMarginModal = false">
      <div class="bg-card border border-border rounded-lg p-4 w-72">
        <div class="text-sm font-bold mb-3">{{ $t('trade.margin_mode') }}</div>
        <div class="flex flex-col gap-2 mb-4">
            <button @click="selectMarginMode('cross')"
                    :class="['flex items-center justify-between p-3 rounded border transition-colors', tempMarginMode === 'cross' ? 'border-primary bg-primary/5' : 'border-border hover:bg-muted']">
                <div>
                    <div class="text-sm font-medium">{{ $t('trade.cross') }}</div>
                    <div class="text-xs text-muted-foreground">{{ $t('trade.cross_desc') }}</div>
                </div>
                <div v-if="tempMarginMode === 'cross'" class="w-4 h-4 rounded-full bg-primary flex items-center justify-center">
                    <Icon icon="lucide:check" class="w-3 h-3 text-primary-foreground" />
                </div>
            </button>
            <button @click="selectMarginMode('isolated')"
                    :class="['flex items-center justify-between p-3 rounded border transition-colors', tempMarginMode === 'isolated' ? 'border-primary bg-primary/5' : 'border-border hover:bg-muted']">
                <div>
                    <div class="text-sm font-medium">{{ $t('trade.isolated') }}</div>
                    <div class="text-xs text-muted-foreground">{{ $t('trade.isolated_desc') }}</div>
                </div>
                <div v-if="tempMarginMode === 'isolated'" class="w-4 h-4 rounded-full bg-primary flex items-center justify-center">
                    <Icon icon="lucide:check" class="w-3 h-3 text-primary-foreground" />
                </div>
            </button>
        </div>
        <div class="flex gap-2">
          <button @click="showMarginModal = false" class="flex-1 py-2 text-sm border border-border rounded hover:bg-muted">{{ $t('common.cancel') }}</button>
          <button @click="confirmMarginMode" class="flex-1 py-2 text-sm bg-primary text-primary-foreground rounded hover:bg-primary/90">{{ $t('common.confirm') }}</button>
        </div>
      </div>
    </div>

    <!-- Transfer Modal -->
    <div v-if="showTransferModal" class="fixed inset-0 bg-black/50 flex items-center justify-center z-50" @click.self="showTransferModal = false">
      <div class="bg-card border border-border rounded-lg p-6 w-80">
        <div class="text-base font-bold mb-4">资金划转 (Transfer)</div>
        <div class="flex items-center justify-between mb-4 border rounded p-2 bg-muted/20">
            <div class="flex-1 text-center text-sm font-medium">{{ transferDirection === 'SPOT_TO_SWAP' ? '币币 (SPOT)' : '合约 (SWAP)' }}</div>
            <button @click="toggleTransferDirection" class="px-2 text-primary hover:text-primary/80 transition-colors">
                <Icon icon="lucide:arrow-right-left" class="w-4 h-4" />
            </button>
            <div class="flex-1 text-center text-sm font-medium">{{ transferDirection === 'SPOT_TO_SWAP' ? '合约 (SWAP)' : '币币 (SPOT)' }}</div>
        </div>
        <div class="mb-4">
            <div class="flex justify-between text-xs mb-1 text-muted-foreground">
                <span>划转数量 (USDT)</span>
            </div>
            <div class="flex items-center bg-background border border-input rounded px-3 h-10 focus-within:border-primary transition-colors">
                <input v-model="transferAmount" type="number" class="bg-transparent flex-1 outline-none font-mono text-sm" placeholder="0.00" />
                <button @click="transferAmount = transferDirection === 'SPOT_TO_SWAP' ? 0 : availableBalance" class="text-xs text-primary font-bold ml-2">全部</button>
            </div>
            <div class="text-xs text-muted-foreground mt-1" v-if="transferDirection === 'SWAP_TO_SPOT'">可用: {{ formatNumber(availableBalance) }} USDT</div>
        </div>
        <div class="flex gap-2">
          <button @click="showTransferModal = false" class="flex-1 py-2 text-sm border border-border rounded hover:bg-muted">{{ $t('common.cancel') }}</button>
          <button @click="confirmTransfer" :disabled="transferring || !transferAmount || transferAmount <= 0" class="flex-1 py-2 text-sm bg-primary text-primary-foreground rounded hover:bg-primary/90 disabled:opacity-50 flex items-center justify-center">
              <span v-if="transferring" class="animate-spin mr-1">⏳</span> {{ $t('common.confirm') }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { formatNumber } from '@/utils/format'
import { Icon } from '@iconify/vue'
import { useToast } from 'vue-toastification'
import { useContractStore } from '@/stores/contract'
import type { OrderType } from '@/api/contract'

const props = defineProps<{
  symbol?: string
  currentPrice?: number
}>()

const { t: $t } = useI18n()
const toast = useToast()
const contractStore = useContractStore()

const tab = ref<'OPEN' | 'CLOSE'>('OPEN')
const orderType = ref<OrderType>(1) // 0市价 1限价
const price = ref<number | null>(null)
const amount = ref<number | null>(null)
const loading = ref(false)
const leverage = ref(10)
const marginMode = ref<'cross' | 'isolated'>('isolated')
const showLeverageModal = ref(false)
const showMarginModal = ref(false)
const showTransferModal = ref(false)
const tempLeverage = ref(10)
const tempMarginMode = ref<'cross' | 'isolated'>('isolated')

const transferDirection = ref<'SPOT_TO_SWAP' | 'SWAP_TO_SPOT'>('SPOT_TO_SWAP')
const transferAmount = ref<number | null>(null)
const transferring = ref(false)

const baseSymbol = computed(() => props.symbol?.split('/')[0] || 'BTC')

const availableBalance = computed(() => contractStore.getAvailableBalance('USDT'))

// Get the wallet for the current active symbol
const activeWallet = computed(() =>
    contractStore.wallets.find(w => w.symbol === props.symbol)
)

const leverageOptions = computed(() => contractStore.activeCoin?.leverage || [1, 2, 3, 5, 10, 20, 50, 100])

const cost = computed(() => {
    if (!amount.value) return 0
    const p = price.value || props.currentPrice || 0
    return (p * amount.value) / leverage.value
})

const canSubmit = computed(() => {
    if (!props.symbol) return false
    if (!amount.value || amount.value <= 0) return false
    if (orderType.value === 1 && !price.value) return false // 限价需要价格
    return true
})

// Sync leverage and margin mode from wallet data
const syncFromWallet = () => {
    const w = activeWallet.value
    if (w) {
        leverage.value = w.usdtBuyLeverage || 10
        tempLeverage.value = leverage.value
        marginMode.value = w.usdtPattern === 'CROSSED' ? 'cross' : 'isolated'
    }
}

watch(() => props.currentPrice, (newPrice) => {
    if (newPrice && !price.value && orderType.value === 1) {
        price.value = newPrice
    }
}, { immediate: true })

watch(() => props.symbol, () => {
    price.value = props.currentPrice || null
    amount.value = null
    syncFromWallet()
})

// Also sync when wallets are loaded
watch(() => contractStore.wallets, () => {
    syncFromWallet()
}, { deep: true })

const setPercent = (p: number) => {
    const balance = availableBalance.value
    const pPrice = price.value || props.currentPrice || 0
    if (pPrice === 0) return
    const maxAmount = (balance * leverage.value) / pPrice
    amount.value = maxAmount * (p / 100)
}

const toggleMarginMode = () => {
    tempMarginMode.value = marginMode.value
    showMarginModal.value = true
}

const selectMarginMode = (mode: 'cross' | 'isolated') => {
    tempMarginMode.value = mode
}

const confirmMarginMode = async () => {
    const coinId = contractStore.activeCoin?.id
    if (!coinId) return
    const targetPattern = tempMarginMode.value === 'cross' ? 'CROSSED' : 'FIXED'
    try {
        await contractStore.setMarginMode(coinId, targetPattern)
        marginMode.value = tempMarginMode.value
        showMarginModal.value = false
        toast.success($t('trade.switch_success'))
    } catch (e: any) {
        toast.error(e?.response?.data?.message || $t('trade.switch_failed'))
    }
}

const openLeverageModal = () => {
    tempLeverage.value = leverage.value
    showLeverageModal.value = true
}

const confirmLeverage = async () => {
    const coinId = contractStore.activeCoin?.id
    if (!coinId) return
    try {
        await contractStore.setLeverage(coinId, tempLeverage.value, 0)
        leverage.value = tempLeverage.value
        showLeverageModal.value = false
        toast.success($t('trade.leverage_set'))
    } catch (e: any) {
        toast.error(e?.response?.data?.message || $t('trade.leverage_failed'))
    }
}

/**
 * Submit order
 * OPEN tab: direction 0=买入开多, 1=卖出开空
 * CLOSE tab: direction 0=买入平空, 1=卖出平多
 * @param btnDirection 0=long/buy side, 1=short/sell side
 */
const submitOrder = async (btnDirection: 0 | 1) => {
    const coinId = contractStore.activeCoin?.id
    if (!coinId || !props.symbol) {
        toast.warning($t('trade.select_coin'))
        return
    }
    if (!amount.value || amount.value <= 0) return

    loading.value = true
    try {
        // orderType: 0=市价 1=限价
        const type = orderType.value as 0 | 1

        if (tab.value === 'OPEN') {
            // 开仓: direction 0=买入开多 1=卖出开空
            await contractStore.submitOpenPosition({
                contractCoinId: coinId,
                direction: btnDirection,
                type,
                triggerPrice: 0,
                entrustPrice: type === 1 ? (price.value || 0) : 0,
                leverage: leverage.value,
                volume: amount.value
            })
            const dirText = btnDirection === 0 ? $t('trade.open_long') : $t('trade.open_short')
            toast.success(`${dirText} ${$t('trade.success')}`)
        } else {
            // 平仓: direction 0=买入平空 1=卖出平多
            await contractStore.submitClosePosition({
                contractCoinId: coinId,
                direction: btnDirection,
                type,
                triggerPrice: 0,
                entrustPrice: type === 1 ? (price.value || 0) : 0,
                volume: amount.value
            })
            const dirText = btnDirection === 0 ? $t('trade.close_short') : $t('trade.close_long')
            toast.success(`${dirText} ${$t('trade.success')}`)
        }
        amount.value = null
    } catch (e: any) {
        toast.error(e?.response?.data?.message || e.message || $t('trade.order_failed'))
    } finally {
        loading.value = false
    }
}

const toggleTransferDirection = () => {
    transferDirection.value = transferDirection.value === 'SPOT_TO_SWAP' ? 'SWAP_TO_SPOT' : 'SPOT_TO_SWAP'
    transferAmount.value = null
}

const confirmTransfer = async () => {
    if (!transferAmount.value || transferAmount.value <= 0) return
    transferring.value = true
    try {
        await contractStore.transfer({
            unit: 'USDT',
            from: transferDirection.value === 'SPOT_TO_SWAP' ? 'SPOT' : 'SWAP',
            to: transferDirection.value === 'SPOT_TO_SWAP' ? 'SWAP' : 'SPOT',
            amount: transferAmount.value
        })
        toast.success('Funds transferred successfully')
        showTransferModal.value = false
        transferAmount.value = null
    } catch (e: any) {
        toast.error(e.message || 'Transfer failed')
    } finally {
        transferring.value = false
    }
}

onMounted(async () => {
    await contractStore.loadWallets()
    syncFromWallet()
})
</script>