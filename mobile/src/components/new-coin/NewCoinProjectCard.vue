<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import AssetMark from '@/components/AssetMark.vue'
import { decimalMultiply, formatDecimalText, normalizeDecimalText } from '@/core/decimal'
import { formatFinancialAmount } from '@/core/financialDisplay'
import { formatDateTime } from '@/core/format'
import { newCoinProjectProgress, newCoinUnlockTypeTranslationKey } from '@/core/newCoinPresentation'
import type { NewCoinProject } from '@/core/newCoinModel'

const props = defineProps<{
  project: NewCoinProject
  now: number
}>()

defineEmits<{
  open: []
}>()

const { t, locale } = useI18n()
const progress = computed(() => newCoinProjectProgress(props.project))
const lifecycleLabel = computed(() => {
  const key = ({
    preheat: 'newCoin.preheat',
    subscription: 'newCoin.subscriptionOpen',
    distribution: 'newCoin.waitingDistribution',
    listed: 'newCoin.listed',
    closed: 'newCoin.closed',
  } as Record<string, string>)[props.project.lifecycleStatus.toLowerCase()]
  return key ? t(key) : props.project.lifecycleStatus
})
const projectName = computed(() => props.project.name || t('newCoin.projectNameUnavailable'))
const quoteSymbol = computed(() => props.project.quoteAssetSymbol || t('newCoin.unavailableValue'))
const issuePrice = computed(() => formatFinancialAmount(props.project.issuePriceText, locale.value, {
  assetSymbol: props.project.quoteAssetSymbol,
  minimumFractionDigits: props.project.quoteAssetSymbol ? undefined : 0,
}))
const totalSupply = computed(() => formatFinancialAmount(
  props.project.totalSupplyText,
  locale.value,
  { assetSymbol: props.project.symbol },
))
const issuePriceDisplay = computed(() => `${issuePrice.value} ${quoteSymbol.value}`)
const totalSupplyDisplay = computed(() => `${totalSupply.value} ${props.project.symbol}`)
const progressLabel = computed(() => formatDecimalText(
  decimalMultiply(progress.value.ratio, normalizeDecimalText('100')),
  locale.value,
  {
    maximumFractionDigits: 2,
    preserveNonZero: true,
  },
))
const actionLabel = computed(() => props.project.lifecycleStatus.toLowerCase() === 'subscription'
  ? t('newCoin.subscribeNow')
  : t('newCoin.viewDetails'))
const timing = computed(() => {
  if (!props.project.listedAt) return t('newCoin.pendingSchedule')
  const remainingSeconds = Math.max(0, Math.ceil((props.project.listedAt - props.now) / 1000))
  if (!remainingSeconds) return formatDateTime(props.project.listedAt)
  const days = Math.floor(remainingSeconds / 86400)
  const hours = Math.floor((remainingSeconds % 86400) / 3600)
  const minutes = Math.floor((remainingSeconds % 3600) / 60)
  return t('newCoin.listingCountdown', { days, hours, minutes })
})
const unlockLabel = computed(() => {
  if (props.project.fixedUnlockAt) return formatDateTime(props.project.fixedUnlockAt)
  if (props.project.relativeUnlockSeconds !== undefined) {
    return t('newCoin.days', { days: Math.ceil(props.project.relativeUnlockSeconds / 86400) })
  }
  const key = newCoinUnlockTypeTranslationKey(props.project.unlockType)
  return key ? t(key) : props.project.unlockType
})

function financialValueClass(value: string): Record<string, boolean> {
  const length = Array.from(value).length
  return {
    'is-long-value': length > 14,
    'is-very-long-value': length > 21,
  }
}
</script>

<template>
  <article class="new-coin-project-card">
    <button type="button" class="new-coin-project-card__hit" @click="$emit('open')">
      <span class="new-coin-project-card__identity">
        <AssetMark :symbol="project.symbol" :src="project.logoUrl" :size="40" />
        <span class="new-coin-project-card__name">
          <strong>{{ project.symbol }}</strong>
          <small>{{ projectName }}</small>
        </span>
        <b class="new-coin-project-card__status">{{ lifecycleLabel }}</b>
      </span>

      <span class="new-coin-project-card__stats">
        <span>
          <small>{{ t('newCoin.issuePrice') }}</small>
          <strong
            class="new-coin-project-card__issue-price"
            :class="financialValueClass(issuePriceDisplay)"
            :title="issuePriceDisplay"
          >
            <span class="new-coin-project-card__issue-price-value">{{ issuePrice }}</span>
            <span class="new-coin-project-card__issue-price-symbol">{{ quoteSymbol }}</span>
          </strong>
        </span>
        <span><small>{{ t('newCoin.plannedIssue') }}</small><strong :class="financialValueClass(totalSupplyDisplay)" :title="totalSupplyDisplay">{{ totalSupplyDisplay }}</strong></span>
      </span>

      <span class="new-coin-project-card__row">
        <small>{{ t('newCoin.subscriptionProgress') }}</small>
        <span class="new-coin-project-card__progress" aria-hidden="true"><i :style="{ width: `${progress.percentage}%` }" /></span>
        <strong>{{ t('newCoin.progressRatio', { ratio: progressLabel }) }}</strong>
      </span>

      <span class="new-coin-project-card__row">
        <small>{{ t('newCoin.unlockMethod') }}</small>
        <strong>{{ unlockLabel }}</strong>
      </span>

      <span class="new-coin-project-card__countdown">
        <small>{{ t('newCoin.listingTime') }}</small>
        <strong>{{ timing }}</strong>
      </span>

      <span class="new-coin-project-card__action">{{ actionLabel }}</span>
    </button>
  </article>
