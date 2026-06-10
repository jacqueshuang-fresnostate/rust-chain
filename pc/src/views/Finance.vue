<template>
  <div class="min-h-full p-4 md:p-8 max-w-7xl mx-auto space-y-8">
    <!-- Header Section -->
    <div class="flex flex-col md:flex-row justify-between items-end gap-4 border-b border-border pb-6">
      <div>
        <h1 class="text-4xl font-black text-transparent bg-clip-text bg-gradient-to-r from-primary to-neon-blue mb-2 tracking-tighter">
          {{ $t('ai_finance.title') }}
        </h1>
        <p class="text-muted-foreground text-lg">
          {{ $t('ai_finance.subtitle') }}
        </p>
      </div>

      <div v-if="userStore.isLoggedIn" class="flex gap-4">
        <button @click="$router.push('/user/finance-orders')" class="flex items-center gap-2 px-6 py-3 bg-muted text-foreground font-bold rounded-lg hover:bg-muted/80 transition-all shadow-sm">
            <span class="i-mdi-format-list-bulleted"></span>
            {{ $t('ai_finance.my_orders') }}
        </button>
      </div>
    </div>

    <!-- User Dashboard (If Logged In) -->
    <div v-if="userStore.isLoggedIn" class="grid grid-cols-1 md:grid-cols-3 gap-6">
        <div class="bg-card border border-border rounded-xl p-6 relative overflow-hidden group">
            <div class="absolute right-0 top-0 p-4 opacity-10 group-hover:opacity-20 transition-opacity">
                <span class="i-mdi-wallet w-24 h-24 text-primary"></span>
            </div>
            <div class="text-sm text-muted-foreground mb-2">{{ $t('ai_finance.total_earnings') }}</div>
            <div class="text-3xl font-mono font-bold text-up">+{{ formatNumber(statistic.earnNum) }} USDT</div>
        </div>
        <div class="bg-card border border-border rounded-xl p-6 relative overflow-hidden group">
            <div class="absolute right-0 top-0 p-4 opacity-10 group-hover:opacity-20 transition-opacity">
                <span class="i-mdi-chart-line w-24 h-24 text-primary"></span>
            </div>
            <div class="text-sm text-muted-foreground mb-2">{{ $t('ai_finance.assets_hosting') }}</div>
            <div class="text-3xl font-mono font-bold text-primary">{{ formatNumber(hostingCount) }} USDT</div>
        </div>
        <div class="bg-card border border-border rounded-xl p-6 relative overflow-hidden group">
            <div class="absolute right-0 top-0 p-4 opacity-10 group-hover:opacity-20 transition-opacity">
                <span class="i-mdi-lightning-bolt w-24 h-24 text-primary"></span>
            </div>
            <div class="text-sm text-muted-foreground mb-2">{{ $t('ai_finance.remaining_quota') }}</div>
            <div class="text-3xl font-mono font-bold">{{ statistic.num }} {{ $t('ai_finance.times') }}</div>
        </div>
    </div>

    <!-- Guest Banner (If Not Logged In) -->
    <div v-else class="relative overflow-hidden rounded-xl border border-primary/20 bg-background/50 backdrop-blur-md p-10 text-center shadow-[0_0_40px_rgba(var(--primary),0.1)] group">
        <!-- Decorative Elements -->
        <div class="absolute -top-10 -left-10 w-40 h-40 bg-primary/20 rounded-full blur-3xl group-hover:bg-primary/30 transition-all duration-700"></div>
        <div class="absolute -bottom-10 -right-10 w-40 h-40 bg-neon-blue/20 rounded-full blur-3xl group-hover:bg-neon-blue/30 transition-all duration-700"></div>

        <div class="relative z-10 flex flex-col items-center gap-6">
            <div class="inline-flex justify-center items-center w-20 h-20 rounded-full bg-gradient-to-br from-primary/20  mb-2 shadow-[0_0_20px_rgba(var(--primary),0.2)]  overflow-hidden">
