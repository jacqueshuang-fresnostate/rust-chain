<script setup lang="ts">
import type { TradePrint } from '@/core/types'
import { currentIntlLocale } from '@/i18n'
import MarketProvenanceLabel from './MarketProvenanceLabel.vue'

defineProps<{ trade: TradePrint; formatValue: (value: number) => string }>()

function formatTradeTime(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return '--'
  const timestamp = value < 1_000_000_000_000 ? value * 1000 : value
  return new Intl.DateTimeFormat(currentIntlLocale(), {
    hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: false,
  }).format(new Date(timestamp))
}
</script>

<template>
  <div class="spot-recent-trades__row">
    <strong class="numeric" :class="trade.side === 'buy' ? 'positive' : 'negative'">
      {{ formatValue(trade.price) }}
      <MarketProvenanceLabel :provenance="trade" />
    </strong>
    <span class="numeric">{{ formatValue(trade.quantity) }}</span>
    <time class="numeric" :datetime="new Date(trade.time).toISOString()">{{ formatTradeTime(trade.time) }}</time>
  </div>
</template>

<style scoped>
.spot-recent-trades__row {
  display: grid;
  gap: 8px;
  grid-template-columns: minmax(0, 1fr) minmax(0, .8fr) minmax(72px, .72fr);
  min-width: 0;
  align-items: center;
  border-bottom: 1px solid var(--line);
  font-size: 10px;
  min-height: 32px;
}
.spot-recent-trades__row > :nth-child(n + 2) { text-align: right; }
.spot-recent-trades__row strong,
.spot-recent-trades__row span,
.spot-recent-trades__row time {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.spot-recent-trades__row time { color: var(--muted); }
</style>
