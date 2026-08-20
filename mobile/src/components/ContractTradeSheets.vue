<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Check, Info, Search, Star, TriangleAlert, X } from 'lucide-vue-next'
import AssetMark from '@/components/AssetMark.vue'
import { formatPrice, normalizeSymbol, splitSymbol } from '@/core/format'
import { useModalDialog } from '@/core/modalDialog'
import { useMarketFavoritesStore } from '@/stores/marketFavorites'
import { useMarketStore } from '@/stores/market'
import type { MarginOrderType, MarginProduct } from '@/core/types'

type ContractSheet = 'pair' | 'leverage' | 'marginMode' | 'orderType' | null
type PairFilter = 'favorites' | 'all' | 'mainstream'
type MarginMode = 'cross' | 'isolated'

const props = defineProps<{
  open: ContractSheet
  pairSymbol: string
  product?: MarginProduct
  products: MarginProduct[]
  leverage: number
  marginMode: MarginMode
  orderType: MarginOrderType | null
  saving: boolean
  error?: string
  productsLoading: boolean
  productsError: boolean
}>()

const emit = defineEmits<{
  close: []
  selectPair: [symbol: string]
  applyLeverage: [leverage: number]
  applyMarginMode: [mode: MarginMode]
  selectOrderType: [orderType: MarginOrderType]
  retryProducts: []
}>()

const { t } = useI18n()
const marketStore = useMarketStore()
const marketFavorites = useMarketFavoritesStore()
const dialog = ref<HTMLElement | null>(null)
const searchQuery = ref('')
const pairFilter = ref<PairFilter>('all')
const draftLeverage = ref(props.leverage)
const draftMarginMode = ref<MarginMode>(props.marginMode)
const dialogOpen = computed(() => props.open !== null)
const { trapFocus } = useModalDialog(dialogOpen, dialog, '[data-dialog-initial]')

const compactPair = computed(() => props.pairSymbol.replace(/[\/_-]/g, '').toUpperCase())
const selectedPair = computed(() => splitSymbol(props.pairSymbol))
const leverageLevels = computed(() => {
  const configured = props.product?.leverageLevels || []
  return [...new Set(configured.filter((level) => Number.isFinite(level) && level > 0))].sort((left, right) => left - right)
})
const leverageIndex = computed({
  get: () => Math.max(0, leverageLevels.value.indexOf(draftLeverage.value)),
  set: (index: number) => {
    const next = leverageLevels.value[Math.max(0, Math.min(Math.round(index), leverageLevels.value.length - 1))]
    if (next) draftLeverage.value = next
  },
})
const leverageProgress = computed(() => {
  const maximumIndex = leverageLevels.value.length - 1
  return maximumIndex > 0 ? (leverageIndex.value / maximumIndex) * 100 : 0
})
const quickLeverageLevels = computed(() => {
  const levels = leverageLevels.value
  if (levels.length <= 6) return levels
  return [...new Set(Array.from({ length: 6 }, (_, index) => (
    levels[Math.round((index / 5) * (levels.length - 1))]
  )))]
})
const supportedMarginModes = computed<MarginMode[]>(() => {
  const modes = props.product?.marginModes || []
  return modes
    .filter((mode): mode is MarginMode => mode === 'cross' || mode === 'isolated')
    .sort((left, right) => (left === right ? 0 : left === 'cross' ? -1 : 1))
})
const supportedOrderTypes = computed<MarginOrderType[]>(() => props.product?.orderTypes || [])
const pairRows = computed(() => props.products.map((product) => {
  const pair = splitSymbol(product.symbol)
  return {
    product,
    pair,
    ticker: marketStore.tickerFor(product.symbol),
    normalized: normalizeSymbol(product.symbol),
  }
}))
const sliderMarkLevels = computed(() => quickLeverageLevels.value)
const mainstreamSymbols = computed(() => new Set(
  [...pairRows.value]
    .sort((left, right) => (right.ticker?.volume || 0) - (left.ticker?.volume || 0))
    .slice(0, Math.min(8, pairRows.value.length))
    .map((row) => row.normalized),
))
const filteredPairRows = computed(() => {
  const query = searchQuery.value.trim().toUpperCase().replace(/[\s/_-]/g, '')
  return pairRows.value.filter((row) => {
    if (pairFilter.value === 'favorites' && !marketFavorites.isFavorite(row.product.symbol)) return false
    if (pairFilter.value === 'mainstream' && !mainstreamSymbols.value.has(row.normalized)) return false
    if (!query) return true
    return row.normalized.includes(query)
      || row.pair.base.includes(query)
      || row.pair.quote.includes(query)
  })
})