<!--                <span class="i-mdi-shield-lock-outline text-4xl text-primary animate-pulse"></span>-->
              <img src="@/assets/logo/zf_logo.png" alt="">
            </div>

            <div class="space-y-2">
                <h2 class="text-3xl font-black text-transparent bg-clip-text bg-gradient-to-r from-white to-white/70">
                    {{ $t('ai_finance.guest_title') }}
                </h2>
                <p class="text-muted-foreground max-w-lg mx-auto text-lg leading-relaxed">
                    {{ $t('ai_finance.guest_desc') }}
                </p>
            </div>

            <div class="flex flex-col sm:flex-row gap-4 w-full sm:w-auto mt-4">
                <button @click="$router.push('/login')" class="px-10 py-3.5 bg-primary text-primary-foreground font-bold rounded-lg hover:bg-primary/90 transition-all hover:shadow-[0_0_25px_rgba(var(--primary),0.5)] active:scale-95 flex items-center justify-center gap-2">
                    <span class="i-mdi-login text-xl"></span>
                    <span>{{ $t('nav.login') }}</span>
                </button>
                <button @click="$router.push('/register')" class="px-10 py-3.5 bg-background/50 border border-primary/50 text-foreground font-bold rounded-lg hover:bg-primary/10 transition-all hover:border-primary active:scale-95 flex items-center justify-center gap-2 backdrop-blur-sm">
                   <span class="i-mdi-account-plus text-xl"></span>
                   <span>{{ $t('nav.signup') }}</span>
                </button>
            </div>

            <div class="flex items-center gap-8 mt-4 text-sm text-muted-foreground/60 font-mono">
                 <div class="flex items-center gap-2">
                     <span class="i-mdi-check-circle text-primary"></span>
                     <span>{{ $t('ai_finance.secure_assets') }}</span>
                 </div>
                 <div class="flex items-center gap-2">
                     <span class="i-mdi-check-circle text-primary"></span>
                     <span>{{ $t('ai_finance.ai_optimized') }}</span>
                 </div>
                 <div class="flex items-center gap-2">
                     <span class="i-mdi-check-circle text-primary"></span>
                     <span>{{ $t('ai_finance.instant_withdrawal') }}</span>
                 </div>
            </div>
        </div>
    </div>

    <!-- Product List -->
    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
      <div v-for="product in products" :key="product.id" class="bg-card border border-border rounded-xl overflow-hidden hover:border-primary/50 transition-all hover:shadow-[0_0_20px_rgba(var(--primary),0.1)] flex flex-col">
          <div class="p-6 flex-1">
              <div class="flex items-center gap-4 mb-6">
                  <img :src="product.iconImageUrl" class="w-12 h-12 rounded-full bg-muted/20" />
                  <div>
                      <h3 class="font-bold text-lg">{{ product.cycle }} {{ $t('ai_finance.days') }}</h3>
                      <div class="text-xs text-muted-foreground bg-muted px-2 py-0.5 rounded inline-block">{{ product.acceptUnit }}</div>
                  </div>
              </div>

              <div class="grid grid-cols-2 gap-4 mb-6">
                  <div>
                      <div class="text-xs text-muted-foreground mb-1"> {{ $t('ai_finance.roi') }}</div>
                      <div class="text-xl font-bold text-up font-mono">{{ (product.minDaysProfit * 100).toFixed(2) }}%</div>
                  </div>
                  <div>
                      <div class="text-xs text-muted-foreground mb-1">{{ $t('ai_finance.cycle') }}</div>
                      <div class="text-xl font-bold font-mono">{{ product.cycle }} {{ $t('ai_finance.days') }}</div>
                  </div>
              </div>

              <div class="space-y-2 text-sm text-muted-foreground font-mono bg-muted/20 p-3 rounded mb-4">
                  <div class="flex justify-between">
                      <span>{{ $t('ai_finance.min_invest') }}:</span>
                      <span class="text-foreground">{{ formatNumber(product.minLimitAmount) }}</span>
                  </div>
                  <div class="flex justify-between">
                      <span>{{ $t('ai_finance.max_invest') }}:</span>
                      <span class="text-foreground">{{ formatNumber(product.maxLimitAmount) }}</span>
                  </div>
              </div>
          </div>

          <div class="p-4 border-t border-border bg-muted/5">
              <button v-if="product.step === 1"
                      @click="openInvestModal(product)"
                      class="w-full py-2 bg-primary text-primary-foreground font-bold rounded hover:bg-primary/90 transition-colors text-sm shadow-[0_0_15px_rgba(var(--primary),0.4)]">
                  {{ $t('ai_finance.subscribe_now') }}
              </button>
              <button v-else class="w-full py-2 bg-muted text-muted-foreground font-bold rounded cursor-not-allowed text-sm">
                  {{ product.step === 0 ? $t('ai_finance.upcoming') : $t('ai_finance.ended') }}
              </button>
          </div>
      </div>
    </div>

    <!-- Investment Modal -->
    <div v-if="showModal" class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm transition-opacity">
      <div class="bg-card border border-primary/50 w-full max-w-md p-0 rounded-xl shadow-[0_0_50px_rgba(var(--primary),0.2)] overflow-hidden animate-in fade-in zoom-in-95 duration-200">
         <div class="bg-muted/30 p-4 border-b border-border flex justify-between items-center">
             <h3 class="text-lg font-bold">{{ $t('ai_finance.modal_title') }}</h3>
             <button @click="showModal = false" class="text-muted-foreground hover:text-foreground">
                 <span class="i-mdi-close text-xl"></span>
             </button>
         </div>

         <div class="p-6 space-y-6" v-if="selectedProduct">
             <!-- Product Summary -->
             <div class="flex items-center gap-4 bg-muted/10 p-4 rounded-lg border border-border">
                 <img :src="selectedProduct.iconImageUrl" class="w-10 h-10 rounded-full" />
                 <div>
                     <div class="font-bold">{{ selectedProduct.cycle }} {{ $t('ai_finance.days') }}</div>
                     <div class="text-xs text-muted-foreground">{{ $t('ai_finance.roi') }}: <span class="text-up">{{ (selectedProduct.minDaysProfit * 100).toFixed(2) }}%</span> </div>
                 </div>
             </div>

             <!-- Input -->
             <div class="space-y-2">
                 <div class="flex justify-between text-sm">
                     <span class="text-muted-foreground">{{ $t('ai_finance.invest_amount') }}</span>
                     <span class="text-xs text-muted-foreground">{{ $t('ai_finance.range') }}: {{ formatNumber(selectedProduct.minLimitAmount) }} - {{ formatNumber(selectedProduct.maxLimitAmount) }}</span>
                 </div>
                 <div class="relative">
                     <input type="number"
                            v-model="investAmount"
                            :placeholder="$t('ai_finance.enter_amount')"
                            class="w-full bg-background border border-border rounded-lg px-4 py-3 text-lg outline-none focus:border-primary focus:ring-1 focus:ring-primary font-mono"
                     />
                     <span class="absolute right-4 top-1/2 -translate-y-1/2 text-sm text-muted-foreground">{{ selectedProduct.acceptUnit }}</span>
                 </div>
             </div>

             <!-- Action Buttons -->
             <div class="flex gap-3 pt-2">
                 <button @click="showModal = false" class="flex-1 py-3 bg-muted hover:bg-muted/80 text-foreground font-bold rounded-lg transition-colors text-sm">
                     {{ $t('ai_finance.cancel') }}
                 </button>
                 <button @click="confirmInvest"
                         :disabled="investing"
                         class="flex-1 py-3 bg-primary text-primary-foreground font-bold rounded-lg hover:bg-primary/90 transition-colors text-sm disabled:opacity-50 flex justify-center items-center gap-2 shadow-[0_0_20px_rgba(var(--primary),0.3)]">
                     <span v-if="investing" class="i-mdi-loading animate-spin text-lg"></span>
                     <span>{{ investing ? $t('ai_finance.processing') : $t('ai_finance.confirm_sub') }}</span>
                 </button>
             </div>
         </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useUserStore } from '@/stores/user'
