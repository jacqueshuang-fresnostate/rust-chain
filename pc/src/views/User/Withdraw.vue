<template>
  <div class="space-y-6">
    <h2 class="text-2xl font-bold flex items-center gap-2">
      <Icon icon="mdi:arrow-up-circle-outline" class="text-primary" />
      {{ t('wallet.withdraw') }}
    </h2>

    <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
      <!-- Withdraw Form -->
      <div class="bg-card border border-border rounded-xl p-6 shadow-sm space-y-6">
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
                  :key="network.name"
                  @click="selectNetwork(network.name)"
                  class="px-3 py-1.5 text-sm rounded border transition-colors"
                  :class="selectedNetwork === network.name ? 'bg-primary/20 text-primary border-primary' : 'bg-background border-border hover:border-primary/50'"
                >
                  {{ network.name }}
                </button>
             </div>
        </div>

        <div v-if="loadingInfo" class="py-8 flex justify-center">
             <Icon icon="mdi:loading" class="animate-spin text-3xl text-primary" />
        </div>

        <div v-else-if="coinInfo" class="space-y-4 animate-fade-in">
             <!-- Balance Display -->
             <div class="bg-muted/30 p-4 rounded-lg flex justify-between items-center">
                 <span class="text-sm text-muted-foreground">{{ t('wallet.available_balance') }}</span>
                 <span class="font-bold font-mono text-lg">{{ getBalance(selectedCoin) }} {{ selectedCoin }}</span>
             </div>

             <!-- Address Input -->
             <div>
                 <label class="block text-xs font-medium text-muted-foreground mb-1">{{ t('wallet.withdraw_address') }}</label>
                 <div class="relative">
                    <input
                        v-model="form.address"
                        type="text"
                        :placeholder="t('wallet.withdraw_address_placeholder')"
                        class="w-full bg-background border border-border rounded-lg px-4 py-3 text-sm focus:border-primary focus:ring-1 focus:ring-primary outline-none transition-all"
                    />
                 </div>
             </div>

             <!-- Amount Input -->
             <div>
                 <label class="block text-xs font-medium text-muted-foreground mb-1">{{ t('wallet.amount') }}</label>
                 <div class="relative">
                    <input
                        v-model.number="form.amount"
                        type="number"
                        placeholder="0.00"
                        class="w-full bg-background border border-border rounded-lg px-4 py-3 text-sm focus:border-primary focus:ring-1 focus:ring-primary outline-none transition-all"
                    />
                    <button
                        @click="setMaxAmount"
                        class="absolute right-3 top-1/2 -translate-y-1/2 text-xs font-bold text-primary hover:text-primary/80"
                    >
                        {{ t('wallet.max') }}
                    </button>
                 </div>
                 <div class="flex justify-between text-xs text-muted-foreground mt-1">
                     <span>{{ t('wallet.fee') }}: {{ withdrawFeeDisplay }} {{ selectedCoin }}</span>
                     <span>{{ t('wallet.min') }}: {{ coinInfo.coin.minWithdrawAmount }}</span>
                 </div>
             </div>

             <!-- Security Verification -->
             <div class="border-t border-border pt-4 space-y-3">
                 <div class="flex justify-between items-center text-sm">
                     <span class="font-medium">{{ t('wallet.security_verification') }}</span>
                     <span class="text-xs text-muted-foreground">{{ withdrawPolicyLabel }}</span>
                 </div>
                 <div v-if="needsFundPassword">
                     <label class="block text-xs font-medium text-muted-foreground mb-1">{{ t('wallet.fund_password') }}</label>
                     <input
                         v-model="form.fundPassword"
                         type="password"
                         :placeholder="t('wallet.fund_password_placeholder')"
                         class="w-full bg-background border border-border rounded-lg px-4 py-3 text-sm focus:border-primary focus:ring-1 focus:ring-primary outline-none transition-all"
                     />
                 </div>
                 <div v-if="needsTwoFactor">
                     <label class="block text-xs font-medium text-muted-foreground mb-1">{{ t('wallet.two_factor_code') }}</label>
                     <input
                         v-model="form.totpCode"
                         inputmode="numeric"
                         maxlength="6"
                         :placeholder="t('wallet.authenticator_code_placeholder')"
                         class="w-full bg-background border border-border rounded-lg px-4 py-3 text-sm focus:border-primary focus:ring-1 focus:ring-primary outline-none transition-all"
                     />
                     <p v-if="twoFactorStatus && !twoFactorStatus.totp_enabled" class="text-xs text-yellow-500 mt-1">
                         {{ t('wallet.withdrawal_2fa_required') }}
                     </p>
                 </div>
             </div>

             <!-- Summary -->
             <div class="border-t border-border pt-4">
                 <div class="flex justify-between items-center mb-4">
                     <span class="text-sm font-medium">{{ t('wallet.receive_amount') }}</span>
                     <span class="text-xl font-bold text-primary">{{ receiveAmount }} {{ selectedCoin }}</span>
                 </div>
                 <button
                    @click="handleSubmit"
                    :disabled="submitting || !isValid"
                    class="w-full py-3 bg-primary text-primary-foreground font-bold rounded-lg hover:bg-primary/90 transition-all disabled:opacity-50 disabled:cursor-not-allowed flex justify-center items-center gap-2"
                 >
                    <Icon v-if="submitting" icon="mdi:loading" class="animate-spin" />
                    {{ submitting ? t('wallet.processing') : t('wallet.withdraw') }}
                 </button>
             </div>
        </div>
      </div>

      <!-- Instructions -->
      <div class="bg-card border border-border rounded-xl p-6 shadow-sm">
        <h3 class="font-bold mb-4">{{ t('wallet.important_info') }}</h3>
        <div class="space-y-4 text-sm text-muted-foreground">
            <p>{{ t('wallet.withdraw_notice') }}</p>
            <div class="bg-muted p-4 rounded-lg space-y-2">
                <div class="flex justify-between">
                     <span>{{ t('wallet.network') }}</span>
                    <span class="font-mono text-foreground">{{ selectedNetwork || '-' }}</span>
                </div>
                <div class="flex justify-between">
                    <span>{{ t('wallet.minimum_withdrawal') }}</span>
                    <span class="font-mono text-foreground">{{ coinInfo?.coin.minWithdrawAmount || '-' }} {{ selectedCoin }}</span>
                </div>
                <div class="flex justify-between">
                    <span>{{ t('wallet.fee') }}</span>
                    <span class="font-mono text-foreground">{{ coinInfo ? withdrawFeeDisplay : '-' }} {{ selectedCoin }}</span>
                </div>
                 <div class="flex justify-between">
                    <span>{{ t('wallet.limit_24h') }}</span>
                    <span class="font-mono text-foreground">{{ coinInfo?.coin.maxWithdrawAmount || '-' }} {{ selectedCoin }}</span>
                </div>
            </div>
        </div>
      </div>
    </div>

    <!-- Withdrawal Records -->
    <div class="bg-card border border-border rounded-xl p-6 shadow-sm">
      <div class="flex justify-between items-center mb-4">
        <h3 class="font-bold">{{ t('wallet.records_title') }}</h3>
        <button @click="loadRecords" :disabled="loadingRecords" class="text-primary text-sm font-bold hover:underline disabled:opacity-50 flex items-center gap-1">
          <Icon v-if="loadingRecords" icon="mdi:loading" class="animate-spin" />
          {{ t('wallet.records_refresh') }}
        </button>
      </div>

      <div v-if="loadingRecords && records.length === 0" class="py-8 flex justify-center">
        <Icon icon="mdi:loading" class="animate-spin text-3xl text-primary" />
      </div>

      <div v-else-if="records.length === 0" class="py-8 text-center text-sm text-muted-foreground">
        {{ t('wallet.records_empty') }}
      </div>

      <div v-else class="space-y-3">
        <div v-for="record in records" :key="record.id" class="p-4 bg-muted/30 rounded-lg border border-border space-y-2">
          <div class="flex flex-col md:flex-row justify-between md:items-center gap-2">
            <div class="font-bold font-mono">{{ formatRecordAmount(record.amount) }} {{ record.asset_symbol }}<span v-if="record.network" class="ml-2 text-xs font-sans font-medium text-muted-foreground">{{ record.network }}</span></div>
            <span class="text-xs font-bold px-2 py-1 rounded border self-start md:self-auto" :class="recordStatusClass(record.status)">
              {{ recordStatusLabel(record.status) }}
            </span>
          </div>
          <div class="grid grid-cols-1 md:grid-cols-2 gap-x-6 gap-y-1 text-xs text-muted-foreground">
            <div class="flex justify-between gap-3">
              <span>{{ t('wallet.withdraw_address') }}</span>
              <span class="font-mono text-foreground break-all text-right">{{ record.address }}</span>
            </div>
            <div class="flex justify-between gap-3">
              <span>{{ t('wallet.fee') }}</span>
              <span class="font-mono text-foreground">{{ formatRecordAmount(record.fee) }} {{ record.asset_symbol }}</span>
            </div>
            <div class="flex justify-between gap-3">
              <span>{{ t('wallet.records_time') }}</span>
              <span class="font-mono text-foreground">{{ formatRecordTime(record.created_at) }}</span>
            </div>
            <div v-if="record.tx_hash" class="flex justify-between gap-3">
              <span>{{ t('wallet.records_tx_hash') }}</span>
              <span class="font-mono text-foreground break-all text-right">{{ record.tx_hash }}</span>
            </div>
            <div v-if="record.failure_reason || record.review_reason" class="flex justify-between gap-3 md:col-span-2">
              <span>{{ t('wallet.records_reason') }}</span>
              <span class="text-foreground text-right">{{ record.failure_reason || record.review_reason }}</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { Icon } from '@iconify/vue'