watch(() => props.open, (open) => {
  if (open === 'pair') {
    searchQuery.value = ''
    pairFilter.value = 'all'
  }
  if (open === 'leverage') {
    draftLeverage.value = leverageLevels.value.includes(props.leverage)
      ? props.leverage
      : leverageLevels.value[0] || props.leverage
  }
  if (open === 'marginMode') {
    draftMarginMode.value = supportedMarginModes.value.includes(props.marginMode)
      ? props.marginMode
      : supportedMarginModes.value[0] || props.marginMode
  }
})

watch(() => props.product?.id, () => {
  draftLeverage.value = leverageLevels.value.includes(props.leverage)
    ? props.leverage
    : leverageLevels.value[0] || props.leverage
  draftMarginMode.value = supportedMarginModes.value.includes(props.marginMode)
    ? props.marginMode
    : supportedMarginModes.value[0] || props.marginMode
})

function requestClose(): void {
  if (!props.saving) emit('close')
}

function handleKeydown(event: KeyboardEvent): void {
  trapFocus(event, requestClose)
}

function selectPair(symbol: string): void {
  if (props.saving) return
  emit('selectPair', symbol)
}

function applyLeverage(): void {
  if (!props.saving && leverageLevels.value.includes(draftLeverage.value)) {
    emit('applyLeverage', draftLeverage.value)
  }
}

function applyMarginMode(): void {
  if (!props.saving && supportedMarginModes.value.includes(draftMarginMode.value)) {
    emit('applyMarginMode', draftMarginMode.value)
  }
}

function selectOrderType(orderType: MarginOrderType): void {
  if (!props.saving && supportedOrderTypes.value.includes(orderType)) {
    emit('selectOrderType', orderType)
  }
}

function marginModeLabel(mode: MarginMode): string {
  return t(mode === 'cross' ? 'trade.cross' : 'trade.isolated')
}

function marginModeDescription(mode: MarginMode): string {
  return t(mode === 'cross' ? 'trade.crossDescription' : 'trade.isolatedDescription')
}

function orderTypeLabel(orderType: MarginOrderType): string {
  return t(orderType === 'market' ? 'trade.marketOrderShort' : 'trade.limitOrderShort')
}

function orderTypeDescription(orderType: MarginOrderType): string {
  return t(orderType === 'market' ? 'trade.marginMarketOrderDescription' : 'trade.marginLimitOrderDescription')
}
</script>

