<template>
  <div class="space-y-6">
    <h2 class="text-2xl font-bold flex items-center gap-2">
      <Icon icon="mdi:arrow-down-circle-outline" class="text-primary" />
      {{ t('wallet.deposit') }}
    </h2>

    <div class="bg-card border border-border rounded-xl shadow-sm overflow-hidden">
      <div class="flex gap-1 border-b border-border px-3 pt-3 overflow-x-auto">
        <button
          v-for="tab in rechargeTabs"
          :key="tab.value"
          @click="activeRechargeTab = tab.value"
          class="px-4 py-3 text-sm font-medium border-b-2 transition-colors whitespace-nowrap"
          :class="activeRechargeTab === tab.value ? 'border-primary text-primary' : 'border-transparent text-muted-foreground hover:text-foreground'"
        >
          {{ t(tab.labelKey) }}
        </button>
      </div>

      <!-- Deposit Form -->
      <div v-if="activeRechargeTab === 'normal'" class="p-6 space-y-6">
        <div>
          <label class="block text-sm font-medium text-muted-foreground mb-2">{{ t('wallet.select_coin') }}</label>
          <div class="flex flex-wrap gap-2">
            <button
              v-for="coin in supportedCoins"
              :key="coin"
              @click="selectCoin(coin)"
              class="px-4 py-2 rounded border transition-colors flex items-center gap-2"
              :class="selectedCoin === coin ? 'bg-primary text-primary-foreground border-primary' : 'bg-background border-border hover:border-primary/50'"
            >
              <Icon :icon="getCoinIcon(coin)" />
              {{ coin }}
            </button>
          </div>
        </div>

        <div v-if="loadingNetworks" class="py-4 flex justify-center">
            <Icon icon="mdi:loading" class="animate-spin text-xl text-primary" />
        </div>

        <div v-else-if="availableNetworks.length > 0" class="animate-fade-in">
             <label class="block text-sm font-medium text-muted-foreground mb-2">{{ t('wallet.select_network') }}</label>
             <div class="flex flex-wrap gap-2">
                <button
                  v-for="network in availableNetworks"
                  :key="network.networkKey || network.name"
                  @click="selectNetwork(network)"
                  class="px-3 py-1.5 text-sm rounded border transition-colors"
                  :class="selectedNetworkKey === (network.networkKey || network.name) ? 'bg-primary/20 text-primary border-primary' : 'bg-background border-border hover:border-primary/50'"
                >
                  {{ network.name }}
                </button>
             </div>
        </div>

        <div v-if="loading" class="py-8 flex justify-center">
            <Icon icon="mdi:loading" class="animate-spin text-3xl text-primary" />
        </div>

        <div v-else-if="walletData" class="space-y-6 animate-fade-in">
            <div class="p-4 bg-muted/30 rounded-lg border border-border">
                <div class="text-xs text-muted-foreground mb-1">{{ t('wallet.deposit_address') }}</div>
                <div class="flex items-center gap-2">
                    <code class="flex-1 bg-background p-3 rounded border border-border font-mono text-sm break-all">
                        {{ walletData.address }}
                    </code>
                    <button @click="copyAddress" class="p-3 bg-primary/10 text-primary rounded hover:bg-primary/20 transition-colors" :title="t('wallet.copy_address')">
                        <Icon icon="mdi:content-copy" />
                    </button>
                </div>
            </div>

            <div class="flex flex-col items-center justify-center p-6 border border-dashed border-border rounded-lg bg-background">
                <div class="w-48 h-48 bg-white p-2 rounded mb-4">
                     <img :src="`https://api.qrserver.com/v1/create-qr-code/?size=200x200&data=${walletData.address}`" :alt="t('wallet.qr_alt')" class="w-full h-full object-contain" />
                </div>
                <p class="text-sm text-muted-foreground text-center">{{ t('wallet.scan_qr', { coin: selectedCoin, network: selectedNetwork }) }}</p>
            </div>

            <div class="space-y-2 text-sm text-muted-foreground bg-yellow-500/5 p-4 rounded border border-yellow-500/20">
                <div class="flex items-start gap-2">
                    <Icon icon="mdi:alert-circle-outline" class="text-yellow-500 mt-0.5" />
                    <p>{{ t('wallet.deposit_warning', { coin: selectedCoin, network: selectedNetwork }) }}</p>
                </div>
                <div class="flex items-start gap-2">
                    <Icon icon="mdi:information-outline" class="text-blue-500 mt-0.5" />
                    <p>{{ t('wallet.minimum_deposit') }}: <span class="font-bold text-foreground">{{ walletData.coin.minRechargeAmount }} {{ walletData.unit || selectedCoin }}</span></p>
                </div>
                <div class="flex items-start gap-2">
                    <Icon icon="mdi:cash-minus" class="text-blue-500 mt-0.5" />
                    <p>{{ t('wallet.fee') }}: <span class="font-bold text-foreground">{{ walletData.coin.depositFee }} {{ walletData.unit || selectedCoin }}</span></p>
                </div>
            </div>
        </div>
      </div>

      <div v-else class="p-6 space-y-5">
        <div class="flex items-center justify-between gap-3">
          <h3 class="font-bold">{{ t('wallet.quick_deposit') }}</h3>
          <span v-if="quickConfig?.enabled" class="text-xs px-2 py-1 rounded bg-primary/10 text-primary">
            {{ quickConfig.token.toUpperCase() }} · {{ quickConfig.network }}
          </span>
        </div>

        <div v-if="quickConfig?.enabled" class="space-y-4">
          <label class="block text-sm font-medium text-muted-foreground">
            {{ t('wallet.amount') }} ({{ quickConfig.currency.toUpperCase() }})
            <input
              v-model="quickAmount"
              type="number"
              min="0"
              step="0.01"
              class="mt-2 w-full px-3 py-2 rounded border border-border bg-background text-foreground outline-none focus:border-primary"
            />
          </label>
          <div class="flex flex-wrap gap-2 text-xs text-muted-foreground">
            <span>{{ t('wallet.min') }} {{ formatQuickAmount(quickConfig.min_amount) }} {{ quickConfig.currency.toUpperCase() }}</span>
            <span v-if="quickConfig.max_amount">{{ t('wallet.max') }} {{ formatQuickAmount(quickConfig.max_amount) }} {{ quickConfig.currency.toUpperCase() }}</span>
          </div>
          <button
            @click="submitQuickRecharge"
            :disabled="quickRechargeLoading || !quickAmount"
            class="w-full py-2.5 rounded bg-primary text-primary-foreground font-medium hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            <span v-if="quickRechargeLoading" class="inline-flex items-center gap-2">
              <Icon icon="mdi:loading" class="animate-spin" />
              {{ t('wallet.creating_order') }}
            </span>
            <span v-else>{{ t('wallet.create_payment') }}</span>
          </button>

          <div v-if="quickOrder" class="p-4 rounded border border-border bg-muted/20 space-y-3">
            <div class="text-xs text-muted-foreground">{{ t('wallet.order') }} {{ quickOrder.order_id }}</div>
            <div class="grid grid-cols-2 gap-3 text-sm">
              <div>
                <div class="text-muted-foreground text-xs">{{ t('wallet.pay') }}</div>
                <div class="font-medium">{{ formatQuickAmount(quickOrder.fiat_amount) }} {{ quickOrder.currency.toUpperCase() }}</div>
              </div>
              <div>
                <div class="text-muted-foreground text-xs">{{ t('wallet.receive') }}</div>
                <div class="font-medium">{{ quickOrder.actual_amount ? formatQuickAmount(quickOrder.actual_amount) : '-' }} {{ quickOrder.token.toUpperCase() }}</div>
              </div>
            </div>
            <button
              v-if="quickOrder.payment_url"
              @click="openQuickPayment"
              class="w-full py-2 rounded border border-primary/40 text-primary hover:bg-primary/10 transition-colors"
            >
              {{ t('wallet.open_payment') }}
            </button>
          </div>
        </div>

        <div v-else class="text-sm text-muted-foreground">
          {{ t('wallet.quick_unavailable') }}
        </div>
      </div>
    </div>

    <div class="bg-card border border-border rounded-xl p-6 shadow-sm">
      <h3 class="font-bold mb-4">{{ t('wallet.tips') }}</h3>
      <ul class="list-disc pl-5 space-y-2 text-sm text-muted-foreground">
          <li>{{ t('wallet.tip_confirmations') }}</li>
          <li>{{ t('wallet.tip_security') }}</li>
          <li>{{ t('wallet.tip_address') }}</li>
      </ul>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { Icon } from '@iconify/vue'
