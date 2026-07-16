<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { formatAmount, formatPrice } from '@/core/format'
import type { OrderBookLevel } from '@/core/types'

const props = defineProps<{ bids: OrderBookLevel[]; asks: OrderBookLevel[]; currentPrice: number }>()
const { t } = useI18n()
const maxQuantity = computed(() => Math.max(1, ...props.bids.map((item) => item.quantity), ...props.asks.map((item) => item.quantity)))

function width(quantity: number): string {
  return `${Math.max(7, (quantity / maxQuantity.value) * 100)}%`
}
</script>

<template>
  <section class="order-book">
    <header><strong>{{ t('orderBook.title') }}</strong><span>{{ t('orderBook.priceQuantity') }}</span></header>
    <div class="order-book__rows order-book__rows--asks">
      <div v-for="item in asks.slice(0, 6).reverse()" :key="`ask-${item.price}`" class="order-book__row">
        <i class="order-book__bar order-book__bar--ask" :style="{ width: width(item.quantity) }" />
        <span class="down">{{ formatPrice(item.price) }}</span><span>{{ formatAmount(item.quantity) }}</span>
      </div>
    </div>
    <div class="order-book__last"><strong>{{ formatPrice(currentPrice) }}</strong><span>{{ t('orderBook.lastPrice') }}</span></div>
    <div class="order-book__rows">
      <div v-for="item in bids.slice(0, 6)" :key="`bid-${item.price}`" class="order-book__row">
        <i class="order-book__bar order-book__bar--bid" :style="{ width: width(item.quantity) }" />
        <span class="up">{{ formatPrice(item.price) }}</span><span>{{ formatAmount(item.quantity) }}</span>
      </div>
    </div>
  </section>
</template>

<style scoped>
.order-book { background: var(--dark-surface); color: #e5e7eb; min-height: 0; padding: 14px; }
.order-book header { color: #9ca3af; display: flex; font-size: 11px; justify-content: space-between; margin-bottom: 10px; }
.order-book header strong { color: #f3f4f6; font-size: 14px; }
.order-book__rows { display: grid; gap: 3px; }
.order-book__row { display: grid; font-size: 12px; grid-template-columns: 1fr 1fr; overflow: hidden; padding: 2px 0; position: relative; }
.order-book__row span { position: relative; z-index: 1; }
.order-book__row span:last-child { color: #c7cbd1; text-align: right; }
.order-book__bar { height: 100%; opacity: .3; position: absolute; right: 0; top: 0; }
.order-book__bar--ask { background: #74283d; }
.order-book__bar--bid { background: #164d38; }
.order-book__last { align-items: baseline; display: flex; gap: 8px; padding: 9px 0; }
.order-book__last strong { color: #00b86b; font-size: 17px; }
.order-book__last span { color: #9ca3af; font-size: 11px; }
.up { color: #00c076; }.down { color: #f05b7c; }
</style>