<template>
  <Teleport to="body">
    <div
      v-if="open"
      class="contract-sheet-layer"
      :data-contract-sheet="open"
      data-pencil-source="f0L8yf R8t0p aNuw6 PKAcD Crw8v YuKtQ"
    >
      <button
        class="contract-sheet-overlay"
        type="button"
        tabindex="-1"
        :aria-label="t('common.close')"
        :disabled="saving"
        @click="requestClose"
      />

      <section
        :id="`contract-${open}-dialog`"
        ref="dialog"
        class="contract-sheet"
        :class="`contract-sheet--${open}`"
        role="dialog"
        aria-modal="true"
        :aria-busy="saving"
        :aria-labelledby="`contract-${open}-title`"
        tabindex="-1"
        @keydown="handleKeydown"
      >
        <span class="contract-sheet__grab" aria-hidden="true" />

        <template v-if="open === 'orderType'">
          <header class="contract-sheet__header">
            <div>
              <h2 id="contract-orderType-title">{{ t('trade.marginOrderTypeSheetTitle') }}</h2>
              <p>{{ t('trade.marginOrderTypeSheetHint') }}</p>
            </div>
            <button :data-dialog-initial="orderType === null ? '' : undefined" class="contract-sheet__close" type="button" :disabled="saving" :aria-label="t('common.close')" @click="requestClose">
              <X :size="18" aria-hidden="true" />
            </button>
          </header>

          <div class="contract-sheet__scroll contract-order-type-body">
            <div class="contract-mode-options" role="radiogroup" :aria-label="t('trade.marginOrderTypeSheetTitle')">
              <button
                v-for="item in supportedOrderTypes"
                :key="item"
                :data-dialog-initial="orderType === item ? '' : undefined"
                type="button"
                role="radio"
                :class="{ active: orderType === item }"
                :aria-checked="orderType === item"
                @click="selectOrderType(item)"
              >
                <span class="contract-mode-radio" aria-hidden="true"><Check v-if="orderType === item" :size="13" /></span>
                <span class="contract-mode-option__copy">
                  <span class="contract-mode-option__title">
                    <strong>{{ orderTypeLabel(item) }}</strong>
                    <small v-if="orderType === item">{{ t('trade.currentOrderType') }}</small>
                  </span>
                  <small>{{ orderTypeDescription(item) }}</small>
                </span>
              </button>
            </div>

            <aside class="contract-sheet-notice contract-sheet-notice--plain">
              <Info :size="15" aria-hidden="true" />
              <span>{{ t('trade.marginOrderTypeNotice') }}</span>
            </aside>
          </div>
        </template>

        <template v-else-if="open === 'leverage'">
          <header class="contract-sheet__header">
            <div>
              <h2 id="contract-leverage-title">{{ t('trade.leverageSheetTitle') }}</h2>
            </div>
            <button data-dialog-initial class="contract-sheet__close" type="button" :disabled="saving" :aria-label="t('common.close')" @click="requestClose">
              <X :size="18" aria-hidden="true" />
            </button>
          </header>

          <div class="contract-sheet__scroll contract-leverage-body">
            <section class="contract-leverage-card" :aria-label="t('trade.currentLeverage')">
              <div class="contract-leverage-current-row">
                <span>{{ t('trade.currentLeverage') }}</span>
                <strong class="numeric">{{ draftLeverage }}x</strong>
              </div>
              <input
                v-if="leverageLevels.length"
                v-model.number="leverageIndex"
                class="contract-leverage-slider"
                type="range"
                min="0"
                :max="Math.max(0, leverageLevels.length - 1)"
                step="1"
                :style="{ '--leverage-progress': `${leverageProgress}%` }"
                :aria-label="t('trade.leverageSheetTitle')"
              />
              <div class="contract-leverage-range" aria-hidden="true">
                <span v-for="level in sliderMarkLevels" :key="`mark-${level}`">{{ level }}x</span>
              </div>
            </section>

            <div class="contract-leverage-quick" role="group" :aria-label="t('trade.leverageQuickOptions')">
              <button
                v-for="level in quickLeverageLevels"
                :key="level"
                type="button"
                :class="{ active: draftLeverage === level }"
                :aria-pressed="draftLeverage === level"
                @click="draftLeverage = level"
              >
                <span class="numeric">{{ level }}x</span>
              </button>
            </div>

            <div class="contract-scope-row">
              <span class="contract-scope-row__copy">
                <strong>{{ t('trade.applyBothDirections') }}</strong>
                <small>{{ t('trade.applyBothDirectionsHint', { pair: compactPair }) }}</small>
              </span>
              <span class="contract-scope-toggle" aria-hidden="true"><i /></span>
            </div>

            <aside class="contract-sheet-notice">
              <TriangleAlert :size="15" aria-hidden="true" />
              <span>{{ t('trade.leverageRiskDescription') }}</span>
            </aside>
          </div>

          <p v-if="error" class="contract-sheet__error" role="alert">{{ error }}</p>
          <button class="contract-sheet__submit" type="button" :disabled="saving || !leverageLevels.length" @click="applyLeverage">
            {{ saving ? t('common.saving') : t('trade.confirmLeverage', { leverage: draftLeverage }) }}
          </button>
        </template>

        <template v-else-if="open === 'marginMode'">
          <header class="contract-sheet__header">
            <div>
              <h2 id="contract-marginMode-title">{{ t('trade.marginModeSheetTitle') }}</h2>
              <p>{{ compactPair }} · {{ t('trade.perpetualShort') }}</p>
            </div>
            <button data-dialog-initial class="contract-sheet__close" type="button" :disabled="saving" :aria-label="t('common.close')" @click="requestClose">
              <X :size="18" aria-hidden="true" />
            </button>
          </header>

          <div class="contract-sheet__scroll contract-mode-body">
            <div class="contract-mode-options" role="radiogroup" :aria-label="t('trade.marginModeSheetTitle')">
              <button
                v-for="item in supportedMarginModes"
                :key="item"
                type="button"
                role="radio"
                :class="{ active: draftMarginMode === item }"
                :aria-checked="draftMarginMode === item"
                @click="draftMarginMode = item"
              >
                <span class="contract-mode-radio" aria-hidden="true"><Check v-if="draftMarginMode === item" :size="13" /></span>
                <span class="contract-mode-option__copy">
                  <span class="contract-mode-option__title">
                    <strong>{{ marginModeLabel(item) }}</strong>
                    <small v-if="draftMarginMode === item">{{ t('trade.currentMode') }}</small>
                  </span>
                  <small>{{ marginModeDescription(item) }}</small>
                </span>
              </button>
            </div>

            <aside class="contract-sheet-notice contract-sheet-notice--plain">
              <Info :size="15" aria-hidden="true" />
              <span>{{ t('trade.marginModeNoticeDescription') }}</span>
            </aside>
          </div>

          <p v-if="error" class="contract-sheet__error" role="alert">{{ error }}</p>
          <button class="contract-sheet__submit" type="button" :disabled="saving || !supportedMarginModes.length" @click="applyMarginMode">
            {{ saving ? t('common.saving') : t('trade.confirmMarginMode', { mode: marginModeLabel(draftMarginMode) }) }}
          </button>
        </template>

        <template v-else>
          <header class="contract-sheet__header contract-pair-header">
            <div>
              <h2 id="contract-pair-title">{{ t('trade.contractPairSheetTitle') }}</h2>
              <p>{{ t('trade.contractPairSheetHint', { asset: selectedPair.quote }) }}</p>
            </div>
            <button data-dialog-initial class="contract-sheet__close" type="button" :aria-label="t('common.close')" @click="requestClose">
              <X :size="18" aria-hidden="true" />
            </button>
          </header>

          <label class="contract-pair-search">
            <Search :size="16" aria-hidden="true" />
            <input v-model="searchQuery" type="search" :placeholder="t('trade.contractPairSearchPlaceholder')" />
          </label>

          <div class="contract-pair-filters" role="tablist" :aria-label="t('trade.contractPairFilters')">
            <button type="button" role="tab" :class="{ active: pairFilter === 'favorites' }" :aria-selected="pairFilter === 'favorites'" @click="pairFilter = 'favorites'">
              <Star :size="14" aria-hidden="true" /> {{ t('home.favorites') }}
            </button>
            <button type="button" role="tab" :class="{ active: pairFilter === 'all' }" :aria-selected="pairFilter === 'all'" @click="pairFilter = 'all'">
              {{ t('common.all') }}
            </button>
            <button type="button" role="tab" :class="{ active: pairFilter === 'mainstream' }" :aria-selected="pairFilter === 'mainstream'" @click="pairFilter = 'mainstream'">
              {{ t('home.mainstream') }}
            </button>
          </div>

          <div class="contract-sheet__scroll contract-pair-list">
            <p v-if="productsLoading" class="contract-pair-state" role="status">{{ t('common.loading') }}</p>
            <div v-else-if="productsError" class="contract-pair-state" role="alert">
              <span>{{ t('common.loadFailed') }}</span>
              <button type="button" @click="emit('retryProducts')">{{ t('common.retry') }}</button>
            </div>
            <template v-else>
              <button
                v-for="row in filteredPairRows"
                :key="row.product.id"
                type="button"
                class="contract-pair-row"
                :class="{ active: row.normalized === normalizeSymbol(pairSymbol) }"
                :aria-current="row.normalized === normalizeSymbol(pairSymbol) ? 'true' : undefined"
                @click="selectPair(row.product.symbol)"
              >
                <AssetMark :symbol="row.pair.base" :src="row.product.logoUrl || row.ticker?.iconUrl" :fallback-src="row.ticker?.baseIconUrl" :size="34" />
                <span class="contract-pair-row__name">
                  <strong>{{ row.pair.base }}<small>/{{ row.pair.quote }}</small></strong>
                  <small>{{ row.pair.base }} · {{ t('trade.perpetualShort') }}</small>
                </span>
                <span class="contract-pair-row__market numeric">
                  <strong>{{ row.ticker ? formatPrice(row.ticker.lastPrice) : '--' }}</strong>
                  <small v-if="row.ticker" :class="row.ticker.changePercent >= 0 ? 'positive' : 'negative'">
                    {{ row.ticker.changePercent >= 0 ? '+' : '' }}{{ row.ticker.changePercent.toFixed(2) }}%
                  </small>
                  <small v-else>--</small>
                </span>
                <Check v-if="row.normalized === normalizeSymbol(pairSymbol)" :size="16" aria-hidden="true" />
              </button>
              <p v-if="!filteredPairRows.length" class="contract-pair-empty" role="status">{{ t('markets.noResults') }}</p>
            </template>
          </div>

          <p class="contract-pair-source">{{ t('trade.contractPairSourceHint') }}</p>
        </template>
      </section>
    </div>
  </Teleport>
