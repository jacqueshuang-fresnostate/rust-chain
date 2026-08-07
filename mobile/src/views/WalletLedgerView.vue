<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { CircleAlert, FileSearch, LoaderCircle, RefreshCw } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
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
  <main
    class="page page--plain pencil-page wallet-pencil-page wallet-ledger-pencil"
    data-pencil-source="y6Y7TW m25xr0 Bcug6 IVMAO"
  >
    <PageHeader
      :back="true"
      :eyebrow="t('assets.title')"
      :fallback="{ name: 'assets' }"
      :pencil="true"
      :subtitle="t('ledger.loginDescription')"
      :title="t('assets.fundLedger')"
    >
      <template #actions>
        <button class="icon-button" type="button" :aria-label="t('ledger.refresh')" :disabled="loading" @click="load()">
          <RefreshCw :size="18" :class="{ spin: loading }" aria-hidden="true" />
        </button>
      </template>
    </PageHeader>
    <div class="page-content ledger-page">
      <LoginRequiredState
        v-if="!session.isAuthenticated"
        class="wallet-login-prompt"
        :description="t('ledger.loginDescription')"
      />
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
        <div v-if="error && !entries.length" class="ledger-state ledger-state--error" role="alert">
          <span class="ledger-state__plate"><CircleAlert :size="24" aria-hidden="true" /></span>
          <strong>{{ t('common.serviceUnavailable') }}</strong>
          <span>{{ error }}</span>
          <button type="button" :disabled="loading" @click="load()">{{ t('common.retry') }}</button>
        </div>
        <div v-else-if="loading" class="ledger-loading" role="status">
          <LoaderCircle :size="23" class="spin" aria-hidden="true" />
          <span>{{ t('ledger.loading') }}</span>
        </div>
        <div v-else-if="sortedEntries.length" class="ledger-list" role="list">
          <article v-for="entry in sortedEntries" :key="entry.id" class="ledger-row" role="listitem">
            <div class="ledger-row__title">
              <strong>{{ entryLabel(entry.changeType) }}</strong>
              <small>{{ entry.symbol }} · {{ formatDateTime(entry.createdAt) }}</small>
            </div>
            <div class="ledger-row__amount">
              <strong class="numeric" :class="isPositive(entry) ? 'up' : 'down'">{{ isPositive(entry) ? '+' : '' }}{{ formatAmount(entry.amount) }} {{ entry.symbol }}</strong>
              <small class="numeric">{{ t('ledger.balance', { amount: formatAmount(entry.balanceAfter) }) }}</small>
            </div>
          </article>
        </div>
        <div v-else class="ledger-state ledger-state--empty" role="status">
          <span class="ledger-state__plate"><FileSearch :size="24" aria-hidden="true" /></span>
          <strong>{{ t('ledger.empty') }}</strong>
          <span>{{ t('ledger.emptyDescription') }}</span>
        </div>
        <div v-if="error && entries.length" class="ledger-inline-error" role="alert">
          <CircleAlert :size="16" aria-hidden="true" />
          <span>{{ error }}</span>
          <button type="button" :aria-label="t('common.retry')" :disabled="loading" @click="load()">
            <RefreshCw :size="16" aria-hidden="true" />
          </button>
        </div>
        <button v-if="!loading && !exhausted && entries.length" class="button button--secondary button--full" type="button" :disabled="loadingMore" @click="load(false)">{{ loadingMore ? t('common.loading') : t('common.loadMore') }}</button>
      </template>
    </div>
  </main>
</template>

<style scoped>
.wallet-pencil-page {
  background: var(--page);
}

.ledger-page {
  display: grid;
  gap: 10px;
  padding: 6px 20px calc(20px + env(safe-area-inset-bottom));
}

.ledger-filter {
  align-items: center;
  display: flex;
  gap: 8px;
  min-height: 44px;
  overflow-x: auto;
  scrollbar-width: none;
}

.ledger-filter::-webkit-scrollbar {
  display: none;
}