import { fetchFinanceList, fetchFinanceStatistic, fetchFinanceCount, investFinance, type FinanceProduct } from '@/api/finance'
import { useToast } from 'vue-toastification'
import { useI18n } from 'vue-i18n'
import numeral from 'numeral'

const { t } = useI18n()
const userStore = useUserStore()
const toast = useToast()

const products = ref<FinanceProduct[]>([])
const statistic = ref({ earnNum: 0, num: 0 })
const hostingCount = ref(0)

// Modal State
const showModal = ref(false)
const selectedProduct = ref<FinanceProduct | null>(null)
const investAmount = ref<string | number>('')
const investing = ref(false)

const formatNumber = (val: number) => {
    return numeral(val).format('0,0.[00]')
}

const loadData = async () => {
    try {
        const res = await fetchFinanceList()
        if (res.data && res.data.data) {
            products.value = res.data.data
        }
    } catch (e) {
        console.error("Failed to load finance products", e)
    }

    if (userStore.isLoggedIn) {
        try {
            const statRes = await fetchFinanceStatistic()
            if (statRes.data && statRes.data.data) {
                statistic.value = statRes.data.data
            }
            const countRes = await fetchFinanceCount()
            if (countRes.data) {
                hostingCount.value = Number(countRes.data.data)
            }
        } catch (e) {
            console.error("Failed to load user stats", e)
        }
    }
}

const openInvestModal = (product: FinanceProduct) => {
    if (!userStore.isLoggedIn) {
        toast.warning("Please login first")
        return
    }
    selectedProduct.value = product
    investAmount.value = ''
    showModal.value = true
}

const confirmInvest = async () => {
    if (!selectedProduct.value) return

    const amount = Number(investAmount.value)
    if (!amount || amount <= 0) {
        toast.warning("Please enter a valid amount")
        return
    }

    if (amount < selectedProduct.value.minLimitAmount) {
        toast.warning(`Minimum investment is ${selectedProduct.value.minLimitAmount}`)
        return
    }

    if (amount > selectedProduct.value.maxLimitAmount) {
        toast.warning(`Maximum investment is ${selectedProduct.value.maxLimitAmount}`)
        return
    }

    investing.value = true
    try {
        await investFinance({ id: selectedProduct.value.id, amount })
        toast.success(t('ai_finance.invest_success'))
        showModal.value = false
        loadData() // Refresh stats
    } catch (e) {
        toast.error(t('ai_finance.invest_failed'))
        console.error(e)
    } finally {
        investing.value = false
    }
}

onMounted(() => {
    loadData()
})
</script>