</template>

<style scoped>
.contract-sheet-layer,
.contract-sheet-layer * {
  box-sizing: border-box;
}

.contract-sheet-layer {
  align-items: end;
  bottom: 0;
  display: grid;
  justify-items: center;
  left: auto;
  position: fixed;
  right: 5.5vw;
  top: 0;
  width: min(100%, 448px);
  z-index: var(--layer-overlay, 80);
}

.contract-sheet-overlay {
  background: rgb(7 17 13 / 64%);
  border: 0;
  inset: 0;
  padding: 0;
  position: absolute;
  width: 100%;
}

.contract-sheet {
  --sheet-page: #ffffff;
  --sheet-canvas: #f7f9f8;
  --sheet-raised: #eef2f0;
  --sheet-line: #ccd5d0;
  --sheet-line-strong: #aebbb4;
  --sheet-text: #111714;
  --sheet-muted: #68736d;
  --sheet-accent: #43efa9;
  --sheet-accent-strong: #087b52;
  --sheet-accent-soft: #d9f9eb;
  --sheet-negative: #ff654a;
  background: var(--sheet-page);
  border: 0;
  border-radius: 22px 22px 0 0;
  border-top: 1px solid var(--sheet-line);
  box-shadow: 0 -10px 28px rgb(7 17 13 / 20%);
  color: var(--sheet-text);
  display: grid;
  align-content: start;
  grid-template-rows: 14px 36px auto auto;
  max-height: calc(100dvh - max(12px, env(safe-area-inset-top)));
  max-width: 448px;
  min-height: 0;
  overflow: hidden;
  overscroll-behavior: contain;
  padding: 11px 16px calc(22px + env(safe-area-inset-bottom));
  position: relative;
  row-gap: 14px;
  width: 100%;
}

