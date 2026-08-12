<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChartNoAxesCombined, LoaderCircle } from 'lucide-vue-next'
import LightweightMarketChart from '@/components/LightweightMarketChart.vue'
import { normalizeMarketChartPoints } from '@/core/marketChart'
import { calculateMarketMovingAverages } from '@/core/marketIndicators'
import type { KlinePoint } from '@/core/types'

const props = withDefaults(defineProps<{
  points: KlinePoint[]
  symbol: string
  loading?: boolean
  interval?: string
}>(), {
  loading: false,
  interval: '',
})

const { locale, t } = useI18n()
const normalizedPoints = computed(() => normalizeMarketChartPoints(props.points))
const movingAverages = computed(() => calculateMarketMovingAverages(normalizedPoints.value))
const chartLocale = computed(() => locale.value === 'en' ? 'en-US' : 'zh-CN')
const hasRenderableData = computed(() => normalizedPoints.value.length > 0)
</script>

<template>
  <div
    class="mobile-market-chart"
    :class="{ 'has-data': hasRenderableData }"
    :data-chart-state="loading ? 'loading' : hasRenderableData ? 'ready' : 'empty'"
    data-chart-engine="lightweight-charts"
    data-fit-policy="initial-or-dataset"
    :aria-busy="loading"
  >
    <div class="mobile-market-chart__viewport">
      <LightweightMarketChart
        :points="normalizedPoints"
        :moving-averages="movingAverages"
        :symbol="symbol"
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
  height: 100%;
  min-height: 0;
  min-width: 0;
  overflow: hidden;
  position: relative;
  width: 100%;
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
