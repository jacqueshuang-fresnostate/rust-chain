<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChartNoAxesCombined, ChevronDown, ListFilter, LoaderCircle } from 'lucide-vue-next'
import { formatAmount, formatPrice } from '@/core/format'
import type { OrderBookLevel } from '@/core/types'

const props = withDefaults(defineProps<{
  bids: OrderBookLevel[]
  asks: OrderBookLevel[]
  currentPrice: number
  baseAsset?: string
  quoteAsset?: string
  loading?: boolean
  layout?: 'stacked' | 'split' | 'paired' | 'matrix' | 'mini'
  miniLevels?: number
  miniAskLevels?: number
  miniBidLevels?: number
  miniPrecision?: string
  showMiniPrecision?: boolean
}>(), {
  baseAsset: '',
  quoteAsset: '',
  loading: false,
  layout: 'stacked',
  miniLevels: 5,
  miniAskLevels: undefined,
  miniBidLevels: undefined,
  miniPrecision: '0.01',
  showMiniPrecision: true,
})

const { t } = useI18n()
const visibleAsks = computed(() => props.asks.slice(0, 6).reverse())
const splitAsks = computed(() => props.asks.slice(0, 6))
const visibleBids = computed(() => props.bids.slice(0, 6))
const miniAsks = computed(() => props.asks.slice(0, 5).reverse())
const miniBids = computed(() => props.bids.slice(0, 5))
const renderedMiniAsks = computed(() => (
  (props.miniAskLevels ?? props.miniLevels) === 5
    ? miniAsks.value
    : props.asks.slice(0, Math.max(1, props.miniAskLevels ?? props.miniLevels)).reverse()
))
const renderedMiniBids = computed(() => (
  (props.miniBidLevels ?? props.miniLevels) === 5
    ? miniBids.value
    : props.bids.slice(0, Math.max(1, props.miniBidLevels ?? props.miniLevels))
))
const matrixMode = computed(() => props.layout === 'paired' || props.layout === 'matrix')
const matrixBids = computed(() => props.bids.slice(0, 7))
const matrixAsks = computed(() => props.asks.slice(0, 7))
const matrixRows = computed(() => Array.from({ length: 7 }, (_, index) => ({
  bid: matrixBids.value[index],
  ask: matrixAsks.value[index],
})))
const maxQuantity = computed(() => Math.max(
  1,
  ...props.bids.slice(0, 7).map((item) => item.quantity),
  ...props.asks.slice(0, 7).map((item) => item.quantity),
))
const hasRows = computed(() => props.bids.length > 0 || props.asks.length > 0)
const miniBidRatio = computed(() => {
  const bidTotal = renderedMiniBids.value.reduce((total, item) => total + item.quantity, 0)
  const askTotal = renderedMiniAsks.value.reduce((total, item) => total + item.quantity, 0)
  const total = bidTotal + askTotal
  return total > 0 ? Math.round((bidTotal / total) * 100) : 50
})

function width(quantity: number): string {
  return `${Math.max(7, (quantity / maxQuantity.value) * 100)}%`
}

function matrixWidth(quantity: number): string {
  return `${Math.max(4, (quantity / maxQuantity.value) * 50)}%`
}
</script>