html[data-theme='dark'] .contract-sheet {
  --sheet-page: #0c100e;
  --sheet-canvas: #070a09;
  --sheet-raised: #121714;
  --sheet-line: #29342e;
  --sheet-line-strong: #3a4a42;
  --sheet-text: #f2f7f4;
  --sheet-muted: #95a19a;
  --sheet-accent: #43efa9;
  --sheet-accent-strong: #61f1b6;
  --sheet-accent-soft: #103326;
  --sheet-negative: #ff654a;
  box-shadow: 0 -10px 28px rgb(0 0 0 / 64%);
}

.contract-sheet--leverage {
  height: min(500px, calc(100dvh - max(12px, env(safe-area-inset-top))));
}

.contract-sheet--marginMode {
  grid-template-rows: 14px 45px auto auto;
  height: min(446px, calc(100dvh - max(12px, env(safe-area-inset-top))));
}

.contract-sheet--orderType {
  grid-template-rows: 14px 45px auto;
  height: min(338px, calc(100dvh - max(12px, env(safe-area-inset-top))));
}

.contract-sheet--marginMode .contract-sheet__header {
  height: 45px;
}

.contract-sheet--pair {
  grid-template-rows: 14px 45px 40px 22px 322px auto;
  height: min(620px, calc(100dvh - max(12px, env(safe-area-inset-top))));
  padding-bottom: calc(20px + env(safe-area-inset-bottom));
  row-gap: 10px;
}

.contract-sheet__grab {
  align-items: center;
  background: transparent;
  display: flex;
  height: 14px;
  justify-content: center;
  width: 100%;
}

.contract-sheet__grab::before {
  background: var(--sheet-muted);
  border-radius: 2px;
  content: '';
  height: 4px;
  width: 38px;
}

.contract-sheet__header {
  align-items: center;
  display: flex;
  height: 36px;
  justify-content: space-between;
  min-width: 0;
}

.contract-sheet__header > div {
  min-width: 0;
}

.contract-sheet__header h2 {
  font-size: 19px;
  font-weight: 750;
  letter-spacing: -.02em;
  line-height: 1.15;
  margin: 0;
}

.contract-sheet__header p {
  color: var(--sheet-muted);
  font-family: var(--font-geist-mono), var(--data-font), monospace;
  font-size: 10px;
  font-weight: 550;
  line-height: 1.2;
  margin: 2px 0 0;
}