.ledger-filter button {
  background: transparent;
  border: 0;
  border-radius: var(--wallet-pill-radius, 999px);
  color: var(--muted);
  flex: 0 0 auto;
  font-size: 12px;
  font-weight: 500;
  height: 28px;
  min-height: 28px;
  padding: 0 12px;
  position: relative;
}

.ledger-filter button::before {
  content: '';
  inset: -8px 0;
  position: absolute;
}

.ledger-filter .is-active {
  background: var(--accent-soft);
  color: var(--positive);
  font-weight: 400;
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

.ledger-state {
  align-items: center;
  color: var(--muted);
  display: flex;
  flex-direction: column;
  font-size: 11px;
  gap: 12px;
  justify-content: center;
  min-height: 225px;
  padding: 48px 20px;
  text-align: center;
}

.ledger-state__plate {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--line);
  border-radius: 50%;
  color: var(--muted);
  display: flex;
  height: 56px;
  justify-content: center;
  width: 56px;
}

.ledger-state strong {
  color: var(--ink);
  font-size: 15px;
  font-weight: 650;
  line-height: 20px;
}

.ledger-state > span:last-child {
  line-height: 17px;
  max-width: 300px;
}

.ledger-state--error .ledger-state__plate,
.ledger-state--error strong {
  color: var(--negative);
}

.ledger-state--error button {
  background: transparent;
  border: 1px solid var(--line);
  color: var(--positive);
  font-size: 11px;
  min-height: 44px;
  padding: 0 18px;
}

.ledger-inline-error {
  align-items: center;
  background: var(--negative-soft);
  color: var(--negative);
  display: grid;
  font-size: 11px;
  gap: 8px;
  grid-template-columns: 18px minmax(0, 1fr) 44px;
  min-height: 44px;
  padding-left: 10px;
}

.ledger-inline-error button {
  background: transparent;
  color: inherit;
  display: grid;
  min-height: 44px;
  min-width: 44px;
  padding: 0;
  place-items: center;
}

.ledger-list {
  display: grid;
}

.ledger-row {
  align-items: center;
  border-bottom: 1px solid var(--hairline);
  display: grid;
  gap: 12px;
  grid-template-columns: minmax(0, 1fr) minmax(96px, auto);
  height: 56px;
  min-height: 56px;
  padding: 0;
}

.ledger-row__title,
.ledger-row__amount {
  display: grid;
  gap: 3px;
  min-width: 0;
}

.ledger-row strong {
  font-size: 13px;
  line-height: 18px;
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
  line-height: 15px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ledger-row__amount {
  max-width: 168px;
  text-align: right;
}

.ledger-page > .button {
  border-radius: var(--wallet-pill-radius, 999px);
  min-height: 44px;
}

.wallet-feedback {
  margin: 0;
}

.wallet-login-prompt {
  background: transparent;
  background-image: none;
  border: 0;
  border-top: 1px solid var(--hairline);
  gap: 10px;
  grid-template-columns: 34px minmax(0, 1fr) auto;
  min-height: 72px;
  padding: 10px 0;
}

.wallet-login-prompt :deep(.login-required__icon) {
  background: var(--accent-soft);
  border: 0;
  color: var(--positive);
  height: 34px;
  width: 34px;
}

.wallet-login-prompt :deep(.login-required__copy) {
  gap: 2px;
}

.wallet-login-prompt :deep(.login-required__copy strong) {
  font-size: 13px;
}

.wallet-login-prompt :deep(.login-required__copy p) {
  color: var(--muted);
  font-size: 11px;
  line-height: 1.4;
}

.wallet-login-prompt :deep(.button) {
  border-radius: var(--wallet-pill-radius, 999px);
  min-height: 44px;
  padding-inline: 14px;
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
    grid-template-columns: minmax(0, 1fr) minmax(84px, auto);
  }

  .ledger-row__amount {
    max-width: 128px;
  }

  .wallet-login-prompt {
    align-items: center;
    grid-template-columns: 34px minmax(0, 1fr);
  }

  .wallet-login-prompt :deep(.button) {
    grid-column: 2;
    justify-self: start;
  }
}

@media (prefers-reduced-motion: reduce) {
  .spin {
    animation: none;
  }
}
</style>
