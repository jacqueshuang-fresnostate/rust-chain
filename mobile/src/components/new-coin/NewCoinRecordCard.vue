<script setup lang="ts">
import { computed } from 'vue'
import { ChevronRight, CreditCard, LockOpen } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import AssetMark from '@/components/AssetMark.vue'
import { formatFinancialAmount } from '@/core/financialDisplay'
import { formatDateTime } from '@/core/format'
import type { UnifiedNewCoinRecord } from '@/core/newCoinPresentation'

const props = defineProps<{
  record: UnifiedNewCoinRecord
  saving?: boolean
}>()

defineEmits<{
  open: []
  payFee: [event: MouseEvent]
  release: []
}>()

const { t, locale } = useI18n()
const symbol = computed(() => props.record.assetSymbol
  || (props.record.assetId ? t('newCoin.assetNumber', { id: props.record.assetId }) : t('newCoin.unavailableValue')))
const title = computed(() => props.record.project
  ? props.record.project.name || t('newCoin.projectNameUnavailable')
  : symbol.value)
const status = computed(() => {
  const key = ({
    pending: 'newCoin.statusPending',
    processing: 'newCoin.statusProcessing',
    completed: 'newCoin.statusCompleted',
    allocated: 'newCoin.statusAllocated',
    partial_allocated: 'newCoin.statusPartialAllocated',
    refunded: 'newCoin.statusRefunded',
    distributed: 'newCoin.statusDistributed',
    locked: 'newCoin.statusLocked',
    available: 'newCoin.statusAvailable',
    paid: 'newCoin.statusPaid',
    unpaid: 'newCoin.statusUnpaid',
    released: 'newCoin.statusReleased',
    cancelled: 'newCoin.statusCancelled',
    canceled: 'newCoin.statusCancelled',
  } as Record<string, string>)[props.record.status.toLowerCase()]
  return key ? t(key) : props.record.status
})
const typeLabel = computed(() => t(`newCoin.recordType.${props.record.kind}`))
const primaryAmount = computed(() => {
  const value = props.record.subscription?.requestedQuantityText
    || props.record.distribution?.quantityText
    || props.record.purchase?.quantityText
    || props.record.unlock?.unlockQuantityText
  return format(value, symbol.value)
})
const quoteSymbol = computed(() => {
  const quoteAssetId = props.record.subscription?.quoteAssetId || props.record.purchase?.quoteAssetId
  if (
    quoteAssetId
    && props.record.project?.quoteAssetId === quoteAssetId
    && props.record.project.quoteAssetSymbol
  ) {
    return props.record.project.quoteAssetSymbol
  }
  return quoteAssetId
    ? t('newCoin.assetNumber', { id: quoteAssetId })
    : t('newCoin.unavailableValue')
})
const secondaryLabel = computed(() => {
  if (props.record.subscription?.settlementMode === 'manual_distribution') return t(props.record.status === 'pending' ? 'newCoin.frozenLabel' : 'newCoin.settledRefundLabel')
  if (props.record.subscription) return t('newCoin.paidLabel')
  if (props.record.distribution) return t('newCoin.distributionDestination')
  if (props.record.purchase) return t('newCoin.paidLabel')
  return props.record.unlock?.unlockFeeEnabled ? t('newCoin.unlockFee') : t('newCoin.release')
})
const secondaryAmount = computed(() => {
  const subscription = props.record.subscription
  if (subscription?.settlementMode === 'manual_distribution') {
    const amount = subscription.status === 'pending'
      ? format(subscription.frozenQuoteAmountText, quoteSymbol.value)
      : `${format(subscription.settledQuoteAmountText, quoteSymbol.value)} / ${format(subscription.refundedQuoteAmountText, quoteSymbol.value)}`
    return `${amount} ${quoteSymbol.value}`
  }
  if (subscription) return `${format(subscription.quoteAmountText, quoteSymbol.value)} ${quoteSymbol.value}`
  if (props.record.purchase) return `${format(props.record.purchase.quoteAmountText, quoteSymbol.value)} ${quoteSymbol.value}`
  if (props.record.distribution) {
    if (props.record.distribution.status === 'refunded') return t('newCoin.statusRefunded')
    return props.record.distribution.lockPositionId
      ? t('newCoin.lockPositionNumber', { id: props.record.distribution.lockPositionId })
      : t('newCoin.credited')
  }
  const unlock = props.record.unlock
  if (unlock?.unlockFeeEnabled) {
    const feeAsset = unlock.unlockFeeAssetId && props.record.project?.quoteAssetId === unlock.unlockFeeAssetId
      ? props.record.project.quoteAssetSymbol || t('newCoin.assetNumber', { id: unlock.unlockFeeAssetId })
      : unlock.unlockFeeAssetId
        ? t('newCoin.assetNumber', { id: unlock.unlockFeeAssetId })
        : t('newCoin.unavailableValue')
    return `${format(unlock.unlockFeeAmountText, feeAsset)} ${feeAsset}`
  }
  return t('newCoin.noUnlockFee')
})
const unlock = computed(() => props.record.unlock)
const feePaid = computed(() => ['paid', 'not_required'].includes(unlock.value?.feePaidStatus.toLowerCase() || ''))
const canPayFee = computed(() => Boolean(
  unlock.value?.unlockFeeEnabled
  && !feePaid.value
  && unlock.value.unlockFeeAmountText
  && unlock.value.unlockFeeAssetId,
))
const canRelease = computed(() => Boolean(
  unlock.value
  && (!unlock.value.unlockFeeEnabled || feePaid.value)
  && !['released', 'completed'].includes(unlock.value.status.toLowerCase()),
))