import {
    createQuickRechargeOrder,
    fetchQuickRechargeConfig,
    fetchSupportedCoins,
    fetchCoinNetworks,
    getDepositAddress,
    type QuickRechargeConfig,
    type QuickRechargeOrder,
    type QuickRechargeReturnTarget,
    type WalletAddress,
    type CoinNetwork,
} from '@/api/wallet'
import { useToast } from 'vue-toastification'
import { useI18n } from 'vue-i18n'

const toast = useToast()
const { t } = useI18n()
const supportedCoins = ref<string[]>([])
const selectedCoin = ref<string>('')
const availableNetworks = ref<CoinNetwork[]>([])
const selectedNetwork = ref<string>('')
const selectedNetworkKey = ref<string>('')
const walletData = ref<WalletAddress | null>(null)
const quickConfig = ref<QuickRechargeConfig | null>(null)
const quickAmount = ref('')
const quickOrder = ref<QuickRechargeOrder | null>(null)

const loading = ref(false)
const loadingNetworks = ref(false)
const quickRechargeLoading = ref(false)

type QuickRechargeBridgeWindow = Window & {
    __TAURI__?: unknown
    __TAURI_INTERNALS__?: unknown
    Capacitor?: { getPlatform?: () => string }
    cordova?: unknown
}
type RechargeTab = 'normal' | 'quick'