.contract-sheet__close {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: 50%;
  color: var(--sheet-text);
  display: inline-flex;
  flex: 0 0 auto;
  height: 44px;
  justify-content: center;
  margin: -4px;
  padding: 0;
  position: relative;
  width: 44px;
  z-index: 0;
}

.contract-sheet__close::before {
  background: var(--sheet-canvas);
  border-radius: 50%;
  content: '';
  inset: 4px;
  position: absolute;
  z-index: -1;
}

.contract-sheet__scroll {
  min-height: 0;
  overflow-x: hidden;
  overflow-y: auto;
  overscroll-behavior: contain;
  scrollbar-width: none;
}

.contract-sheet__scroll::-webkit-scrollbar {
  display: none;
}

.contract-leverage-body,
.contract-mode-body {
  align-content: start;
  display: grid;
  gap: 14px;
}

.contract-order-type-body {
  align-content: start;
  display: grid;
  gap: 14px;
  min-height: 0;
}

.contract-leverage-body {
  height: 279px;
}

.contract-mode-body {
  height: 185px;
}

.contract-leverage-card {
  background: var(--sheet-canvas);
  border: 1px solid var(--sheet-line);
  border-radius: 12px;
  display: grid;
  gap: 12px;
  height: 126px;
  padding: 14px 16px;
}

.contract-leverage-current-row {
  align-items: center;
  display: flex;
  justify-content: space-between;
  min-width: 0;
}

.contract-leverage-current-row > span {
  color: var(--sheet-muted);
  font-size: 12px;
  font-weight: 550;
}

.contract-leverage-current-row > strong {
  color: var(--sheet-accent-strong);
  font-size: 26px;
  font-weight: 750;
  line-height: 1;
}

.contract-leverage-slider {
  --leverage-progress: 0%;
  appearance: none;
  background: linear-gradient(
    90deg,
    var(--sheet-accent) var(--leverage-progress),
    var(--sheet-line) var(--leverage-progress)
  ) center / calc(100% - 4px) 4px no-repeat;
  border-radius: 999px;
  height: 44px;
  margin: -8px 0;
  outline: 0;
  width: 100%;
}

.contract-leverage-slider::-webkit-slider-thumb {
  appearance: none;
  background: var(--sheet-page);
  border: 4px solid var(--sheet-accent);
  border-radius: 50%;
  box-shadow: 0 2px 6px rgb(7 17 13 / 18%);
  height: 18px;
  width: 18px;
}

.contract-leverage-slider::-moz-range-thumb {
  background: var(--sheet-page);
  border: 4px solid var(--sheet-accent);
  border-radius: 50%;
  box-shadow: 0 2px 6px rgb(7 17 13 / 18%);
  height: 10px;
  width: 10px;
}

.contract-leverage-range {
  align-items: center;
  color: var(--sheet-muted);
  display: flex;
  font-family: var(--font-geist-mono), var(--data-font), monospace;
  font-size: 9px;
  font-weight: 550;
  justify-content: space-between;
}

.contract-leverage-quick {
  display: grid;
  gap: 6px;
  grid-template-columns: repeat(6, minmax(0, 1fr));
  height: 34px;
}

.contract-leverage-quick button {
  align-items: center;
  background: transparent;
  border: 0;
  color: var(--sheet-text);
  display: flex;
  font-size: 11px;
  font-weight: 550;
  height: 44px;
  justify-content: center;
  margin-block: -5px;
  min-width: 0;
  padding: 0 2px;
  position: relative;
  z-index: 0;
}

.contract-leverage-quick button::before {
  background: var(--sheet-canvas);
  border: 1px solid var(--sheet-line);
  border-radius: 8px;
  content: '';
  inset: 5px 0;
  position: absolute;
  z-index: -1;
}

.contract-leverage-quick button.active {
  color: var(--sheet-accent-strong);
  font-weight: 700;
}

.contract-leverage-quick button.active::before {
  background: var(--sheet-accent-soft);
  border-color: var(--sheet-accent);
}

.contract-scope-row {
  align-items: center;
  border: 1px solid var(--sheet-line);
  border-radius: 10px;
  display: flex;
  height: 44px;
  justify-content: space-between;
  padding: 0 12px;
}

.contract-scope-row__copy {
  display: grid;
  gap: 2px;
  min-width: 0;
}

.contract-scope-row strong {
  font-size: 12px;
  font-weight: 650;
}