import { calculateWithdrawFee, fetchWithdrawCoins, fetchCoinNetworks, fetchWithdrawRecords, getNetworkInfo, submitWithdraw, type WalletAddress, type CoinNetwork, type WithdrawalRecord } from '@/api/wallet'
import { getTwoFactorStatus, type PaymentPolicy, type TwoFactorStatus } from '@/api/user'
import { useUserStore } from '@/stores/user'
import { useToast } from 'vue-toastification'
import { useI18n } from 'vue-i18n'

const toast = useToast()
const userStore = useUserStore()
const { t } = useI18n()

const supportedCoins = ref<string[]>([])
const selectedCoin = ref<string>('')
const availableNetworks = ref<CoinNetwork[]>([])
const selectedNetwork = ref<string>('')
const coinInfo = ref<WalletAddress | null>(null) // reusing WalletAddress type for coin info
const loadingInfo = ref(false)
const loadingNetworks = ref(false)
const submitting = ref(false)
const twoFactorStatus = ref<TwoFactorStatus | null>(null)
const records = ref<WithdrawalRecord[]>([])
const loadingRecords = ref(false)

const form = ref({
    address: '',
    amount: 0,
    code: '',
    fundPassword: '',
    totpCode: ''
})

const getCoinIcon = (coin: string) => {
    switch(coin) {
        case 'USDT': return 'mdi:currency-usd'
        case 'BTC': return 'mdi:bitcoin'
        case 'ETH': return 'mdi:ethereum'
        default: return 'mdi:currency-usd-circle'
    }
}

