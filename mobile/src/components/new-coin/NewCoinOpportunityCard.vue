<script setup lang="ts">
import { computed } from 'vue'
import { ArrowUpRight } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import AssetMark from '@/components/AssetMark.vue'
import { formatFinancialAmount } from '@/core/financialDisplay'
import { formatCompact } from '@/core/format'
import type { NewCoinOpportunity } from '@/core/newCoinPresentation'

const props = defineProps<{
  opportunity: NewCoinOpportunity
  now: number
}>()

defineEmits<{
  trade: []
}>()

const { t, locale } = useI18n()
const project = computed(() => props.opportunity.project)
const ticker = computed(() => props.opportunity.ticker)
const price = computed(() => ticker.value.lastPriceText
  ? formatFinancialAmount(ticker.value.lastPriceText, locale.value, { assetSymbol: ticker.value.quote })
  : t('newCoin.unavailableValue'))
const change = computed(() => new Intl.NumberFormat(locale.value, {
  maximumFractionDigits: 2,
  minimumFractionDigits: 2,
  signDisplay: 'exceptZero',
}).format(ticker.value.changePercent))
const volume = computed(() => Number.isFinite(ticker.value.volume)
  ? formatCompact(ticker.value.volume)
  : t('newCoin.unavailableValue'))
const launchTiming = computed(() => {
  const listedAt = project.value.listedAt
  if (!listedAt) return t('newCoin.unavailableValue')
  if (listedAt > props.now) return t('newCoin.upcoming')
  return t('newCoin.listedDays', {
    days: Math.max(0, Math.floor((props.now - listedAt) / 86_400_000)),
  })
})
const marketDescriptor = computed(() => t('newCoin.spotMarketDescriptor', { quote: ticker.value.quote }))
const projectName = computed(() => project.value.name || t('newCoin.projectNameUnavailable'))
</script>

<template>
  <article class="new-coin-opportunity-card">
    <header>
      <AssetMark :symbol="project.symbol" :src="project.logoUrl" :size="36" />
      <span class="new-coin-opportunity-card__identity">
        <strong>{{ ticker.symbol }}</strong>
        <small>{{ projectName }}</small>
      </span>
      <b>{{ launchTiming }}</b>
    </header>

    <dl>
      <div><dt>{{ t('newCoin.latestPrice') }}</dt><dd>{{ price }}</dd></div>
      <div><dt>{{ t('newCoin.change24h') }}</dt><dd :class="{ up: ticker.changePercent > 0, down: ticker.changePercent < 0 }">{{ change }}%</dd></div>
      <div><dt>{{ t('newCoin.volume24h') }}</dt><dd>{{ volume }}</dd></div>
    </dl>

    <footer>
      <small>{{ marketDescriptor }}</small>
      <button type="button" @click="$emit('trade')">
        {{ t('newCoin.tradeNow') }}
        <ArrowUpRight :size="13" />
      </button>
    </footer>
  </article>
</template>

<style scoped>
.new-coin-opportunity-card {
  background: var(--new-coin-card);
  border: 1px solid var(--new-coin-card-border);
  border-radius: 18px;
  box-sizing: border-box;
  color: var(--new-coin-ink);
  display: grid;
  gap: 8px;
  height: 140px;
  padding: 12px;
  width: 100%;
}

.new-coin-opportunity-card header {
  align-items: center;
  display: flex;
  height: 36px;
  min-width: 0;
}

.new-coin-opportunity-card__identity {
  display: grid;
  flex: 1;
  gap: 2px;
  margin-left: 9px;
  min-width: 0;
}

.new-coin-opportunity-card__identity strong {
  font-size: 14px;
  font-weight: 700;
  line-height: 17px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.new-coin-opportunity-card__identity small {
  color: var(--new-coin-muted);
  font-size: 9px;
  line-height: 11px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.new-coin-opportunity-card header > b {
  align-items: center;
  background: var(--new-coin-status);
  border-radius: 8px;
  color: var(--new-coin-signal);
  display: inline-flex;
  font-size: 9px;
  font-weight: 700;
  height: 22px;
  padding: 0 8px;
}

.new-coin-opportunity-card dl {
  display: grid;
  gap: 8px;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  height: 36px;
  margin: 0;
}

.new-coin-opportunity-card dl div {
  display: grid;
  min-width: 0;
}

.new-coin-opportunity-card dt {
  color: var(--new-coin-muted);
  font-size: 9px;
  line-height: 12px;
}

.new-coin-opportunity-card dd {
  font-family: var(--font-numeric);
  font-size: 12px;
  font-weight: 700;
  line-height: 15px;
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.new-coin-opportunity-card dd.up {
  color: var(--positive);
}

.new-coin-opportunity-card dd.down {
  color: var(--negative);
}

.new-coin-opportunity-card footer {
  align-items: center;
  display: flex;
  height: 24px;
  justify-content: space-between;
  min-width: 0;
}

.new-coin-opportunity-card footer small {
  color: var(--new-coin-muted);
  font-size: 9px;
  line-height: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.new-coin-opportunity-card footer button {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: 12px;
  color: var(--new-coin-action-ink);
  display: inline-flex;
  flex: 0 0 auto;
  font-size: 10px;
  font-weight: 700;
  gap: 2px;
  height: 44px;
  isolation: isolate;
  justify-content: center;
  margin-right: -6px;
  min-width: 76px;
  padding: 10px 6px;
  position: relative;
}

.new-coin-opportunity-card footer button::before {
  background: var(--new-coin-action);
  border-radius: 12px;
  content: '';
  height: 24px;
  position: absolute;
  width: 64px;
  z-index: -1;
}

.new-coin-opportunity-card footer button:focus-visible {
  outline: 2px solid var(--focus-ring);
  outline-offset: 1px;
}

@media (max-width: 340px) {
  .new-coin-opportunity-card {
    padding-inline: 10px;
  }

  .new-coin-opportunity-card dl {
    gap: 4px;
  }
}
</style>
