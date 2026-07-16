<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Gauge, X } from 'lucide-vue-next'
import AssetMark from '@/components/AssetMark.vue'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchSecondsOrders, fetchSecondsProducts, openSecondsOrder, type SecondsCycle, type SecondsOrder, type SecondsProduct } from '@/api/seconds'
import { fetchWalletAccounts } from '@/api/wallet'
import { formatAmount, formatDateTime } from '@/core/format'
import { useSessionStore } from '@/stores/session'
import type { WalletAccount } from '@/core/types'

const session = useSessionStore()
const { t } = useI18n()
const products = ref<SecondsProduct[]>([])
const orders = ref<SecondsOrder[]>([])
const accounts = ref<WalletAccount[]>([])
const selected = ref<SecondsProduct | null>(null)
const selectedCycleId = ref(0)
const direction = ref<'up' | 'down'>('up')
const amount = ref('')
const loading = ref(false)
const submitting = ref(false)
const error = ref('')
const success = ref('')

const cycle = computed<SecondsCycle | undefined>(() => selected.value?.cycles.find((item) => item.id === selectedCycleId.value) || selected.value?.cycles[0])
const account = computed(() => accounts.value.find((item) => item.assetId === selected.value?.stakeAssetId))
const amountNumber = computed(() => Number(amount.value || 0))
const valid = computed(() => Boolean(cycle.value && Number.isFinite(amountNumber.value) && amountNumber.value >= cycle.value.minStake && (!cycle.value.maxStake || amountNumber.value <= cycle.value.maxStake) && amountNumber.value <= (account.value?.available || 0)))

async function load(): Promise<void> {
  if (!session.isAuthenticated) return
  loading.value = true
  error.value = ''
  try {
    const [nextProducts, nextOrders, nextAccounts] = await Promise.all([fetchSecondsProducts(), fetchSecondsOrders(), fetchWalletAccounts()])
    products.value = nextProducts
    orders.value = nextOrders
    accounts.value = nextAccounts
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('seconds.loadFailed'))
  } finally {
    loading.value = false
  }
}

function openOrder(product: SecondsProduct): void {
  selected.value = product
  selectedCycleId.value = product.cycles[0]?.id || 0
  direction.value = 'up'
  amount.value = String(product.cycles[0]?.minStake || '')
  error.value = ''
}

async function submit(): Promise<void> {
  if (!selected.value || !cycle.value || !valid.value) {
    error.value = t('seconds.invalidAmount')
    return
  }
  submitting.value = true
  error.value = ''
  try {
    await openSecondsOrder({ productId: selected.value.id, durationSeconds: cycle.value.durationSeconds, direction: direction.value, stakeAmount: amountNumber.value })
    selected.value = null
    success.value = t('seconds.created')
    await load()
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('seconds.orderFailed'))
  } finally {
    submitting.value = false
  }
}

function statusLabel(status: string): string {
  const keys: Record<string, string> = {
    pending: 'seconds.statusPending',
    active: 'seconds.statusActive',
    won: 'seconds.statusWon',
    lost: 'seconds.statusLost',
    settled: 'seconds.statusSettled',
    cancelled: 'seconds.statusCancelled',
    canceled: 'seconds.statusCancelled',
  }
  const key = keys[status.toLowerCase()]
  return key ? t(key) : status
}

onMounted(() => { void load() })
</script>

<template>
  <main class="page page--plain seconds-page"><PageHeader :title="t('seconds.title')" /><div class="page-content"><LoginRequiredState v-if="!session.isAuthenticated" :description="t('seconds.loginDescription')" /><template v-else><p v-if="error" class="error-message">{{ error }}</p><p v-if="success" class="success-message">{{ success }}</p><p v-if="loading" class="empty-state">{{ t('seconds.loading') }}</p><template v-else><section class="seconds-intro"><Gauge :size="25" /><div><strong>{{ t('seconds.title') }}</strong><p>{{ t('seconds.introDescription') }}</p></div></section><div class="seconds-list"><button v-for="product in products" :key="product.id" type="button" @click="openOrder(product)"><AssetMark :symbol="product.symbol.split(/[\/_-]/)[0] || product.symbol" :size="41" /><div><strong>{{ product.symbol }}</strong><small>{{ t('seconds.settlementSummary', { asset: product.stakeAssetSymbol, count: product.cycles.length }) }}</small></div><span><b>{{ t('seconds.highest', { rate: Math.max(...product.cycles.map((item) => item.payoutRate * 100)).toFixed(0) }) }}</b><small>{{ t('seconds.payoutRate') }}</small></span></button></div><p v-if="!products.length" class="empty-state">{{ t('seconds.noProducts') }}</p><section class="seconds-orders"><div class="section-heading"><span>{{ t('seconds.myOrders') }}</span></div><article v-for="order in orders" :key="order.id"><div><strong>{{ order.symbol }} · {{ t(order.direction === 'up' ? 'seconds.bullish' : 'seconds.bearish') }}</strong><small>{{ formatDateTime(order.createdAt) }} · {{ t('seconds.duration', { seconds: order.durationSeconds }) }}</small></div><span><b>{{ formatAmount(order.stakeAmount) }} {{ order.stakeAssetSymbol }}</b><small>{{ statusLabel(order.status) }}</small></span></article><p v-if="!orders.length" class="empty-state">{{ t('seconds.noOrders') }}</p></section></template></template></div><div v-if="selected" class="seconds-mask" @click.self="selected = null"><form class="seconds-dialog" role="dialog" aria-modal="true" @submit.prevent="submit"><header><div><strong>{{ selected.symbol }}</strong><small>{{ t('seconds.settledIn', { asset: selected.stakeAssetSymbol }) }}</small></div><button class="icon-button" type="button" :aria-label="t('common.close')" @click="selected = null"><X :size="21" /></button></header><div class="direction-toggle" :aria-label="t('seconds.direction')"><button type="button" :class="{ 'is-up': direction === 'up' }" @click="direction = 'up'">{{ t('seconds.bullish') }}</button><button type="button" :class="{ 'is-down': direction === 'down' }" @click="direction = 'down'">{{ t('seconds.bearish') }}</button></div><label><span>{{ t('seconds.term') }}</span><select v-model="selectedCycleId"><option v-for="item in selected.cycles" :key="item.id" :value="item.id">{{ t('seconds.cycleOption', { seconds: item.durationSeconds, rate: (item.payoutRate * 100).toFixed(2) }) }}</option></select></label><label><span>{{ t('seconds.stakeAmount') }}</span><div class="seconds-amount"><input v-model="amount" inputmode="decimal" /><b>{{ selected.stakeAssetSymbol }}</b></div></label><p>{{ t('seconds.balanceMinimum', { available: formatAmount(account?.available), asset: selected.stakeAssetSymbol, minimum: formatAmount(cycle?.minStake) }) }}</p><button class="button button--primary button--full" type="submit" :disabled="submitting">{{ submitting ? t('common.submitting') : t('seconds.confirmOrder') }}</button></form></div></main>