const getBalance = (coin: string) => {
    // @ts-ignore
    return userStore.assets[coin] || 0
}

const receiveAmount = computed(() => {
    if (!coinInfo.value) return 0
    const val = form.value.amount - withdrawFee.value
    return val > 0 ? val.toFixed(8) : 0
})

const withdrawFee = computed(() => {
    if (!coinInfo.value) return 0
    return calculateWithdrawFee(form.value.amount, coinInfo.value.coin)
})

const withdrawFeeDisplay = computed(() => {
    const value = withdrawFee.value
    return Number.isFinite(value) ? value.toFixed(8).replace(/\.?0+$/, '') || '0' : '0'
})

const withdrawPolicy = computed(() => twoFactorStatus.value?.payment_policies.withdraw)
const needsFundPassword = computed(() => methodNeedsFundPassword(withdrawPolicy.value))
const needsTwoFactor = computed(() => methodNeedsTwoFactor(withdrawPolicy.value))
const withdrawPolicyLabel = computed(() => paymentPolicyLabel(withdrawPolicy.value))

const isValid = computed(() => {
    if (!coinInfo.value) return false
    const { minWithdrawAmount, maxWithdrawAmount } = coinInfo.value.coin
    const balance = getBalance(selectedCoin.value)

    return !!form.value.address &&
           form.value.amount >= minWithdrawAmount &&
           form.value.amount <= maxWithdrawAmount &&
           form.value.amount <= balance &&
           (!needsFundPassword.value || !!form.value.fundPassword.trim()) &&
           (!needsTwoFactor.value || !!form.value.totpCode.trim()) &&
           (!needsTwoFactor.value || twoFactorStatus.value?.totp_enabled !== false)
})

