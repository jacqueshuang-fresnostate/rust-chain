<template>
  <div class="flex flex-col h-full bg-card">
    <!-- Margin & Leverage Header -->
    <div class="flex items-center justify-between p-2 border-b border-border">
        <div class="flex items-center gap-2">
             <span class="text-xs px-2 py-1 rounded border bg-muted/40 text-muted-foreground border-transparent">
                {{ $t('trade.isolated') }}
             </span>
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
    <AuthRequiredState v-if="!isLoggedIn" compact class="m-4" />

    <div v-else class="p-4 flex flex-col gap-4 flex-1 overflow-y-auto">
      <!-- Balance -->
      <div class="flex justify-between text-xs items-center">
         <span class="text-muted-foreground flex items-center gap-1">
            <Icon icon="mdi:wallet-outline" class="w-3 h-3" /> {{ $t('trade.avbl') }}
         </span>
         <div class="flex items-center gap-2">
           <span class="font-mono font-bold text-foreground">
              {{ formatNumber(availableBalance) }} {{ marginAssetSymbol }}
           </span>
           <button type="button" class="text-primary hover:underline" @click="openTransferModal">
              {{ $t('assets.transfer') }}
           </button>
         </div>
      </div>

      <div class="h-10 flex items-center px-3 bg-muted/20 border border-transparent rounded text-sm text-muted-foreground">
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
          <span class="text-xs text-muted-foreground ml-2 w-8 text-right">{{ marginAssetSymbol }}</span>
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

    <!-- Transfer Modal -->
    <div v-if="showTransferModal" class="fixed inset-0 bg-black/50 flex items-center justify-center z-50" @click.self="showTransferModal = false">
      <div class="bg-card border border-border rounded-lg p-6 w-80">
        <div class="text-base font-bold mb-4">{{ $t('trade.transfer_funds') }}</div>
        <div class="flex gap-2 mb-4">
          <button
            type="button"
            class="flex-1 rounded border py-2 text-xs font-bold"
            :class="transferDirection === 'SPOT_TO_SWAP' ? 'border-primary bg-primary/10 text-primary' : 'border-border bg-muted/30'"
            @click="transferDirection = 'SPOT_TO_SWAP'"
          >
            {{ $t('trade.spot_account') }} → {{ $t('trade.contract_account') }}
          </button>
          <button
            type="button"
            class="flex-1 rounded border py-2 text-xs font-bold"
            :class="transferDirection === 'SWAP_TO_SPOT' ? 'border-primary bg-primary/10 text-primary' : 'border-border bg-muted/30'"
            @click="transferDirection = 'SWAP_TO_SPOT'"
          >
            {{ $t('trade.contract_account') }} → {{ $t('trade.spot_account') }}
          </button>
        </div>
        <div class="mb-4 flex items-center bg-background border border-input rounded px-3 h-10 focus-within:border-primary transition-colors">
          <span class="text-xs text-muted-foreground w-16 shrink-0">{{ $t('trade.amount') }}</span>
          <input v-model="transferAmount" type="number" min="0" step="any" class="bg-transparent flex-1 outline-none text-right font-mono text-sm" placeholder="0.00" />
          <span class="text-xs text-muted-foreground ml-2">USDT</span>
        </div>
        <div class="flex gap-2">
          <button @click="showTransferModal = false" class="flex-1 py-2 text-sm border border-border rounded hover:bg-muted">{{ $t('common.cancel') }}</button>
          <button @click="confirmTransfer" :disabled="transferLoading || !canTransfer" class="flex-1 py-2 text-sm bg-primary text-primary-foreground rounded hover:bg-primary/90 disabled:opacity-50">
            {{ transferLoading ? '...' : $t('common.confirm') }}
          </button>
        </div>
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

  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { formatNumber } from '@/utils/format'
import { Icon } from '@iconify/vue'
import { useToast } from 'vue-toastification'
import { useContractStore } from '@/stores/contract'
import AuthRequiredState from '@/components/common/AuthRequiredState.vue'
import { useAuthRequired } from '@/composables/useAuthRequired'

const props = defineProps<{
  symbol?: string
  currentPrice?: number
}>()

const { t: $t } = useI18n()
const toast = useToast()
const contractStore = useContractStore()
const { isLoggedIn, goToLogin } = useAuthRequired()

const tab = ref<'OPEN' | 'CLOSE'>('OPEN')
const amount = ref<number | null>(null)
const loading = ref(false)
const leverage = ref(10)
const showLeverageModal = ref(false)
const showTransferModal = ref(false)
const tempLeverage = ref(10)
const transferDirection = ref<'SPOT_TO_SWAP' | 'SWAP_TO_SPOT'>('SPOT_TO_SWAP')
const transferAmount = ref<number | null>(null)
const transferLoading = ref(false)