function format(value: string | null | undefined, asset?: string): string {
  return value
    ? formatFinancialAmount(value, locale.value, { assetSymbol: asset })
    : t('newCoin.unavailableValue')
}
</script>

<template>
  <article class="new-coin-record-card">
    <i class="new-coin-record-card__rail" aria-hidden="true" />
    <div class="new-coin-record-card__body">
      <header>
        <AssetMark class="new-coin-record-card__mark" :symbol="symbol" :src="record.assetLogoUrl" :size="40" />
        <span class="new-coin-record-card__identity">
          <strong>{{ title }}</strong>
          <small>{{ formatDateTime(record.createdAt) }}</small>
        </span>
        <b>{{ status }}</b>
      </header>

      <dl>
        <div>
          <dt>{{ typeLabel }}</dt>
          <dd>{{ primaryAmount }} {{ symbol }}</dd>
        </div>
        <div>
          <dt>{{ secondaryLabel }}</dt>
          <dd>{{ secondaryAmount }}</dd>
        </div>
      </dl>

      <footer>
        <small>{{ t('newCoin.recordNumber', { number: record.recordNo }) }}</small>
        <button v-if="canPayFee" type="button" :disabled="saving" @click="$emit('payFee', $event)">
          <span class="new-coin-record-card__action-face">
            <CreditCard :size="13" /><span>{{ t(saving ? 'newCoin.paying' : 'newCoin.payFee') }}</span>
          </span>
        </button>
        <button v-else-if="canRelease" type="button" :disabled="saving" @click="$emit('release')">
          <span class="new-coin-record-card__action-face">
            <LockOpen :size="13" /><span>{{ t(saving ? 'newCoin.releasing' : 'newCoin.release') }}</span>
          </span>
        </button>
        <button v-else-if="record.project" type="button" :aria-label="t('newCoin.viewDetails')" @click="$emit('open')">
          <span class="new-coin-record-card__action-face">
            <span>{{ t('newCoin.viewDetails') }}</span><ChevronRight :size="14" />
          </span>
        </button>
        <span v-else class="new-coin-record-card__source">{{ typeLabel }}</span>
      </footer>
    </div>
  </article>
</template>

<style scoped>
.new-coin-record-card {
  background: var(--new-coin-record-card);
  border: 1px solid var(--new-coin-record-border);
  border-radius: 18px;
  box-shadow: var(--new-coin-record-shadow);
  box-sizing: border-box;
  color: var(--new-coin-record-ink);
  height: 168px;
  overflow: hidden;
  position: relative;
  width: 100%;
}

