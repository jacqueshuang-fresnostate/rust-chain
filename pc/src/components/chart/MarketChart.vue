<template>
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
  />
</template>

<script setup lang="ts">
import { computed, defineAsyncComponent, onMounted } from 'vue'
import { useSettingStore } from '@/stores/setting'
import { normalizeChartProvider } from '@/utils/chartProvider'
import type { KlineFetcher, KlineModule } from './klineData'

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

onMounted(() => {
  void settingStore.loadPlatformBrand()
})
</script>