const activeRechargeTab = ref<RechargeTab>('normal')
const rechargeTabs: Array<{ value: RechargeTab; labelKey: string }> = [
    { value: 'normal', labelKey: 'wallet.normal_deposit' },
    { value: 'quick', labelKey: 'wallet.quick_deposit' },
]

const getCoinIcon = (coin: string) => {
    switch(coin) {
        case 'USDT': return 'mdi:currency-usd'
        case 'BTC': return 'mdi:bitcoin'
        case 'ETH': return 'mdi:ethereum'
        default: return 'mdi:currency-usd-circle'
    }
}

const loadCoins = async () => {
    try {
        const res = await fetchSupportedCoins()
        if (res.data.code === 0) {
            supportedCoins.value = res.data.data
            if (supportedCoins.value.length > 0) {
                selectCoin(supportedCoins.value[0])
            }
        }
    } catch (e) {
        console.error(e)
        toast.error(t('wallet.load_coins_failed'))
    }
}

const selectCoin = async (coin: string) => {
    selectedCoin.value = coin
    selectedNetwork.value = ''
    selectedNetworkKey.value = ''
    walletData.value = null
    availableNetworks.value = []

    loadingNetworks.value = true
    try {
        const res = await fetchCoinNetworks(coin)
        if (res.data.code === 0 && res.data.data.length > 0) {
            availableNetworks.value = res.data.data.filter(n => n.depositEnabled)
            if (availableNetworks.value.length > 0) {
                // Auto select first network
                selectNetwork(availableNetworks.value[0])
            }
        } else {
             toast.warning(t('wallet.no_networks'))
        }
    } catch (e) {
        console.error(e)
        toast.error(t('wallet.load_networks_failed'))
    } finally {
        loadingNetworks.value = false
    }
}

