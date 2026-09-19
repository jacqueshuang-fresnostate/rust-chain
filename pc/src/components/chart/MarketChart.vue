<template>
  <div class="market-chart-frame">
  <div class="market-chart-sources" :aria-label="$t('marketProvenance.chartSources')">
    <span>{{ $t('marketProvenance.chartSources') }}</span>
    <MarketProvenanceLabel v-for="item in sources" :key="item.key" :provenance="item.provenance" />
  </div>
  <component
    :is="chartComponent"
    :key="chartKey"
    :data-list="dataList"
    :fetch-k-line="fetchKLine"
    :kline-topic="klineTopic"
    :module="module"
    :period="period"
    :precision="precision"
    :symbol="symbol"
    class="market-chart-canvas"
    @provenance="receiveProvenance"
  />
  </div>
</template>

<script setup lang="ts">
import { computed, defineAsyncComponent, onMounted, ref, watch } from 'vue'
import MarketProvenanceLabel from '@/components/trade/MarketProvenanceLabel.vue'
import { marketSource, type MarketProvenance } from '@/api/marketProvenance'
import { useSettingStore } from '@/stores/setting'
import { normalizeChartProvider } from '@/utils/chartProvider'
import type { KlineBar, KlineFetcher, KlineModule } from './klineData'

const props = withDefaults(defineProps<{
  module?: KlineModule
  symbol: string
  period?: string
  precision?: number
  dataList?: unknown[]
  klineTopic?: string
  fetchKLine?: KlineFetcher
}>(), {
  module: 'market',
  period: '1m',
  precision: 8
})

const settingStore = useSettingStore()
const KlineChartsChart = defineAsyncComponent(() => import('./TVChart.vue'))
const TradingViewChart = defineAsyncComponent(() => import('./TradingViewChart.vue'))
const chartProvider = computed(() => normalizeChartProvider(settingStore.chartProvider))
const chartComponent = computed(() => chartProvider.value === 'tradingview' ? TradingViewChart : KlineChartsChart)
const chartKey = computed(() => `${chartProvider.value}-${props.symbol}-${props.period}-${props.precision}`)
const evidence = ref(new Map<number, MarketProvenance>())
let evidenceSeries = ''
watch(chartKey, () => { evidence.value = new Map(); evidenceSeries = '' })
function receiveProvenance(bars: KlineBar[], series: string) {
  const next = series === evidenceSeries ? new Map(evidence.value) : new Map<number, MarketProvenance>()
  for (const bar of bars) next.set(bar.timestamp, { source: bar.source, provider: bar.provider })
  evidenceSeries = series
  evidence.value = next
}
const sources = computed(() => {
  const distinct = new Map<string, MarketProvenance>()
  for (const row of evidence.value.values()) distinct.set(marketSource(row), row)
  if (!distinct.size) distinct.set('unknown', {})
  return [...distinct].map(([key, provenance]) => ({ key, provenance }))
})

onMounted(() => {
  void settingStore.loadPlatformBrand()
})
</script>

<style scoped>
.market-chart-frame { display: flex; flex-direction: column; min-width: 0; min-height: 0; height: 100%; }
.market-chart-sources { display: flex; flex-wrap: wrap; align-items: center; gap: 4px 10px; padding: 4px 8px; font-size: 10px; flex: none; }
.market-chart-canvas { flex: 1; min-height: 0; height: auto; }
</style>