</template>

<style scoped>
.new-coin-project-card {
  background: var(--new-coin-card);
  border: 1px solid var(--new-coin-card-border);
  border-radius: 22px;
  box-sizing: border-box;
  height: 300px;
  overflow: hidden;
  width: 100%;
}

.new-coin-project-card__hit {
  align-items: stretch;
  background: transparent;
  border: 0;
  color: var(--new-coin-ink);
  display: flex;
  flex-direction: column;
  gap: 8px;
  height: 100%;
  padding: 14px;
  text-align: left;
  width: 100%;
}

.new-coin-project-card__hit:focus-visible {
  box-shadow: inset 0 0 0 3px var(--focus-ring);
  outline: 0;
}

.new-coin-project-card__identity {
  align-items: center;
  display: flex;
  height: 44px;
  min-width: 0;
}

.new-coin-project-card__name {
  display: grid;
  flex: 1;
  gap: 1px;
  margin-left: 10px;
  min-width: 0;
}

.new-coin-project-card__name strong {
  font-size: 18px;
  font-weight: 750;
  line-height: 26px;
}

.new-coin-project-card__name small,
.new-coin-project-card__stats small,
.new-coin-project-card__row small,
.new-coin-project-card__countdown small {
  color: var(--new-coin-muted);
}

.new-coin-project-card__name small {
  font-size: 10px;
  line-height: 14px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.new-coin-project-card__status {
  align-items: center;
  background: var(--new-coin-status);
  border-radius: 9px;
  color: var(--new-coin-signal);
  display: inline-flex;
  font-size: 10px;
  font-weight: 700;
  height: 26px;
  justify-content: center;
  max-width: 96px;
  min-width: 66px;
  overflow: hidden;
  padding: 0 8px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.new-coin-project-card__stats {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  height: 48px;
}

.new-coin-project-card__stats > span {
  display: grid;
  min-width: 0;
}

.new-coin-project-card__stats > span:last-child {
  text-align: right;
}

.new-coin-project-card__stats small {
  font-size: 11px;
  line-height: 16px;
}

.new-coin-project-card__stats strong {
  font-family: var(--font-numeric);
  font-size: clamp(15px, 5.35vw, 21px);
  font-weight: 750;
  line-height: 30px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.new-coin-project-card__stats .new-coin-project-card__issue-price {
  align-items: baseline;
  display: flex;
  gap: 5px;
  max-width: 100%;
  min-width: 0;
  overflow: visible;
  width: 100%;
}

.new-coin-project-card__issue-price-value {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.new-coin-project-card__issue-price-symbol {
  flex: 0 0 auto;
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0;
  line-height: 16px;
}

.new-coin-project-card__stats strong.is-long-value {
  font-size: 14px;
  letter-spacing: -.01em;
}

.new-coin-project-card__stats strong.is-very-long-value {
  font-size: 12px;
  letter-spacing: -.02em;
}

.new-coin-project-card__row {
  align-items: center;
  display: grid;
  font-size: 11px;
  gap: 7px;
  grid-template-columns: auto minmax(32px, 1fr) auto;
  height: 26px;
  min-width: 0;
}

.new-coin-project-card__row strong {
  font-size: 11px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.new-coin-project-card__row:nth-of-type(4) {
  grid-template-columns: auto minmax(0, 1fr);
}

.new-coin-project-card__row:nth-of-type(4) strong {
  text-align: right;
}

.new-coin-project-card__progress {
  background: var(--new-coin-progress-track);
  border-radius: 4px;
  height: 6px;
  overflow: hidden;
}

.new-coin-project-card__progress i {
  background: var(--new-coin-signal);
  border-radius: inherit;
  display: block;
  height: 100%;
}

.new-coin-project-card__countdown {
  align-items: center;
  background: var(--new-coin-countdown);
  border-radius: 10px;
  display: flex;
  gap: 8px;
  height: 40px;
  justify-content: space-between;
  min-width: 0;
  padding: 0 10px;
}

.new-coin-project-card__countdown small,
.new-coin-project-card__countdown strong {
  font-size: 10px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.new-coin-project-card__countdown strong {
  font-family: var(--font-numeric);
  font-weight: 700;
}

.new-coin-project-card__action {
  align-items: center;
  background: var(--new-coin-action);
  border-radius: 13px;
  color: var(--new-coin-action-ink);
  display: flex;
  font-size: 15px;
  font-weight: 750;
  height: 42px;
  justify-content: center;
  line-height: 22px;
}

@media (max-width: 340px) {
  .new-coin-project-card__hit {
    padding-inline: 12px;
  }

  .new-coin-project-card__stats strong {
    font-size: 15px;
  }

  .new-coin-project-card__stats strong.is-long-value {
    font-size: 13px;
  }

  .new-coin-project-card__stats strong.is-very-long-value {
    font-size: 11px;
  }
}
</style>