</template>

<style scoped>
    .seconds-page .page-content { display: grid; gap: 18px; padding-bottom: 42px; padding-top: 16px; }.seconds-intro { align-items: center; background: #f7edff; border: 1px solid #e7d3f9; border-radius: var(--radius); color: #9a4ec2; display: flex; gap: 11px; padding: 15px; }.seconds-intro div { display: grid; gap: 4px; }.seconds-intro strong { color: var(--ink); font-size: 17px; }.seconds-intro p { color: var(--muted-strong); font-size: 12px; line-height: 1.4; margin: 0; }.seconds-list { display: grid; gap: 10px; }.seconds-list button { align-items: center; background: white; border: 1px solid var(--line); border-radius: var(--radius); box-shadow: var(--shadow-soft); display: grid; gap: 12px; grid-template-columns: 41px minmax(0, 1fr) auto; min-height: 79px; padding: 12px; text-align: left; width: 100%; }.seconds-list div,.seconds-list > span { display: grid; gap: 5px; }.seconds-list strong { font-size: 15px; }.seconds-list small { color: var(--muted); font-size: 11px; }.seconds-list > span { text-align: right; }.seconds-list > span b { color: #9a4ec2; font-size: 13px; }.seconds-orders { border-top: 1px solid var(--line); }.seconds-orders .section-heading { margin-top: 20px; }.seconds-orders article { align-items: center; border-bottom: 1px solid var(--line); display: flex; justify-content: space-between; min-height: 62px; }.seconds-orders article div,.seconds-orders article > span { display: grid; gap: 5px; }.seconds-orders strong,.seconds-orders b { font-size: 13px; }.seconds-orders small { color: var(--muted); font-size: 11px; }.seconds-orders article > span { text-align: right; }.seconds-mask { align-items: flex-end; background: rgb(15 23 42 / 42%); display: flex; inset: 0; justify-content: center; padding: 16px 16px calc(16px + env(safe-area-inset-bottom)); position: fixed; z-index: 60; }.seconds-dialog { background: white; border-radius: var(--radius); display: grid; gap: 14px; max-height: calc(100dvh - 32px - env(safe-area-inset-top) - env(safe-area-inset-bottom)); max-width: 520px; overflow-y: auto; overscroll-behavior: contain; padding: 17px; width: 100%; }.seconds-dialog header { align-items: center; display: flex; justify-content: space-between; }.seconds-dialog header div { display: grid; gap: 4px; }.seconds-dialog header strong { font-size: 18px; }.seconds-dialog header small,.seconds-dialog > p { color: var(--muted); font-size: 12px; margin: 0; }.direction-toggle { background: var(--soft); border-radius: var(--radius); display: grid; grid-template-columns: 1fr 1fr; padding: 4px; }.direction-toggle button { background: transparent; border-radius: calc(var(--radius) - 2px); color: var(--muted); font-size: 14px; font-weight: 700; min-height: 37px; }.direction-toggle .is-up { background: var(--positive); color: white; }.direction-toggle .is-down { background: var(--negative); color: white; }.seconds-dialog label { display: grid; gap: 7px; }.seconds-dialog label > span { color: var(--muted); font-size: 13px; }.seconds-dialog select { background: var(--soft); border: 0; border-radius: var(--radius); color: var(--ink); font: inherit; min-height: 48px; padding: 0 12px; }.seconds-amount { align-items: center; background: var(--soft); border-radius: var(--radius); display: grid; grid-template-columns: 1fr auto; min-height: 52px; padding: 0 12px; }.seconds-amount input { background: transparent; border: 0; font-size: 20px; font-weight: 720; min-width: 0; outline: 0; width: 100%; }.seconds-amount b { font-size: 13px; }.success-message { color: var(--positive); font-size: 13px; font-weight: 650; margin: 0; }
</style>