.contract-scope-row small {
  color: var(--sheet-muted);
  font-size: 9px;
  font-weight: 450;
  line-height: 1.2;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.contract-scope-toggle {
  background: var(--sheet-accent);
  border-radius: 11px;
  display: block;
  flex: 0 0 auto;
  height: 22px;
  position: relative;
  width: 38px;
}

.contract-scope-toggle i {
  background: #ffffff;
  border-radius: 50%;
  height: 16px;
  position: absolute;
  right: 4px;
  top: 3px;
  width: 16px;
}

.contract-sheet-notice {
  align-items: center;
  background: rgb(255 180 84 / 9%);
  border: 1px solid rgb(255 180 84 / 30%);
  border-radius: 8px;
  color: var(--sheet-text);
  display: flex;
  font-size: 10px;
  font-weight: 500;
  gap: 8px;
  line-height: 1.35;
  height: 33px;
  min-height: 33px;
  padding: 8px 9px;
}

.contract-sheet-notice > svg {
  color: #e79a2b;
  flex: 0 0 auto;
}

.contract-sheet-notice > span {
  min-width: 0;
}

.contract-sheet-notice--plain {
  background: rgb(255 180 84 / 9%);
  border-color: rgb(255 180 84 / 30%);
}

.contract-sheet__error {
  color: var(--sheet-negative);
  font-size: 10px;
  line-height: 1.35;
  margin: 0;
  overflow-wrap: anywhere;
}

.contract-sheet__submit {
  background: var(--sheet-accent);
  border: 0;
  border-radius: 24px;
  color: #07110d;
  font-size: 14px;
  font-weight: 750;
  height: 48px;
  margin: 0;
  padding: 0 16px;
  width: 100%;
}

.contract-mode-options {
  display: grid;
  gap: 10px;
}

.contract-mode-options button {
  align-items: center;
  background: var(--sheet-canvas);
  border: 1px solid var(--sheet-line);
  border-radius: 12px;
  color: var(--sheet-text);
  display: flex;
  gap: 12px;
  height: 64px;
  min-height: 64px;
  padding: 12px 14px;
  text-align: left;
  width: 100%;
}

.contract-mode-options button.active {
  background: var(--sheet-accent-soft);
  border-color: var(--sheet-accent);
}

.contract-mode-radio {
  align-items: center;
  border: 1.5px solid var(--sheet-muted);
  border-radius: 50%;
  color: #07110d;
  display: inline-flex;
  flex: 0 0 auto;
  height: 22px;
  justify-content: center;
  order: 0;
  width: 22px;
}

.contract-mode-options button.active .contract-mode-radio {
  background: var(--sheet-accent);
  border-color: var(--sheet-accent);
}

.contract-mode-option__copy {
  display: grid;
  flex: 1;
  gap: 4px;
  min-width: 0;
}

.contract-mode-option__title {
  align-items: center;
  display: flex;
  justify-content: space-between;
  min-width: 0;
}

.contract-mode-option__title strong {
  font-size: 14px;
  font-weight: 750;
}

.contract-mode-option__title > small {
  background: var(--sheet-accent);
  border-radius: 9px;
  color: #07110d;
  font-size: 9px;
  font-weight: 700;
  line-height: 18px;
  padding: 0 7px;
}

.contract-mode-option__copy > small {
  color: var(--sheet-muted);
  font-size: 10px;
  font-weight: 450;
  line-height: 1.4;
}

.contract-pair-header {
  height: 45px;
}

.contract-pair-search {
  align-items: center;
  background: var(--sheet-canvas);
  border: 1px solid var(--sheet-line);
  border-radius: 10px;
  color: var(--sheet-muted);
  display: flex;
  gap: 8px;
  height: 40px;
  padding: 0 12px;
}

.contract-pair-search:focus-within {
  border-color: var(--sheet-accent);
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--sheet-accent) 12%, transparent);
}

.contract-pair-search input {
  background: transparent;
  border: 0;
  color: var(--sheet-text);
  font-size: 11px;
  font-weight: 450;
  height: 44px;
  margin-block: -2px;
  min-width: 0;
  outline: 0;
  padding: 0;
  width: 100%;
}

.contract-pair-search input:focus-visible {
  outline: 0;
}

.contract-pair-filters {
  align-items: center;
  display: flex;
  gap: 18px;
  height: 22px;
}

.contract-pair-filters button {
  align-items: center;
  background: transparent;
  border: 0;
  color: var(--sheet-muted);
  display: inline-flex;
  flex: 0 0 auto;
  font-size: 11px;
  font-weight: 550;
  gap: 5px;
  height: 22px;
  justify-content: center;
  padding: 0;
  position: relative;
}

