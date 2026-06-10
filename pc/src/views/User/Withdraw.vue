<template>
  <div class="space-y-6">
    <h2 class="text-2xl font-bold flex items-center gap-2">
      <Icon icon="mdi:arrow-up-circle-outline" class="text-primary" />
      Withdraw
    </h2>

    <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
      <!-- Withdraw Form -->
      <div class="bg-card border border-border rounded-xl p-6 shadow-sm space-y-6">
        <div>
          <label class="block text-sm font-medium text-muted-foreground mb-2">Select Coin</label>
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
             <label class="block text-sm font-medium text-muted-foreground mb-2">Select Network</label>
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
                 <span class="text-sm text-muted-foreground">Available Balance</span>
                 <span class="font-bold font-mono text-lg">{{ getBalance(selectedCoin) }} {{ selectedCoin }}</span>
             </div>

             <!-- Address Input -->
             <div>
                 <label class="block text-xs font-medium text-muted-foreground mb-1">Withdraw Address</label>
                 <div class="relative">
                    <input
                        v-model="form.address"
                        type="text"
                        placeholder="Enter wallet address"
                        class="w-full bg-background border border-border rounded-lg px-4 py-3 text-sm focus:border-primary focus:ring-1 focus:ring-primary outline-none transition-all"
                    />
                 </div>
             </div>

             <!-- Amount Input -->
             <div>
                 <label class="block text-xs font-medium text-muted-foreground mb-1">Amount</label>
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
                        MAX
                    </button>
                 </div>
                 <div class="flex justify-between text-xs text-muted-foreground mt-1">
                     <span>Fee: {{ coinInfo.coin.withdrawFee }} {{ selectedCoin }}</span>
                     <span>Min: {{ coinInfo.coin.minWithdrawAmount }}</span>
                 </div>
             </div>

             <!-- Summary -->
             <div class="border-t border-border pt-4">
                 <div class="flex justify-between items-center mb-4">
                     <span class="text-sm font-medium">Receive Amount</span>
                     <span class="text-xl font-bold text-primary">{{ receiveAmount }} {{ selectedCoin }}</span>
                 </div>
                 <button
                    @click="handleSubmit"
                    :disabled="submitting || !isValid"
                    class="w-full py-3 bg-primary text-primary-foreground font-bold rounded-lg hover:bg-primary/90 transition-all disabled:opacity-50 disabled:cursor-not-allowed flex justify-center items-center gap-2"
                 >
                    <Icon v-if="submitting" icon="mdi:loading" class="animate-spin" />
                    {{ submitting ? 'Processing...' : 'Withdraw' }}
                 </button>
             </div>
        </div>
      </div>

      <!-- Instructions -->
      <div class="bg-card border border-border rounded-xl p-6 shadow-sm">
        <h3 class="font-bold mb-4">Important Information</h3>
        <div class="space-y-4 text-sm text-muted-foreground">
            <p>Please double-check the destination address and selected network. Withdrawals cannot be cancelled once processed.</p>
            <div class="bg-muted p-4 rounded-lg space-y-2">
                <div class="flex justify-between">
                     <span>Network</span>
                    <span class="font-mono text-foreground">{{ selectedNetwork || '-' }}</span>
                </div>
                <div class="flex justify-between">
                    <span>Minimum Withdrawal</span>
                    <span class="font-mono text-foreground">{{ coinInfo?.coin.minWithdrawAmount || '-' }} {{ selectedCoin }}</span>
                </div>
                <div class="flex justify-between">
                    <span>Fee</span>
                    <span class="font-mono text-foreground">{{ coinInfo?.coin.withdrawFee || '-' }} {{ selectedCoin }}</span>
                </div>
                 <div class="flex justify-between">
                    <span>24h Limit</span>
                    <span class="font-mono text-foreground">{{ coinInfo?.coin.maxWithdrawAmount || '-' }} {{ selectedCoin }}</span>
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
import { fetchSupportedCoins, fetchCoinNetworks, getDepositAddress, submitWithdraw, type WalletAddress, type CoinNetwork } from '@/api/wallet'
import { useUserStore } from '@/stores/user'
import { useToast } from 'vue-toastification'

const toast = useToast()
const userStore = useUserStore()

const supportedCoins = ref<string[]>([])
const selectedCoin = ref<string>('')
const availableNetworks = ref<CoinNetwork[]>([])
const selectedNetwork = ref<string>('')
const coinInfo = ref<WalletAddress | null>(null) // reusing WalletAddress type for coin info
const loadingInfo = ref(false)
const loadingNetworks = ref(false)
const submitting = ref(false)

const form = ref({
    address: '',
    amount: 0,
    code: ''
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
    const val = form.value.amount - coinInfo.value.coin.withdrawFee
    return val > 0 ? val.toFixed(8) : 0
})

const isValid = computed(() => {
    if (!coinInfo.value) return false
    const { minWithdrawAmount, maxWithdrawAmount } = coinInfo.value.coin
    const balance = getBalance(selectedCoin.value)

    return form.value.address &&
           form.value.amount >= minWithdrawAmount &&
           form.value.amount <= maxWithdrawAmount &&
           form.value.amount <= balance
})

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
        toast.error('Failed to load coins')
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
        const res = await fetchCoinNetworks(coin)
        if (res.data.code === 0 && res.data.data.length > 0) {
             availableNetworks.value = res.data.data.filter(n => n.withdrawEnabled)
             if (availableNetworks.value.length > 0) {
                 selectNetwork(availableNetworks.value[0].name)
             }
        } else {
             toast.warning('No networks available for this coin')
        }
    } catch (e) {
        console.error(e)
        toast.error('Failed to load networks')
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
        // We use getDepositAddress to get coin info (limits, fees) for specific network
        const res = await getDepositAddress(selectedCoin.value, network)
        if (res.data.code === 0) {
            coinInfo.value = res.data.data
        }
    } catch (e) {
        console.error(e)
        toast.error('Failed to get network info')
    } finally {
        loadingInfo.value = false
    }
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
            fee: coinInfo.value.coin.withdrawFee,
            code: form.value.code
        })

        if (res.data.code === 0) {
            toast.success('Withdrawal submitted successfully')
            // Reset form
            form.value.amount = 0
            form.value.address = ''
        } else {
            toast.error(res.data.message || 'Withdrawal failed')
        }
    } catch (e) {
        console.error(e)
        toast.error('Error submitting withdrawal')
    } finally {
        submitting.value = false
    }
}

onMounted(() => {
    loadCoins()
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
