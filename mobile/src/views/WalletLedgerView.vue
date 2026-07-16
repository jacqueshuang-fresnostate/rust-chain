<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { RefreshCw } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import AssetMark from '@/components/AssetMark.vue'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchWalletLedger, type WalletLedgerEntry } from '@/api/wallet'
import { formatAmount, formatDateTime } from '@/core/format'
import { useSessionStore } from '@/stores/session'

type Filter = { key: 'all' | 'deposit' | 'trade' | 'contract'; value?: string }
const filters: Filter[] = [
  { key: 'all' },
  { key: 'deposit', value: 'deposit' },
  { key: 'trade', value: 'spot_trade_settlement' },
  { key: 'contract', value: 'margin_position_open' },
]

const session = useSessionStore()
const { t } = useI18n()
const entries = ref<WalletLedgerEntry[]>([])
const activeFilter = ref<Filter['key']>('all')
const loading = ref(false)
const loadingMore = ref(false)
const exhausted = ref(false)
const error = ref('')

const sortedEntries = computed(() => [...entries.value].sort((left, right) => right.createdAt - left.createdAt))

async function load(reset = true): Promise<void> {
  if (!session.isAuthenticated) return
  if (reset) loading.value = true
  else loadingMore.value = true
  error.value = ''
  try {
    const offset = reset ? 0 : entries.value.length
    const rows = await fetchWalletLedger(30, offset, filters.find((filter) => filter.key === activeFilter.value)?.value)
    entries.value = reset ? rows : [...entries.value, ...rows]
    exhausted.value = rows.length < 30
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('ledger.loadFailed'))
  } finally {
    loading.value = false
    loadingMore.value = false
  }
}

function changeFilter(filter: Filter['key']): void {
  activeFilter.value = filter
  void load()
}

function isPositive(entry: WalletLedgerEntry): boolean {
  return entry.amount >= 0
}

function entryLabel(changeType: string): string {
  const labels: Record<string, string> = {
    deposit: 'ledger.typeDeposit',
    admin_recharge: 'ledger.typeAdminRecharge',
    quick_recharge: 'ledger.typeQuickRecharge',
    spot_freeze: 'ledger.typeSpotFreeze',
    spot_unfreeze: 'ledger.typeSpotUnfreeze',
    spot_fill: 'ledger.typeSpotFill',
    spot_trade_settlement: 'ledger.typeSpotSettlement',
    margin_position_open: 'ledger.typeMarginOpen',
    margin_position_close: 'ledger.typeMarginClose',
    margin_position_liquidate: 'ledger.typeMarginLiquidate',
    convert_settlement: 'ledger.typeConvertSettlement',
  }
  return labels[changeType] ? t(labels[changeType]) : changeType.replace(/_/g, ' ')
}

function filterLabel(key: Filter['key']): string {
  return t(`ledger.${key}`)
}

onMounted(() => { void load() })
</script>

<template>
  <main class="page page--plain">
    <PageHeader :title="t('ledger.title')"><template #actions><button class="icon-button" type="button" :aria-label="t('ledger.refresh')" :disabled="loading" @click="load()"><RefreshCw :size="21" :class="{ spin: loading }" /></button></template></PageHeader>
    <div class="page-content ledger-page">
      <LoginRequiredState v-if="!session.isAuthenticated" :description="t('ledger.loginDescription')" />
      <template v-else>
        <nav class="ledger-filter" :aria-label="t('ledger.filterLabel')"><button v-for="filter in filters" :key="filter.key" type="button" :class="{ 'is-active': activeFilter === filter.key }" @click="changeFilter(filter.key)">{{ filterLabel(filter.key) }}</button></nav>
        <p v-if="error" class="error-message">{{ error }}</p>
        <p v-if="loading" class="empty-state">{{ t('ledger.loading') }}</p>
        <div v-else-if="sortedEntries.length" class="ledger-list"><article v-for="entry in sortedEntries" :key="entry.id" class="ledger-row"><AssetMark :symbol="entry.symbol" :size="38" /><div class="ledger-row__title"><strong>{{ entryLabel(entry.changeType) }}</strong><small>{{ formatDateTime(entry.createdAt) }}</small></div><div class="ledger-row__amount"><strong :class="isPositive(entry) ? 'up' : 'down'">{{ isPositive(entry) ? '+' : '' }}{{ formatAmount(entry.amount) }} {{ entry.symbol }}</strong><small>{{ t('ledger.balance', { amount: formatAmount(entry.balanceAfter) }) }}</small></div></article></div>
        <p v-else class="empty-state">{{ t('ledger.empty') }}</p>
        <button v-if="!loading && !exhausted && entries.length" class="button button--secondary button--full" type="button" :disabled="loadingMore" @click="load(false)">{{ loadingMore ? t('common.loading') : t('common.loadMore') }}</button>
      </template>
    </div>
  </main>
</template>

<style scoped>
.ledger-page { padding-bottom: 36px; }.ledger-filter { border-bottom: 1px solid var(--line); display: flex; gap: 22px; margin: 0 -20px 4px; overflow: auto; padding: 0 20px; }.ledger-filter button { background: transparent; border-bottom: 2px solid transparent; color: var(--muted); flex: 0 0 auto; font-size: 14px; font-weight: 650; min-height: 48px; padding: 0; }.ledger-filter .is-active { border-color: var(--ink); color: var(--ink); font-weight: 760; }.ledger-list { display: grid; }.ledger-row { align-items: center; border-bottom: 1px solid var(--line); display: grid; gap: 12px; grid-template-columns: 38px minmax(0, 1fr) auto; min-height: 76px; }.ledger-row__title,.ledger-row__amount { display: grid; gap: 5px; min-width: 0; }.ledger-row strong { font-size: 14px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }.ledger-row small { color: var(--muted); font-size: 12px; }.ledger-row__amount { text-align: right; }.spin { animation: spin .8s linear infinite; }@keyframes spin { to { transform: rotate(360deg); } }
</style>