const loadCoins = async () => {
    try {
        const res = await fetchWithdrawCoins()
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

const loadSecurityPolicy = async () => {
    try {
        const res = await getTwoFactorStatus()
        if (res.code === 0 || res.code === 200) {
            twoFactorStatus.value = res.data
        }
    } catch (e) {
        console.error(e)
        toast.error(t('wallet.load_security_failed'))
    }
}

const selectCoin = async (coin: string) => {
    selectedCoin.value = coin
    selectedNetwork.value = ''
    availableNetworks.value = []
    coinInfo.value = null
    form.value.address = ''
    form.value.amount = 0

    loadingNetworks.value = true

    try {
        const res = await fetchCoinNetworks(coin, 'withdraw')
        if (res.data.code === 0 && res.data.data.length > 0) {
             availableNetworks.value = res.data.data.filter(n => n.withdrawEnabled)
             if (availableNetworks.value.length > 0) {
                 selectNetwork(availableNetworks.value[0].name)
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

const selectNetwork = async (network: string) => {
    selectedNetwork.value = network
    loadingInfo.value = true
    coinInfo.value = null
    form.value.amount = 0 // Reset amount on network change as fee might change

    try {
        const res = await getNetworkInfo(selectedCoin.value, network, 'withdraw')
        if (res.data.code === 0) {
            coinInfo.value = res.data.data
        }
    } catch (e) {
        console.error(e)
        toast.error(t('wallet.network_info_failed'))
    } finally {
        loadingInfo.value = false
    }
}

const loadRecords = async () => {
    loadingRecords.value = true
    try {
        const res = await fetchWithdrawRecords()
        if (res.data.code === 0) {
            records.value = res.data.data
        }
    } catch (e) {
        console.error(e)
        toast.error(t('wallet.records_load_failed'))
    } finally {
        loadingRecords.value = false
    }
}

const recordStatusLabel = (status: string) => {
    switch (status) {
        case 'pending_review': return t('wallet.status_pending_review')
        case 'approved': return t('wallet.status_approved')
        case 'broadcasting': return t('wallet.status_broadcasting')
        case 'broadcasted': return t('wallet.status_broadcasted')
        case 'confirmed': return t('wallet.status_confirmed')
        case 'manual_review': return t('wallet.status_manual_review')
        case 'rejected': return t('wallet.status_rejected')
        case 'failed': return t('wallet.status_failed')
        default: return status
    }
}

const recordStatusClass = (status: string) => {
    switch (status) {
        case 'confirmed': return 'text-up border-up/30 bg-up/10'
        case 'rejected':
        case 'failed': return 'text-destructive border-destructive/30 bg-destructive/10'
        case 'approved':
        case 'broadcasting':
        case 'broadcasted': return 'text-primary border-primary/30 bg-primary/10'
        default: return 'text-yellow-500 border-yellow-500/30 bg-yellow-500/10'
    }
}

const formatRecordTime = (ts: number) => new Date(ts).toLocaleString()

// 后端 DECIMAL(36,18) 序列化为定长字符串，去掉无意义的尾随零但保留全部有效位。
const formatRecordAmount = (value: string) => {
    if (!value.includes('.')) return value
    const trimmed = value.replace(/0+$/, '').replace(/\.$/, '')
    return trimmed === '' || trimmed === '-' ? '0' : trimmed
}

const setMaxAmount = () => {
    if (!coinInfo.value) return
    const balance = getBalance(selectedCoin.value)
    form.value.amount = balance
}

const handleSubmit = async () => {
    if (!isValid.value || !coinInfo.value) return

    submitting.value = true
    try {
        const res = await submitWithdraw({
            unit: selectedCoin.value,
            network: selectedNetwork.value,
            address: form.value.address,
            amount: form.value.amount,
            fee: withdrawFee.value,
            code: form.value.code,
            fundPassword: form.value.fundPassword,
            totpCode: form.value.totpCode
        })

        if (res.data.code === 0) {
            toast.success(t('wallet.withdraw_success'))
            // Reset form
            form.value.amount = 0
            form.value.address = ''
            form.value.fundPassword = ''
            form.value.totpCode = ''
            loadRecords()
        } else {
            toast.error(res.data.message || t('wallet.withdraw_failed'))
        }
    } catch (e) {
        console.error(e)
        toast.error(t('wallet.withdraw_submit_failed'))
    } finally {
        submitting.value = false
    }
}

function methodNeedsFundPassword(policy?: PaymentPolicy) {
    return !!policy?.enabled && (policy.method === 'fund_password' || policy.method === 'fund_password_and_two_factor')
}

function methodNeedsTwoFactor(policy?: PaymentPolicy) {
    return !!policy?.enabled && (policy.method === 'two_factor' || policy.method === 'fund_password_and_two_factor')
}

function paymentPolicyLabel(policy?: PaymentPolicy) {
    if (!policy?.enabled) return t('wallet.policy_none')
    if (policy.method === 'fund_password') return t('wallet.policy_fund_password')
    if (policy.method === 'two_factor') return t('wallet.policy_two_factor')
    return t('wallet.policy_fund_password_two_factor')
}

onMounted(() => {
    loadCoins()
    loadSecurityPolicy()
    loadRecords()
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