.new-coin-record-card__rail {
  background: var(--new-coin-record-rail);
  height: 100%;
  left: 0;
  position: absolute;
  top: 0;
  width: 4px;
}

.new-coin-record-card :deep(.new-coin-record-card__mark.asset-mark--fallback) {
  --asset-color: var(--new-coin-record-active);
  --asset-ink: var(--new-coin-record-active);
  background: var(--new-coin-record-mark);
  border-color: var(--new-coin-record-mark-border);
}

.new-coin-record-card__body {
  display: grid;
  gap: 10px;
  height: 100%;
  padding: 14px 14px 14px 18px;
}

.new-coin-record-card header {
  align-items: center;
  display: flex;
  height: 40px;
  min-width: 0;
}

.new-coin-record-card__identity {
  display: grid;
  flex: 1;
  gap: 1px;
  margin-left: 9px;
  min-width: 0;
}

.new-coin-record-card header strong {
  font-size: 15px;
  font-weight: 750;
  line-height: 22px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.new-coin-record-card header small {
  color: var(--new-coin-record-muted);
  font-size: 9px;
  line-height: 13px;
}

.new-coin-record-card header > b {
  align-items: center;
  background: var(--new-coin-record-status);
  border-radius: 12px;
  color: var(--new-coin-record-active);
  display: flex;
  font-size: 9px;
  height: 24px;
  justify-content: center;
  max-width: 84px;
  min-width: 57px;
  overflow: hidden;
  padding: 0 7px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.new-coin-record-card header > b::before {
  background: currentColor;
  border-radius: 50%;
  content: '';
  flex: 0 0 auto;
  height: 5px;
  margin-right: 4px;
  width: 5px;
}

.new-coin-record-card dl {
  border-bottom: 1px solid var(--new-coin-record-line);
  display: grid;
  grid-template-columns: minmax(0, 1.7fr) minmax(0, 1fr);
  height: 54px;
  margin: 0;
  padding-bottom: 9px;
}

.new-coin-record-card dl > div {
  display: grid;
  min-width: 0;
}

.new-coin-record-card dl > div + div {
  border-left: 1px solid var(--new-coin-record-line);
  padding-left: 12px;
}

.new-coin-record-card dt {
  color: var(--new-coin-record-muted);
  font-size: 9px;
  line-height: 14px;
}

.new-coin-record-card dd {
  font-family: var(--font-numeric);
  font-size: 13px;
  font-weight: 700;
  line-height: 19px;
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.new-coin-record-card dl > div:first-child dd {
  color: var(--new-coin-record-active);
  font-size: 15px;
  font-weight: 750;
}

.new-coin-record-card footer {
  align-items: center;
  display: flex;
  height: 26px;
  justify-content: space-between;
  min-width: 0;
}

.new-coin-record-card footer > small {
  color: var(--new-coin-record-muted);
  font-family: var(--font-numeric);
  font-size: 9px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.new-coin-record-card footer button {
  align-items: center;
  background: transparent;
  border: 0;
  color: var(--new-coin-record-active);
  display: flex;
  flex: 0 0 auto;
  font-size: 9px;
  height: 44px;
  justify-content: center;
  margin-right: -8px;
  min-width: 44px;
  padding: 0 4px;
}

.new-coin-record-card__action-face {
  align-items: center;
  background: var(--new-coin-record-action);
  border-radius: 12px;
  display: flex;
  gap: 3px;
  height: 24px;
  justify-content: center;
  padding: 0 8px;
}

.new-coin-record-card footer button:focus-visible {
  outline: 2px solid var(--focus-ring);
  outline-offset: 1px;
}

.new-coin-record-card__source {
  color: var(--new-coin-record-muted);
  font-size: 9px;
}

@media (max-width: 340px) {
  .new-coin-record-card__body {
    padding-right: 11px;
  }

  .new-coin-record-card dl {
    grid-template-columns: minmax(0, 1.45fr) minmax(0, 1fr);
  }

  .new-coin-record-card__action-face > span {
    display: none;
  }
}
</style>