<template>
  <section
    class="order-book"
    :class="{
      'order-book--split': layout === 'split',
      'order-book--matrix': matrixMode,
      'order-book--mini': layout === 'mini',
    }"
    :data-layout="layout"
    :aria-label="t('orderBook.title')"
    :aria-busy="loading"
  >
    <template v-if="layout === 'mini'">
      <div class="order-book__mini" role="table" :aria-label="t('orderBook.title')">
        <div class="order-book__mini-header" role="row">
          <span role="columnheader">{{ t('marketDetail.price') }}</span>
          <span role="columnheader">{{ t('marketDetail.quantity') }}</span>
        </div>
        <template v-if="hasRows">
          <div
            v-for="(item, index) in renderedMiniAsks"
            :key="`mini-ask-${item.price}-${index}`"
            class="order-book__mini-row"
            role="row"
            data-book-side="ask"
          >
            <i
              class="order-book__mini-depth order-book__mini-depth--ask"
              :style="{ width: width(item.quantity) }"
              aria-hidden="true"
            />
            <span class="down numeric" role="cell">{{ formatPrice(item.price) }}</span>
            <span class="numeric" role="cell">{{ formatAmount(item.quantity) }}</span>
          </div>
          <div class="order-book__mini-mid" role="row">
            <strong class="up numeric" role="cell">{{ currentPrice > 0 ? formatPrice(currentPrice) : '--' }}</strong>
            <small class="numeric" role="cell">
              {{ currentPrice > 0 ? `≈ ${formatPrice(currentPrice)} ${quoteAsset}` : t('common.marketUnavailable') }}
            </small>
          </div>
          <div
            v-for="(item, index) in renderedMiniBids"
            :key="`mini-bid-${item.price}-${index}`"
            class="order-book__mini-row"
            role="row"
            data-book-side="bid"
          >
            <i
              class="order-book__mini-depth order-book__mini-depth--bid"
              :style="{ width: width(item.quantity) }"
              aria-hidden="true"
            />
            <span class="up numeric" role="cell">{{ formatPrice(item.price) }}</span>
            <span class="numeric" role="cell">{{ formatAmount(item.quantity) }}</span>
          </div>
          <div class="order-book__mini-ratio" role="row" :style="{ '--mini-bid-ratio': `${miniBidRatio}%` }">
            <span class="up numeric" role="cell">B&nbsp;&nbsp;{{ miniBidRatio }}%</span>
            <span class="down numeric" role="cell">{{ 100 - miniBidRatio }}%&nbsp;&nbsp;S</span>
          </div>
          <div v-if="showMiniPrecision" class="order-book__mini-precision" aria-hidden="true">
            <span class="order-book__mini-precision-value">
              <span class="numeric">{{ miniPrecision }}</span>
              <ChevronDown :size="11" />
            </span>
            <ListFilter :size="13" />
          </div>
        </template>
        <div v-else class="order-book__mini-state" role="status">
          <LoaderCircle v-if="loading" :size="18" class="spin" />
          <ChartNoAxesCombined v-else :size="19" />
          <span>{{ loading ? t('common.loading') : t('common.marketUnavailable') }}</span>
        </div>
      </div>
    </template>
    <template v-else-if="matrixMode">
      <div class="order-book__matrix" role="table" :aria-label="t('orderBook.title')">
        <div class="order-book__matrix-header" role="row">
          <span role="columnheader">{{ t('orderBook.buySide', { asset: baseAsset }) }}</span>
          <span role="columnheader" aria-colspan="2">
            {{ t('marketDetail.price') }}<small v-if="quoteAsset"> ({{ quoteAsset }})</small>
          </span>
          <span role="columnheader">{{ t('orderBook.sellSide', { asset: baseAsset }) }}</span>
        </div>
        <template v-if="hasRows">
          <div
            v-for="(row, index) in matrixRows"
            :key="`matrix-${row.bid?.price ?? 'empty'}-${row.ask?.price ?? 'empty'}-${index}`"
            class="order-book__matrix-row"
            role="row"
          >
            <i
              v-if="row.bid"
              class="order-book__matrix-depth order-book__matrix-depth--bid"
              :style="{ width: matrixWidth(row.bid.quantity) }"
              aria-hidden="true"
            />
            <i
              v-if="row.ask"
              class="order-book__matrix-depth order-book__matrix-depth--ask"
              :style="{ width: matrixWidth(row.ask.quantity) }"
              aria-hidden="true"
            />
            <span class="numeric" role="cell">{{ row.bid ? formatAmount(row.bid.quantity) : '--' }}</span>
            <span class="up numeric" role="cell">{{ row.bid ? formatPrice(row.bid.price) : '--' }}</span>
            <span class="down numeric" role="cell">{{ row.ask ? formatPrice(row.ask.price) : '--' }}</span>
            <span class="numeric" role="cell">{{ row.ask ? formatAmount(row.ask.quantity) : '--' }}</span>
          </div>
        </template>
        <div v-if="!hasRows" class="order-book__matrix-state" role="status">
          <LoaderCircle v-if="loading" :size="19" class="spin" />
          <ChartNoAxesCombined v-else :size="20" />
          <span>{{ loading ? t('common.loading') : t('common.marketUnavailable') }}</span>
        </div>
      </div>
    </template>
    <template v-else>
      <header>
        <strong>{{ t('orderBook.title') }}</strong>
        <span v-if="layout === 'split'" class="order-book__split-last">
          <small>{{ t('orderBook.lastPrice') }}</small>
          <b class="numeric">{{ currentPrice > 0 ? formatPrice(currentPrice) : '--' }}</b>
        </span>
        <span v-else class="order-book__columns">
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
        <div v-if="layout === 'split'" class="order-book__split">
          <div class="order-book__split-label order-book__split-label--bid">
            {{ t('orderBook.buySide', { asset: baseAsset }) }}
          </div>
          <span class="order-book__split-divider" aria-hidden="true" />
          <div class="order-book__split-label order-book__split-label--ask">
            {{ t('orderBook.sellSide', { asset: baseAsset }) }}
          </div>

          <div class="order-book__side order-book__side--bid">
            <div class="order-book__side-columns">
              <span>{{ t('marketDetail.quantity') }}</span>
              <span>{{ t('marketDetail.price') }}<small v-if="quoteAsset"> {{ quoteAsset }}</small></span>
            </div>
            <div class="order-book__rows">
              <div
                v-for="(item, index) in visibleBids"
                :key="`split-bid-${item.price}-${index}`"
                class="order-book__row"
                data-book-side="bid"
              >
                <i
                  class="order-book__bar order-book__bar--bid"
                  :style="{ width: width(item.quantity) }"
                  aria-hidden="true"
                />
                <span class="numeric">{{ formatAmount(item.quantity) }}</span>
                <span class="up numeric">{{ formatPrice(item.price) }}</span>
              </div>
            </div>
          </div>
          <span class="order-book__split-divider order-book__split-divider--body" aria-hidden="true" />
          <div class="order-book__side order-book__side--ask">
            <div class="order-book__side-columns">
              <span>{{ t('marketDetail.price') }}<small v-if="quoteAsset"> {{ quoteAsset }}</small></span>
              <span>{{ t('marketDetail.quantity') }}</span>
            </div>
            <div class="order-book__rows">
              <div
                v-for="(item, index) in splitAsks"
                :key="`split-ask-${item.price}-${index}`"
                class="order-book__row"
                data-book-side="ask"
              >
                <i
                  class="order-book__bar order-book__bar--ask order-book__bar--from-left"
                  :style="{ width: width(item.quantity) }"
                  aria-hidden="true"
                />
                <span class="down numeric">{{ formatPrice(item.price) }}</span>
                <span class="numeric">{{ formatAmount(item.quantity) }}</span>
              </div>
            </div>
          </div>
        </div>
        <template v-else>
          <div class="order-book__rows order-book__rows--asks">
            <div
              v-for="(item, index) in visibleAsks"
              :key="`ask-${item.price}-${index}`"
              class="order-book__row"
              data-book-side="ask"
            >
              <i
                class="order-book__bar order-book__bar--ask"
                :style="{ width: width(item.quantity) }"
                aria-hidden="true"
              />
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
              <i
                class="order-book__bar order-book__bar--bid"
                :style="{ width: width(item.quantity) }"
                aria-hidden="true"
              />
              <span class="up numeric">{{ formatPrice(item.price) }}</span>
              <span class="numeric">{{ formatAmount(item.quantity) }}</span>
            </div>
          </div>
        </template>
      </template>
      <div v-else class="order-book__state" role="status">
        <LoaderCircle v-if="loading" :size="19" class="spin" />
        <ChartNoAxesCombined v-else :size="20" />
        <span>{{ loading ? t('common.loading') : t('common.marketUnavailable') }}</span>
      </div>
    </template>
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