const marginAssetSymbol = computed(() => contractStore.activeCoin?.baseSymbol || 'USDT')

const availableBalance = computed(() => contractStore.getAvailableBalance(marginAssetSymbol.value))

// Get the wallet for the current active symbol
const activeWallet = computed(() =>
    contractStore.wallets.find(w => w.symbol === props.symbol)
)

const leverageOptions = computed(() => contractStore.activeCoin?.leverage || [1, 2, 3, 5, 10, 20, 50, 100])
const cost = computed(() => {
    if (!amount.value) return 0
    return amount.value
})

const canSubmit = computed(() => {
    if (!isLoggedIn.value) return false
    if (!props.symbol) return false
    if (!amount.value || amount.value <= 0) return false
    return true
})

const canTransfer = computed(() => Boolean(transferAmount.value && transferAmount.value > 0))

// Sync leverage and margin mode from wallet data
const syncFromWallet = () => {
    const w = activeWallet.value
    if (w) {
        leverage.value = w.usdtBuyLeverage || 10
        tempLeverage.value = leverage.value
    }
}

watch(() => props.symbol, () => {
    amount.value = null
    syncFromWallet()
})

// Also sync when wallets are loaded
watch(() => contractStore.wallets, () => {
    syncFromWallet()
}, { deep: true })

const setPercent = (p: number) => {
    if (!isLoggedIn.value) {
        goToLogin()
        return
    }
    const balance = availableBalance.value
    amount.value = balance * (p / 100)
}

const openLeverageModal = () => {
    if (!isLoggedIn.value) {
        goToLogin()
        return
    }
    tempLeverage.value = leverage.value
    showLeverageModal.value = true
}

const confirmLeverage = async () => {
    if (!isLoggedIn.value) {
        goToLogin()
        return
    }
    const coinId = contractStore.activeCoin?.id
    if (!coinId) return
    try {
        await contractStore.submitModifyLeverage(coinId, tempLeverage.value, 0)
        leverage.value = tempLeverage.value
        showLeverageModal.value = false
        toast.success($t('trade.leverage_set'))
    } catch (e: any) {
        toast.error(e?.response?.data?.message || e.message || $t('trade.leverage_failed'))
    }
}

const openTransferModal = () => {
    if (!isLoggedIn.value) {
        goToLogin()
        return
    }
    transferDirection.value = 'SPOT_TO_SWAP'
    transferAmount.value = null
    showTransferModal.value = true
}

const confirmTransfer = async () => {
    if (!transferAmount.value || transferAmount.value <= 0) return
    transferLoading.value = true
    try {
        await contractStore.submitTransfer({
            unit: marginAssetSymbol.value,
            from: transferDirection.value === 'SPOT_TO_SWAP' ? 'SPOT' : 'SWAP',
            to: transferDirection.value === 'SPOT_TO_SWAP' ? 'SWAP' : 'SPOT',
            amount: transferAmount.value
        })
        toast.success($t('trade.transfer_success'))
        showTransferModal.value = false
    } catch (e: any) {
        toast.error(e?.response?.data?.message || e.message || $t('trade.transfer_failed'))
    } finally {
        transferLoading.value = false
    }
}

/**
 * Submit order
 * OPEN tab: direction 0=买入开多, 1=卖出开空
 * CLOSE tab: direction 0=买入平空, 1=卖出平多
 * @param btnDirection 0=long/buy side, 1=short/sell side
 */
const submitOrder = async (btnDirection: 0 | 1) => {
    if (!isLoggedIn.value) {
        goToLogin()
        return
    }
    const coinId = contractStore.activeCoin?.id
    if (!coinId || !props.symbol) {
        toast.warning($t('trade.select_coin'))
        return
    }
    if (!amount.value || amount.value <= 0) return

    loading.value = true
    try {
        if (tab.value === 'OPEN') {
            // 开仓: direction 0=买入开多 1=卖出开空
            await contractStore.submitOpenPosition({
                contractCoinId: coinId,
                direction: btnDirection,
                type: 0,
                leverage: leverage.value,
                marginMode: 'isolated',
                volume: amount.value
            })
            const dirText = btnDirection === 0 ? $t('trade.open_long') : $t('trade.open_short')
            toast.success(`${dirText} ${$t('trade.success')}`)
        } else {
            // 平仓: direction 0=买入平空 1=卖出平多
            await contractStore.submitClosePosition({
                contractCoinId: coinId,
                direction: btnDirection,
                type: 0,
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

onMounted(async () => {
    if (!isLoggedIn.value) return
    await contractStore.loadWallets()
    syncFromWallet()
})

watch(isLoggedIn, async (loggedIn) => {
    if (!loggedIn) return
    await contractStore.loadWallets()
    syncFromWallet()
})
</script>
