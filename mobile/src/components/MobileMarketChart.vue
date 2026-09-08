<script setup lang="ts">
import { computed, onUnmounted, shallowRef, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChartNoAxesCombined, LoaderCircle } from 'lucide-vue-next'
import LightweightMarketChart from '@/components/LightweightMarketChart.vue'
import { fetchOlderKlines } from '@/api/market'
import { createMarketChartHistorySession, type MarketChartHistorySnapshot } from '@/core/marketChartHistory'
import { calculateMarketMovingAverages } from '@/core/marketIndicators'
import type { KlinePoint, MarketTicker } from '@/core/types'

const props = withDefaults(defineProps<{
  points: KlinePoint[]
  symbol: string
  marketType?: MarketTicker['marketType']
  loading?: boolean
  interval?: string
}>(), {
  loading: false,
  interval: '',
})

const { locale, t } = useI18n()
const history = shallowRef<MarketChartHistorySnapshot>({ points: [], status: 'idle' })
const historySession = createMarketChartHistorySession({
  loadOlder: fetchOlderKlines,
  onChange: (snapshot) => { history.value = snapshot },
})
watch(
  () => ({ symbol: props.symbol, interval: props.interval, points: props.points, loading: props.loading }),
  (input) => historySession.sync(input),
  { immediate: true },
)
onUnmounted(() => historySession.dispose())
const normalizedPoints = computed(() => history.value.points)
const movingAverages = computed(() => calculateMarketMovingAverages(normalizedPoints.value))
const chartLocale = computed(() => locale.value === 'en' ? 'en-US' : 'zh-CN')
const hasGeneratedSource = computed(() => props.marketType === 'strategy' || props.marketType === 'internal')
const hasRenderableData = computed(() => normalizedPoints.value.length > 0)
</script>

<template>
  <div
    class="mobile-market-chart"
    :class="{ 'has-data': hasRenderableData, 'has-generated-source': hasGeneratedSource }"
    :data-chart-state="loading ? 'loading' : hasRenderableData ? 'ready' : 'empty'"
    data-chart-engine="lightweight-charts"
    data-fit-policy="initial-or-dataset"
    :data-history-state="history.status"
    :aria-busy="loading"
  >
    <p v-if="hasGeneratedSource" class="mobile-market-chart__source">{{ t('marketDetail.generatedMarketSource') }}</p>
    <div class="mobile-market-chart__viewport">
      <LightweightMarketChart
        :points="normalizedPoints"
        :moving-averages="movingAverages"
        :symbol="symbol"
        :interval="interval"
        :history-loading="loading"
        :locale="chartLocale"
        :label="t('marketDetail.market')"
        @load-history="historySession.requestOlder()"
      />

      <div
        v-if="hasRenderableData && history.status !== 'idle'"
        class="mobile-market-chart__history"
        role="status"
        aria-live="polite"
        :aria-busy="history.status === 'loading'"
      >
        <template v-if="history.status === 'loading'">
          <LoaderCircle :size="14" class="spin" aria-hidden="true" />
          <span>{{ t('marketDetail.loadingOlderChart') }}</span>
        </template>
        <template v-else-if="history.status === 'error'">
          <span>{{ t('marketDetail.olderChartLoadFailed') }}</span>
          <button type="button" :aria-label="t('marketDetail.retryOlderChart')" @click="historySession.retry()">
            {{ t('common.retry') }}
          </button>
        </template>
        <span v-else>{{ t('marketDetail.noOlderChart') }}</span>
      </div>

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
  height: 100%;
  min-height: 0;
  min-width: 0;
  overflow: hidden;
  position: relative;
  width: 100%;
}

.mobile-market-chart.has-generated-source {
  display: flex;
  flex-direction: column;
}

.mobile-market-chart__source {
  color: var(--muted-strong);
  flex: none;
  font-size: 11px;
  line-height: 1.4;
  margin: 0;
  min-height: 44px;
  overflow-wrap: anywhere;
  padding: 6px 8px 6px 68px;
}

.mobile-market-chart__viewport {
  height: 100%;
  min-height: 0;
  min-width: 0;
  overflow: hidden;
  position: relative;
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

.mobile-market-chart__history {
  align-items: center;
  background: color-mix(in srgb, var(--surface) 92%, transparent);
  color: var(--muted-strong);
  display: flex;
  font-size: 12px;
  gap: 6px;
  max-width: calc(100% - 68px);
  padding: 0 6px;
  pointer-events: none;
  position: absolute;
  right: 4px;
  top: 4px;
  z-index: 2;
}

.mobile-market-chart__history button {
  background: transparent;
  border: 0;
  color: var(--text);
  flex-shrink: 0;
  min-height: 44px;
  min-width: 44px;
  pointer-events: auto;
}

.mobile-market-chart__history button:focus-visible {
  outline: 2px solid var(--focus);
  outline-offset: -2px;
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