.contract-pair-filters button.active {
  color: var(--sheet-text);
  font-weight: 700;
}

.contract-pair-filters button.active::after {
  background: var(--sheet-accent);
  border-radius: 1px;
  bottom: 0;
  content: '';
  height: 2px;
  left: 50%;
  position: absolute;
  transform: translateX(-50%);
  width: 18px;
}

.contract-pair-list {
  display: grid;
  gap: 2px;
  height: 322px;
}

.contract-pair-row {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: 10px;
  color: var(--sheet-text);
  display: grid;
  gap: 10px;
  grid-template-columns: 34px minmax(0, 1fr) minmax(76px, auto) 16px;
  height: 52px;
  min-height: 52px;
  padding: 0 10px;
  text-align: left;
  width: 100%;
}

.contract-pair-row.active {
  background: var(--sheet-accent-soft);
  color: var(--sheet-text);
}

.contract-pair-row :deep(.asset-mark) {
  border: 0;
  box-shadow: 0 2px 5px rgb(7 17 13 / 14%);
}

.contract-pair-row__name,
.contract-pair-row__market {
  display: grid;
  gap: 2px;
  min-width: 0;
}

.contract-pair-row__name strong,
.contract-pair-row__market strong {
  font-size: 12px;
  font-weight: 650;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.contract-pair-row__name strong small {
  color: var(--sheet-muted);
  font-size: 10px;
  font-weight: 550;
}

.contract-pair-row__name > small,
.contract-pair-row__market > small {
  color: var(--sheet-muted);
  font-size: 9px;
  font-weight: 450;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.contract-pair-row__market {
  justify-items: end;
  text-align: right;
}

.contract-pair-row__market .positive {
  color: var(--sheet-accent-strong);
}

.contract-pair-row__market .negative {
  color: var(--sheet-negative);
}

.contract-pair-row > svg {
  color: var(--sheet-accent-strong);
}

.contract-pair-empty {
  color: var(--sheet-muted);
  font-size: 11px;
  margin: 0;
  padding: 42px 12px;
  text-align: center;
}

.contract-pair-state {
  align-items: center;
  color: var(--sheet-muted);
  display: flex;
  flex-direction: column;
  font-size: 11px;
  gap: 10px;
  justify-content: center;
  margin: 0;
  min-height: 144px;
  padding: 24px 12px;
  text-align: center;
}

.contract-pair-state button {
  background: var(--sheet-accent-soft);
  border: 1px solid var(--sheet-accent);
  border-radius: 22px;
  color: var(--sheet-text);
  min-height: 44px;
  padding: 0 20px;
}

.contract-pair-source {
  color: var(--sheet-muted);
  font-size: 9px;
  font-weight: 450;
  line-height: 1.35;
  margin: 0;
  text-align: left;
}

.contract-sheet button:focus-visible,
.contract-sheet input:focus-visible {
  outline: 2px solid var(--sheet-accent);
  outline-offset: 2px;
}

.contract-sheet .contract-pair-search input:focus-visible {
  outline: 0;
  outline-offset: 0;
}

.contract-sheet__close:focus-visible,
.contract-leverage-quick button:focus-visible {
  outline: 0;
}

.contract-sheet__close:focus-visible::before,
.contract-leverage-quick button:focus-visible::before {
  box-shadow: 0 0 0 2px var(--sheet-accent);
}

@media (max-width: 820px) {
  .contract-sheet-layer {
    right: 0;
    width: 100%;
  }
}

@media (max-width: 340px) {
  .contract-sheet {
    padding-inline: 12px;
  }

  .contract-leverage-body,
  .contract-mode-body {
    height: auto;
  }

  .contract-sheet-notice {
    height: auto;
  }

  .contract-leverage-quick {
    gap: 4px;
  }

  .contract-leverage-quick button {
    font-size: 9px;
  }

  .contract-scope-row {
    padding-inline: 9px;
  }

  .contract-pair-row {
    gap: 7px;
    grid-template-columns: 34px minmax(0, 1fr) minmax(68px, auto) 16px;
    padding-inline: 6px;
  }
}

@media (prefers-reduced-motion: no-preference) {
  .contract-sheet {
    animation: contract-sheet-enter 240ms cubic-bezier(.2, .8, .2, 1) both;
  }
}

@keyframes contract-sheet-enter {
  from {
    opacity: .8;
    transform: translateY(24px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
</style>
