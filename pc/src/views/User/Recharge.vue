<template>
  <div class="space-y-6">
    <h2 class="text-2xl font-bold flex items-center gap-2">
      <Icon icon="mdi:arrow-down-circle-outline" class="text-primary" />
      Deposit
    </h2>

    <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
      <!-- Deposit Form -->
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

        <div v-if="loading" class="py-8 flex justify-center">
            <Icon icon="mdi:loading" class="animate-spin text-3xl text-primary" />
        </div>

        <div v-else-if="walletData" class="space-y-6 animate-fade-in">
            <div class="p-4 bg-muted/30 rounded-lg border border-border">
                <div class="text-xs text-muted-foreground mb-1">Deposit Address</div>
                <div class="flex items-center gap-2">
                    <code class="flex-1 bg-background p-3 rounded border border-border font-mono text-sm break-all">
                        {{ walletData.address }}
                    </code>
                    <button @click="copyAddress" class="p-3 bg-primary/10 text-primary rounded hover:bg-primary/20 transition-colors" title="Copy Address">
                        <Icon icon="mdi:content-copy" />
                    </button>
                </div>
            </div>

            <div class="flex flex-col items-center justify-center p-6 border border-dashed border-border rounded-lg bg-background">
                <div class="w-48 h-48 bg-white p-2 rounded mb-4">
                     <img :src="`https://api.qrserver.com/v1/create-qr-code/?size=200x200&data=${walletData.address}`" alt="QR Code" class="w-full h-full object-contain" />
                </div>
                <p class="text-sm text-muted-foreground text-center">Scan QR code to deposit {{ selectedCoin }} ({{ selectedNetwork }})</p>
            </div>

            <div class="space-y-2 text-sm text-muted-foreground bg-yellow-500/5 p-4 rounded border border-yellow-500/20">
                <div class="flex items-start gap-2">
                    <Icon icon="mdi:alert-circle-outline" class="text-yellow-500 mt-0.5" />
                    <p>Send only <span class="font-bold text-foreground">{{ selectedCoin }}</span> to this address via <span class="font-bold text-foreground">{{ selectedNetwork }}</span> network. Sending any other coin or using wrong network may result in permanent loss.</p>
                </div>
                <div class="flex items-start gap-2">
                    <Icon icon="mdi:information-outline" class="text-blue-500 mt-0.5" />
                    <p>Minimum deposit amount: <span class="font-bold text-foreground">{{ walletData.coin.minRechargeAmount }} {{ walletData.unit || selectedCoin }}</span></p>
                </div>
            </div>
        </div>
      </div>

      <!-- Recent Deposits (Placeholder or Future Feature) -->
      <div class="bg-card border border-border rounded-xl p-6 shadow-sm">
        <h3 class="font-bold mb-4">Tips</h3>
        <ul class="list-disc pl-5 space-y-2 text-sm text-muted-foreground">
            <li>Deposits will be credited after network confirmations.</li>
            <li>Make sure your computer and browser are secure.</li>
            <li>Double check the address before sending.</li>
        </ul>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { Icon } from '@iconify/vue'
import { fetchSupportedCoins, fetchCoinNetworks, getDepositAddress, type WalletAddress, type CoinNetwork } from '@/api/wallet'
import { useToast } from 'vue-toastification'

const toast = useToast()
const supportedCoins = ref<string[]>([])
const selectedCoin = ref<string>('')
const availableNetworks = ref<CoinNetwork[]>([])
const selectedNetwork = ref<string>('')
const walletData = ref<WalletAddress | null>(null)

const loading = ref(false)
const loadingNetworks = ref(false)

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
        toast.error('Failed to load supported coins')
    }
}

const selectCoin = async (coin: string) => {
    selectedCoin.value = coin
    selectedNetwork.value = ''
    walletData.value = null
    availableNetworks.value = []

    loadingNetworks.value = true
    try {
        const res = await fetchCoinNetworks(coin)
        if (res.data.code === 0 && res.data.data.length > 0) {
            availableNetworks.value = res.data.data.filter(n => n.depositEnabled)
            if (availableNetworks.value.length > 0) {
                // Auto select first network
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
    loading.value = true
    walletData.value = null
    try {
        const res = await getDepositAddress(selectedCoin.value, network)
        if (res.data.code === 0) {
            walletData.value = res.data.data
        } else {
            toast.error('Failed to generate address')
        }
    } catch (e) {
        console.error(e)
        toast.error('Error fetching deposit address')
    } finally {
        loading.value = false
    }
}

const copyAddress = () => {
    if (walletData.value?.address) {
        navigator.clipboard.writeText(walletData.value.address)
        toast.success('Address copied to clipboard')
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