const selectNetwork = async (network: CoinNetwork | string) => {
    const displayName = typeof network === 'string' ? network : network.name
    const networkKey = typeof network === 'string' ? network : network.networkKey || network.name
    selectedNetwork.value = displayName
    selectedNetworkKey.value = networkKey
    loading.value = true
    walletData.value = null
    try {
        const res = await getDepositAddress(selectedCoin.value, networkKey)
        if (res.data.code === 0) {
            walletData.value = res.data.data
        } else {
            toast.error(t('wallet.address_failed'))
        }
    } catch (e) {
        console.error(e)
        toast.error(t('wallet.address_fetch_failed'))
    } finally {
        loading.value = false
    }
}

const copyAddress = () => {
    if (walletData.value?.address) {
        navigator.clipboard.writeText(walletData.value.address)
        toast.success(t('wallet.address_copied'))
    }
}

const loadQuickRechargeConfig = async () => {
    try {
        const res = await fetchQuickRechargeConfig()
        if (res.data.code === 0) {
            quickConfig.value = res.data.data
        }
    } catch (e) {
        console.error(e)
    }
}

const submitQuickRecharge = async () => {
    const amount = Number(quickAmount.value)
    if (!Number.isFinite(amount) || amount <= 0) {
        toast.error(t('wallet.invalid_amount'))
        return
    }
    quickRechargeLoading.value = true
    try {
        const res = await createQuickRechargeOrder(quickAmount.value, detectQuickRechargeReturnTarget())
        if (res.data.code === 0) {
            quickOrder.value = res.data.data
            toast.success(t('wallet.payment_created'))
            openQuickPayment()
        }
    } catch (e) {
        console.error(e)
        toast.error(t('wallet.payment_failed'))
    } finally {
        quickRechargeLoading.value = false
    }
}

const detectQuickRechargeReturnTarget = (): QuickRechargeReturnTarget => {
    const bridgeWindow = window as QuickRechargeBridgeWindow
    const userAgent = navigator.userAgent.toLowerCase()
    const capacitorPlatform = bridgeWindow.Capacitor?.getPlatform?.()
    if (capacitorPlatform === 'ios') return 'ios_app'
    if (capacitorPlatform === 'android') return 'android_app'
    if (bridgeWindow.cordova && /iphone|ipad|ipod/.test(userAgent)) return 'ios_app'
    if (bridgeWindow.cordova && /android/.test(userAgent)) return 'android_app'
    if (bridgeWindow.__TAURI__ || bridgeWindow.__TAURI_INTERNALS__) {
        return /macintosh|mac os x/.test(userAgent) ? 'mac_app' : 'pc_app'
    }
    if (/android|iphone|ipad|ipod|mobile/.test(userAgent)) return 'mobile_web'
    return 'desktop_web'
}

const openQuickPayment = () => {
    const paymentUrl = quickOrder.value?.payment_url
    if (paymentUrl) {
        const opened = window.open(paymentUrl, '_blank', 'noopener,noreferrer')
        if (!opened) {
            window.location.assign(paymentUrl)
        }
    }
}

const formatQuickAmount = (value: string | number | null | undefined) => {
    const amount = Number(value)
    if (!Number.isFinite(amount)) return '-'
    return new Intl.NumberFormat(undefined, { maximumFractionDigits: 8 }).format(amount)
}

onMounted(() => {
    loadCoins()
    loadQuickRechargeConfig()
})
</script>

<style scoped>
.animate-fade-in {
    animation: fadeIn 0.3s ease-in-out;
}
@keyframes fadeIn {
    from { opacity: 0; transform: translateY(10px); }
    to { opacity: 1; transform: translateY(0); }
}
</style>
