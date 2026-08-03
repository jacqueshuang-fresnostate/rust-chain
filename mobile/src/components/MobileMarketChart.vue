<script setup lang="ts">
import { computed, nextTick, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChartNoAxesCombined, LoaderCircle, Settings2 } from 'lucide-vue-next'
import KLineChartMarketChart from '@/components/KLineChartMarketChart.vue'
import TradingViewMarketChart from '@/components/TradingViewMarketChart.vue'
import { normalizeMarketChartPoints } from '@/core/marketChart'
import {
  loadMarketChartEngine,
  persistMarketChartEngine,
  type MarketChartEngine,
} from '@/core/marketChartEngine'
import { calculateMarketMovingAverages } from '@/core/marketIndicators'
import type { KlinePoint } from '@/core/types'

const props = withDefaults(defineProps<{
  points: KlinePoint[]
  symbol: string
  loading?: boolean
  interval?: string
  showEngineSwitch?: boolean
  compactEngineSwitch?: boolean
}>(), {
  loading: false,
  interval: '',
  showEngineSwitch: false,
  compactEngineSwitch: false,
})

const { locale, t } = useI18n()
const engineSwitch = ref<HTMLElement | null>(null)
const compactEngineTrigger = ref<HTMLButtonElement | null>(null)
const compactEngineMenuOpen = ref(false)
const engine = ref<MarketChartEngine>(loadMarketChartEngine())
const normalizedPoints = computed(() => normalizeMarketChartPoints(props.points))
const movingAverages = computed(() => calculateMarketMovingAverages(normalizedPoints.value))
const chartLocale = computed(() => locale.value === 'en' ? 'en-US' : 'zh-CN')
const hasRenderableData = computed(() => normalizedPoints.value.length > 0)
const engineOptions: readonly MarketChartEngine[] = ['klinecharts', 'tradingview']

function engineLabel(value: MarketChartEngine): string {
  return value === 'klinecharts'
    ? t('marketDetail.klineChartEngine')
    : t('marketDetail.tradingViewEngine')
}

function selectEngine(value: MarketChartEngine, closeCompactMenu = true): void {
  if (engine.value !== value) {
    engine.value = value
    persistMarketChartEngine(value)
  }
  if (props.compactEngineSwitch && closeCompactMenu) {
    compactEngineMenuOpen.value = false
    void nextTick(() => compactEngineTrigger.value?.focus())
  }
}

function focusEngineOption(value = engine.value): void {
  void nextTick(() => {
    engineSwitch.value
      ?.querySelector<HTMLButtonElement>(`[data-engine-option="${value}"]`)
      ?.focus()
  })
}

function toggleCompactEngineMenu(): void {
  compactEngineMenuOpen.value = !compactEngineMenuOpen.value
  if (compactEngineMenuOpen.value) focusEngineOption()
}

function handleCompactEngineTriggerKeydown(event: KeyboardEvent): void {
  if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) return
  event.preventDefault()
  compactEngineMenuOpen.value = true
  focusEngineOption(event.key === 'End' ? 'tradingview' : 'klinecharts')
}

function handleEngineKeydown(event: KeyboardEvent, current: MarketChartEngine): void {
  if (event.key === 'Escape' && props.compactEngineSwitch) {
    event.preventDefault()
    compactEngineMenuOpen.value = false
    void nextTick(() => compactEngineTrigger.value?.focus())
    return
  }
  let next: MarketChartEngine | null = null
  if (event.key === 'ArrowLeft' || event.key === 'ArrowUp' || event.key === 'Home') {
    next = 'klinecharts'
  }
  if (event.key === 'ArrowRight' || event.key === 'ArrowDown' || event.key === 'End') {
    next = 'tradingview'
  }
  if (!next) return
  event.preventDefault()
  if (next === current && event.key.startsWith('Arrow')) {
    next = current === 'klinecharts' ? 'tradingview' : 'klinecharts'
  }
  selectEngine(next, false)
  focusEngineOption(next)
}
</script>

