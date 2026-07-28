<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { LoaderCircle, RefreshCw } from 'lucide-vue-next'
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
    <PageHeader
      :back="true"
      :eyebrow="t('assets.title')"
      :subtitle="t('ledger.loginDescription')"
      :title="t('ledger.title')"
    >
      <template #actions>
        <button class="icon-button" type="button" :aria-label="t('ledger.refresh')" :disabled="loading" @click="load()">
          <RefreshCw :size="21" :class="{ spin: loading }" aria-hidden="true" />
        </button>
      </template>
    </PageHeader>
    <div class="page-content ledger-page">
      <LoginRequiredState v-if="!session.isAuthenticated" :description="t('ledger.loginDescription')" />
      <template v-else>
        <nav class="ledger-filter" :aria-label="t('ledger.filterLabel')">
          <button
            v-for="filter in filters"
            :key="filter.key"
            type="button"
            :aria-pressed="activeFilter === filter.key"
            :class="{ 'is-active': activeFilter === filter.key }"
            :disabled="loading"
            @click="changeFilter(filter.key)"
          >
            {{ filterLabel(filter.key) }}
          </button>
        </nav>
        <p v-if="error" class="error-message" role="alert">{{ error }}</p>
        <div v-if="loading" class="ledger-loading" role="status">
          <LoaderCircle :size="23" class="spin" aria-hidden="true" />
          <span>{{ t('ledger.loading') }}</span>
        </div>
        <div v-else-if="sortedEntries.length" class="ledger-list" role="list">
          <article v-for="entry in sortedEntries" :key="entry.id" class="ledger-row" role="listitem">
            <AssetMark :symbol="entry.symbol" :size="38" />
            <div class="ledger-row__title">
              <strong>{{ entryLabel(entry.changeType) }}</strong>
              <small>{{ formatDateTime(entry.createdAt) }}</small>
            </div>
            <div class="ledger-row__amount">
              <strong class="numeric" :class="isPositive(entry) ? 'up' : 'down'">{{ isPositive(entry) ? '+' : '' }}{{ formatAmount(entry.amount) }} {{ entry.symbol }}</strong>
              <small class="numeric">{{ t('ledger.balance', { amount: formatAmount(entry.balanceAfter) }) }}</small>
            </div>
          </article>
        </div>
        <p v-else class="empty-state">{{ t('ledger.empty') }}</p>
        <button v-if="!loading && !exhausted && entries.length" class="button button--secondary button--full" type="button" :disabled="loadingMore" @click="load(false)">{{ loadingMore ? t('common.loading') : t('common.loadMore') }}</button>
      </template>
    </div>
  </main>
</template>

<style scoped>
.ledger-page {
  padding-bottom: calc(36px + env(safe-area-inset-bottom));
  padding-top: 8px;
}

.ledger-filter {
  background: var(--soft);
  border: 1px solid var(--line);
  border-radius: var(--radius);
  display: grid;
  gap: 3px;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  padding: 3px;
}

.ledger-filter button {
  background: transparent;
  border: 1px solid transparent;
  border-radius: calc(var(--radius) - 3px);
  color: var(--muted);
  font-size: 12px;
  font-weight: 680;
  min-height: 44px;
  min-width: 0;
  padding: 0 4px;
}

.ledger-filter .is-active {
  background: var(--surface-elevated);
  border-color: var(--line-strong);
  box-shadow: var(--shadow-soft);
  color: var(--ink);
  font-weight: 760;
}

.ledger-loading {
  align-items: center;
  color: var(--muted);
  display: flex;
  font-size: 13px;
  gap: 10px;
  justify-content: center;
  min-height: 180px;
}

.ledger-list {
  border-top: 1px solid var(--line);
  display: grid;
  margin-top: 14px;
}

.ledger-row {
  align-items: center;
  border-bottom: 1px solid var(--line);
  display: grid;
  gap: 11px;
  grid-template-columns: 38px minmax(0, 1fr) minmax(96px, auto);
  min-height: 78px;
  padding: 8px 0;
}

.ledger-row__title,
.ledger-row__amount {
  display: grid;
  gap: 5px;
  min-width: 0;
}

.ledger-row strong {
  font-size: 13px;
  line-height: 1.35;
  min-width: 0;
  overflow-wrap: anywhere;
}

.ledger-row__title strong {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ledger-row small {
  color: var(--muted);
  font-size: 11px;
  line-height: 1.35;
}

.ledger-row__amount {
  max-width: 168px;
  text-align: right;
}

.spin {
  animation: spin .8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

@media (max-width: 340px) {
  .ledger-page {
    padding-left: 16px;
    padding-right: 16px;
  }

  .ledger-filter button {
    font-size: 11px;
    padding-inline: 2px;
  }

  .ledger-row {
    gap: 8px;
    grid-template-columns: 34px minmax(0, 1fr) minmax(84px, auto);
  }

  .ledger-row__amount {
    max-width: 128px;
  }
}
</style>
