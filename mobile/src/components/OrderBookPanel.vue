<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChartNoAxesCombined, LoaderCircle } from 'lucide-vue-next'
import { formatAmount, formatPrice } from '@/core/format'
import type { OrderBookLevel } from '@/core/types'

const props = withDefaults(defineProps<{
  bids: OrderBookLevel[]
  asks: OrderBookLevel[]
  currentPrice: number
  baseAsset?: string
  quoteAsset?: string
  loading?: boolean
}>(), {
  baseAsset: '',
  quoteAsset: '',
  loading: false,
})

const { t } = useI18n()
const visibleAsks = computed(() => props.asks.slice(0, 6).reverse())
const visibleBids = computed(() => props.bids.slice(0, 6))
const maxQuantity = computed(() => Math.max(
  1,
  ...visibleBids.value.map((item) => item.quantity),
  ...visibleAsks.value.map((item) => item.quantity),
))
const hasRows = computed(() => props.bids.length > 0 || props.asks.length > 0)

function width(quantity: number): string {
  return `${Math.max(7, (quantity / maxQuantity.value) * 100)}%`
}
</script>

<template>
  <section class="order-book" :aria-label="t('orderBook.title')" :aria-busy="loading">
    <header>
      <strong>{{ t('orderBook.title') }}</strong>
      <span class="order-book__columns">
        <b>
          {{ t('marketDetail.price') }}
          <small v-if="quoteAsset">{{ quoteAsset }}</small>
        </b>
        <b>
          {{ t('marketDetail.quantity') }}
          <small v-if="baseAsset">{{ baseAsset }}</small>
        </b>
      </span>
    </header>
    <template v-if="hasRows">
      <div class="order-book__rows order-book__rows--asks">
        <div
          v-for="(item, index) in visibleAsks"
          :key="`ask-${item.price}-${index}`"
          class="order-book__row"
          data-book-side="ask"
        >
          <i class="order-book__bar order-book__bar--ask" :style="{ width: width(item.quantity) }" />
          <span class="down numeric">{{ formatPrice(item.price) }}</span>
          <span class="numeric">{{ formatAmount(item.quantity) }}</span>
        </div>
      </div>
      <div class="order-book__last">
        <strong class="numeric">{{ currentPrice > 0 ? formatPrice(currentPrice) : '--' }}</strong>
        <span>{{ t('orderBook.lastPrice') }}</span>
      </div>
      <div class="order-book__rows">
        <div
          v-for="(item, index) in visibleBids"
          :key="`bid-${item.price}-${index}`"
          class="order-book__row"
          data-book-side="bid"
        >
          <i class="order-book__bar order-book__bar--bid" :style="{ width: width(item.quantity) }" />
          <span class="up numeric">{{ formatPrice(item.price) }}</span>
          <span class="numeric">{{ formatAmount(item.quantity) }}</span>
        </div>
      </div>
    </template>
    <div v-else class="order-book__state" role="status">
      <LoaderCircle v-if="loading" :size="19" class="spin" />
      <ChartNoAxesCombined v-else :size="20" />
      <span>{{ loading ? t('common.loading') : t('common.marketUnavailable') }}</span>
    </div>
  </section>
</template>

<style scoped>
.order-book {
  background: var(--surface-elevated);
  color: var(--ink);
  min-height: 0;
  min-width: 0;
  overflow: hidden;
  padding: 14px;
}

.order-book header {
  align-items: center;
  border-bottom: 1px solid var(--line);
  color: var(--muted);
  display: flex;
  font-size: 10px;
  justify-content: space-between;
  margin-bottom: 8px;
  min-height: 34px;
}

.order-book header strong {
  color: var(--ink);
  font-size: 12px;
}

.order-book__columns {
  display: grid;
  gap: 8px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  min-width: 0;
  width: 66%;
}

.order-book__columns b {
  color: var(--muted);
  display: grid;
  font-size: 9px;
  font-weight: 650;
  gap: 2px;
  min-width: 0;
}

.order-book__columns b:last-child {
  text-align: right;
}

.order-book__columns small {
  color: var(--muted);
  font-size: 8px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.order-book__rows {
  display: grid;
  gap: 2px;
}

.order-book__row {
  display: grid;
  font-size: 11px;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  min-height: 22px;
  overflow: hidden;
  padding: 3px 0;
  position: relative;
}

.order-book__row span {
  min-width: 0;
  overflow: hidden;
  position: relative;
  text-overflow: ellipsis;
  white-space: nowrap;
  z-index: 1;
}

.order-book__row span:last-child {
  color: var(--muted-strong);
  text-align: right;
}

.order-book__bar {
  height: 100%;
  position: absolute;
  right: 0;
  top: 0;
}

.order-book__bar--ask {
  background: color-mix(in srgb, var(--negative) 18%, transparent);
  border-right: 2px solid var(--negative);
}

.order-book__bar--bid {
  background: color-mix(in srgb, var(--positive) 18%, transparent);
  border-right: 2px solid var(--positive);
}

.order-book__last {
  align-items: baseline;
  border-block: 1px solid var(--line);
  display: flex;
  gap: 8px;
  margin: 6px 0;
  min-height: 42px;
  padding: 9px 0;
}

.order-book__last strong {
  color: var(--positive);
  font-size: 16px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.order-book__last span {
  color: var(--muted);
  font-size: 10px;
}

.order-book__state {
  align-items: center;
  color: var(--muted);
  display: flex;
  flex-direction: column;
  font-size: 11px;
  gap: 8px;
  justify-content: center;
  min-height: 220px;
  padding-inline: 8px;
  text-align: center;
}

@media (max-width: 340px) {
  .order-book {
    padding-inline: 8px;
  }

  .order-book__row {
    font-size: 9px;
    gap: 4px;
  }

  .order-book__last {
    gap: 5px;
  }

  .order-book__last strong {
    font-size: 14px;
  }
}

.spin {
  animation: spin .8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

@media (prefers-reduced-motion: reduce) {
  .spin {
    animation: none;
  }
}
</style>