.order-book__split-last {
  align-items: flex-end;
  display: grid;
  gap: 1px;
  justify-items: end;
  min-width: 0;
}

.order-book__split-last small {
  color: var(--muted);
  font-size: 8px;
}

.order-book__split-last b {
  color: var(--ink);
  font-size: 12px;
  max-width: 160px;
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

.order-book__bar--from-left {
  border-left: 2px solid var(--negative);
  border-right: 0;
  left: 0;
  right: auto;
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

.order-book--split {
  background:
    linear-gradient(180deg, color-mix(in srgb, var(--ink) 3%, transparent), transparent 64px),
    var(--surface);
  padding: 0 10px 12px;
}

.order-book--split header {
  background: linear-gradient(180deg, color-mix(in srgb, var(--ink) 4%, var(--surface)), var(--surface));
  box-shadow: inset 0 1px 0 var(--line);
  margin: 0 -10px 8px;
  padding: 0 12px;
}

.order-book__split {
  display: grid;
  gap: 0 7px;
  grid-template-columns: minmax(0, 1fr) 1px minmax(0, 1fr);
  min-width: 0;
}

.order-book__split-label {
  align-items: center;
  display: flex;
  font-size: 9px;
  font-weight: 760;
  min-height: 28px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.order-book__split-label--bid {
  color: var(--positive);
  grid-column: 1;
}

.order-book__split-label--ask {
  color: var(--negative);
  grid-column: 3;
  justify-content: flex-end;
}

.order-book__split-divider {
  background: var(--line);
  grid-column: 2;
  grid-row: 1;
  width: 1px;
}

.order-book__split-divider--body {
  grid-row: 2;
}

.order-book__side {
  min-width: 0;
}

.order-book__side--bid {
  grid-column: 1;
  grid-row: 2;
}

.order-book__side--ask {
  grid-column: 3;
  grid-row: 2;
}

.order-book__side-columns {
  color: var(--muted);
  display: grid;
  font-size: 8px;
  gap: 5px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  min-height: 26px;
}

.order-book__side-columns span {
  align-self: center;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.order-book__side-columns span:last-child {
  text-align: right;
}

.order-book--split .order-book__rows {
  gap: 1px;
}

.order-book--split .order-book__row {
  font-size: 9px;
  gap: 5px;
  min-height: 27px;
  padding: 5px 2px;
}

.order-book--split .order-book__side--bid .order-book__row span:first-of-type,
.order-book--split .order-book__side--ask .order-book__row span:last-child {
  color: var(--muted-strong);
}

.order-book--split .order-book__side--bid .order-book__row span:last-child {
  color: var(--positive);
}

.order-book--split .order-book__state {
  min-height: 210px;
}

.order-book--mini {
  background: transparent;
  height: 424px;
  min-height: 424px;
  padding: 0;
}

.order-book__mini {
  display: grid;
  min-width: 0;
  position: relative;
}

.order-book__mini-header,
.order-book__mini-row,
.order-book__mini-ratio {
  display: grid;
  grid-template-columns: minmax(0, 1.15fr) minmax(0, .85fr);
  min-width: 0;
}

.order-book__mini-header {
  align-items: center;
  color: var(--muted);
  font-size: 9px;
  height: 26px;
}

.order-book__mini-header span:last-child,
.order-book__mini-row span:last-child,
.order-book__mini-ratio span:last-child {
  text-align: right;
}

.order-book__mini-row {
  align-items: center;
  font-size: 10px;
  height: 30px;
  overflow: hidden;
  position: relative;
}

.order-book__mini-row > span {
  min-width: 0;
  overflow: hidden;
  position: relative;
  text-overflow: ellipsis;
  white-space: nowrap;
  z-index: 1;
}

.order-book__mini-depth {
  bottom: 0;
  max-width: 100%;
  min-width: 4px;
  position: absolute;
  right: 0;
  top: 0;
}

.order-book__mini-depth--ask {
  background: color-mix(in srgb, var(--negative) 18%, transparent);
}

.order-book__mini-depth--bid {
  background: color-mix(in srgb, var(--positive) 17%, transparent);
}

.order-book__mini-mid {
  align-content: center;
  display: grid;
  gap: 2px;
  height: 48px;
  min-width: 0;
}

.order-book__mini-mid strong {
  font-size: 20px;
  line-height: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.order-book__mini-mid small {
  color: var(--muted);
  font-size: 8px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.order-book__mini-ratio {
  align-items: center;
  font-size: 9px;
  height: 22px;
}

.order-book__mini-precision {
  align-items: center;
  display: flex;
  font-size: 10px;
  gap: 5px;
  height: 24px;
  justify-content: space-between;
  justify-self: stretch;
  margin-top: 2px;
  padding: 0 3px;
}

.order-book__mini-precision-value {
  align-items: center;
  background: var(--surface-2);
  border-radius: 4px;
  display: inline-flex;
  gap: 3px;
  height: 22px;
  padding: 0 6px;
}

.order-book__mini-state {
  align-items: center;
  color: var(--muted);
  display: flex;
  flex-direction: column;
  font-size: 10px;
  gap: 8px;
  height: 398px;
  justify-content: center;
  padding: 12px 4px;
  text-align: center;
}

.order-book--matrix {
  background: var(--surface);
  height: 272px;
  min-height: 272px;
  padding: 0;
}

.order-book__matrix {
  height: 272px;
  min-width: 0;
  position: relative;
}

.order-book__matrix-header,
.order-book__matrix-row {
  display: grid;
  grid-template-columns: minmax(0, .8fr) minmax(0, 1.05fr) minmax(0, 1.05fr) minmax(0, .8fr);
  min-width: 0;
}

.order-book__matrix-header {
  align-items: center;
  color: var(--muted);
  font-size: 9px;
  height: 34px;
  padding: 0 12px;
}

.order-book__matrix-header span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.order-book__matrix-header span:first-child {
  color: var(--positive);
  grid-column: 1;
}

.order-book__matrix-header span:nth-child(2) {
  grid-column: 2 / 4;
  text-align: center;
}

.order-book__matrix-header span:last-child {
  color: var(--negative);
  grid-column: 4;
  text-align: right;
}

.order-book__matrix-header small {
  font-size: inherit;
}

.order-book__matrix-row {
  align-items: center;
  border-top: 1px solid color-mix(in srgb, var(--line) 48%, transparent);
  font-size: 11px;
  height: 34px;
  overflow: hidden;
  padding: 0 10px;
  position: relative;
}

.order-book__matrix-row > span {
  min-width: 0;
  overflow: hidden;
  position: relative;
  text-overflow: ellipsis;
  white-space: nowrap;
  z-index: 1;
}

.order-book__matrix-row > span:nth-of-type(2),
.order-book__matrix-row > span:nth-of-type(3) {
  text-align: center;
}

.order-book__matrix-row > span:last-child {
  text-align: right;
}

.order-book__matrix-depth {
  bottom: 0;
  display: block;
  max-width: 50%;
  position: absolute;
  top: 0;
}

.order-book__matrix-depth--bid {
  background: color-mix(in srgb, var(--positive) 13%, transparent);
  right: 50%;
}

.order-book__matrix-depth--ask {
  background: color-mix(in srgb, var(--negative) 13%, transparent);
  left: 50%;
}

.order-book__matrix-state {
  align-items: center;
  background: color-mix(in srgb, var(--surface) 90%, transparent);
  color: var(--muted);
  display: flex;
  flex-direction: column;
  font-size: 11px;
  gap: 8px;
  inset: 34px 0 0;
  justify-content: center;
  padding: 12px;
  position: absolute;
  text-align: center;
  z-index: 2;
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

  .order-book--split {
    padding-inline: 6px;
  }

  .order-book--split header {
    margin-inline: -6px;
    padding-inline: 8px;
  }

  .order-book__split {
    gap: 0 5px;
  }

  .order-book--split .order-book__row {
    font-size: 8px;
    gap: 3px;
  }

  .order-book--matrix {
    padding: 0;
  }

  .order-book--mini {
    height: 424px;
    min-height: 424px;
    padding: 0;
  }

  .order-book__mini-row {
    font-size: 8px;
  }

  .order-book__mini-mid strong {
    font-size: 17px;
  }

  .order-book__matrix-header {
    font-size: 8px;
    padding-inline: 7px;
  }

  .order-book__matrix-row {
    font-size: 9px;
    padding-inline: 6px;
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