<template>
  <div
    class="mobile-market-chart"
    :class="{
      'has-data': hasRenderableData,
      'has-engine-switch': showEngineSwitch && !compactEngineSwitch,
      'has-compact-engine-switch': showEngineSwitch && compactEngineSwitch,
    }"
    :data-chart-state="loading ? 'loading' : hasRenderableData ? 'ready' : 'empty'"
    :data-chart-engine="engine"
    data-fit-policy="initial-or-interval"
    :aria-busy="loading"
  >
    <div
      v-if="showEngineSwitch"
      class="mobile-market-chart__engine-switch"
      :class="{ 'is-compact': compactEngineSwitch }"
    >
      <button
        v-if="compactEngineSwitch"
        ref="compactEngineTrigger"
        type="button"
        class="mobile-market-chart__engine-trigger"
        :aria-label="`${t('marketDetail.chartEngine')}: ${engineLabel(engine)}`"
        :aria-expanded="compactEngineMenuOpen"
        aria-haspopup="true"
        @click="toggleCompactEngineMenu"
        @keydown="handleCompactEngineTriggerKeydown"
      >
        <Settings2 :size="18" aria-hidden="true" />
      </button>
      <div
        v-if="!compactEngineSwitch || compactEngineMenuOpen"
        ref="engineSwitch"
        class="mobile-market-chart__engine-options"
        role="radiogroup"
        aria-orientation="horizontal"
        :aria-label="t('marketDetail.chartEngine')"
      >
        <button
          v-for="option in engineOptions"
          :key="option"
          type="button"
          role="radio"
          :data-engine-option="option"
          :aria-checked="engine === option"
          :tabindex="engine === option ? 0 : -1"
          :class="{ 'is-active': engine === option }"
          @click="selectEngine(option)"
          @keydown="handleEngineKeydown($event, option)"
        >
          {{ engineLabel(option) }}
        </button>
      </div>
    </div>

    <div class="mobile-market-chart__viewport">
      <KLineChartMarketChart
        v-if="engine === 'klinecharts'"
        :points="normalizedPoints"
        :interval="interval"
        :locale="chartLocale"
        :label="t('marketDetail.market')"
        :symbol="symbol"
      />
      <TradingViewMarketChart
        v-else
        :points="normalizedPoints"
        :moving-averages="movingAverages"
        :interval="interval"
        :locale="chartLocale"
        :label="t('marketDetail.market')"
      />

      <div v-if="loading && !hasRenderableData" class="mobile-market-chart__state" role="status">
        <LoaderCircle :size="20" class="spin" />
        <span>{{ t('marketDetail.loadingChart') }}</span>
      </div>
      <div v-else-if="!hasRenderableData" class="mobile-market-chart__state">
        <ChartNoAxesCombined :size="22" />
        <span>{{ t('common.marketUnavailable') }}</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.mobile-market-chart {
  background: var(--surface);
  display: grid;
  grid-template-rows: minmax(0, 1fr);
  height: 100%;
  min-height: 0;
  min-width: 0;
  overflow: visible;
  position: relative;
  width: 100%;
}

.mobile-market-chart.has-engine-switch {
  grid-template-rows: 44px minmax(0, 1fr);
}

.mobile-market-chart__engine-switch {
  background: color-mix(in srgb, var(--surface) 94%, var(--market-inset));
  border-bottom: 1px solid var(--line);
  display: flex;
  justify-content: flex-end;
  min-height: 44px;
  min-width: 0;
}

.mobile-market-chart__engine-options {
  align-items: stretch;
  display: flex;
  justify-content: flex-end;
  min-height: 44px;
  min-width: 0;
}

.mobile-market-chart__engine-switch button {
  background: transparent;
  border-bottom: 2px solid transparent;
  color: var(--muted);
  font-family: var(--data-font);
  font-size: 9px;
  font-weight: 720;
  min-height: 44px;
  min-width: 86px;
  padding: 0 8px;
}

.mobile-market-chart__engine-options button.is-active {
  background: color-mix(in srgb, var(--positive) 9%, transparent);
  border-color: var(--positive);
  color: var(--positive);
}

.mobile-market-chart__engine-switch button:focus-visible {
  box-shadow: inset 0 0 0 2px var(--focus);
  outline: none;
  outline-offset: -2px;
}

.mobile-market-chart__viewport {
  height: 100%;
  min-height: 0;
  min-width: 0;
  overflow: hidden;
  position: relative;
}

.mobile-market-chart__engine-switch.is-compact {
  background: transparent;
  border: 0;
  height: 44px;
  min-height: 44px;
  overflow: visible;
  position: absolute;
  right: 10px;
  top: -46px;
  width: 44px;
  z-index: 5;
}

.mobile-market-chart__engine-trigger {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: 50%;
  color: var(--muted);
  display: grid;
  justify-items: center;
  min-width: 44px;
  padding: 0;
  width: 44px;
}

.mobile-market-chart__engine-switch.is-compact .mobile-market-chart__engine-trigger {
  border: 0;
  height: 44px;
  min-height: 44px;
  min-width: 44px;
  width: 44px;
}

.mobile-market-chart__engine-switch.is-compact .mobile-market-chart__engine-options {
  background: var(--surface);
  border: 1px solid var(--line);
  border-radius: 8px;
  box-shadow: 0 8px 24px var(--shadow);
  display: grid;
  min-width: 184px;
  overflow: hidden;
  position: absolute;
  right: 0;
  top: 42px;
}

.mobile-market-chart__engine-switch.is-compact .mobile-market-chart__engine-options button {
  border-bottom: 1px solid var(--line);
  min-width: 184px;
  text-align: left;
}

.mobile-market-chart__engine-switch.is-compact .mobile-market-chart__engine-options button:last-child {
  border-bottom: 0;
}

.mobile-market-chart__state {
  align-items: center;
  background: color-mix(in srgb, var(--surface) 88%, transparent);
  color: var(--muted);
  display: flex;
  flex-direction: column;
  font-size: 12px;
  gap: 8px;
  inset: 0;
  justify-content: center;
  pointer-events: none;
  position: absolute;
  z-index: 2;
}

.spin {
  animation: spin .8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

@media (max-width: 340px) {
  .mobile-market-chart__engine-switch button {
    min-width: 82px;
    padding-inline: 6px;
  }

  .mobile-market-chart__engine-switch.is-compact .mobile-market-chart__engine-trigger {
    min-width: 44px;
    padding: 0;
    width: 44px;
  }
}

@media (prefers-reduced-motion: reduce) {
  .mobile-market-chart__engine-switch button {
    transition: none;
  }

  .spin {
    animation: none;
  }
}
</style>
