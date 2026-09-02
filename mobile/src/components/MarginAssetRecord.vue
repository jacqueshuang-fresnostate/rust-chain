<script setup lang="ts">
import { ChevronRight } from 'lucide-vue-next'
import AssetMark from '@/components/AssetMark.vue'

interface MarginAssetMetric {
  label: string
  value: string
  title?: string
  tone?: 'positive' | 'negative' | 'muted'
}

defineProps<{
  symbol: string
  logoUrl?: string
  metrics: MarginAssetMetric[]
  valuesHidden?: boolean
}>()
</script>

<template>
  <article class="margin-asset-record" role="listitem">
    <header class="margin-asset-record__heading">
      <AssetMark :symbol="symbol" :src="logoUrl" :size="30" />
      <strong :title="symbol">{{ symbol }}</strong>
      <ChevronRight :size="20" aria-hidden="true" />
    </header>
    <dl class="margin-asset-record__metrics">
      <div v-for="metric in metrics" :key="metric.label">
        <dt :title="metric.label">{{ metric.label }}</dt>
        <dd :class="metric.tone ? `is-${metric.tone}` : undefined" :title="metric.title || metric.value">
          {{ valuesHidden ? '••••' : metric.value }}
        </dd>
      </div>
    </dl>
  </article>
</template>

<style scoped>
.margin-asset-record {
  background: var(--records-canvas);
  border-bottom: 1px solid var(--records-divider);
  box-sizing: border-box;
  color: var(--records-ink);
  display: grid;
  gap: 14px;
  min-height: 228px;
  min-width: 0;
  padding: 14px 18px;
}

.margin-asset-record__heading {
  align-items: center;
  display: flex;
  gap: 9px;
  min-width: 0;
  width: fit-content;
}

.margin-asset-record__heading strong {
  font-size: 20px;
  font-weight: 700;
  line-height: 28px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.margin-asset-record__heading svg { color: var(--records-muted); flex: 0 0 auto; }

.margin-asset-record__metrics {
  display: grid;
  gap: 14px 16px;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  margin: 0;
  min-width: 0;
}

.margin-asset-record__metrics > div { display: grid; gap: 4px; min-width: 0; }
.margin-asset-record__metrics > div:nth-child(3n + 2) { text-align: center; }
.margin-asset-record__metrics > div:nth-child(3n) { text-align: right; }
.margin-asset-record__metrics dt { color: var(--records-muted); font-size: 12px; line-height: 18px; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.margin-asset-record__metrics dd {
  font-family: var(--font-geist-mono), var(--data-font);
  font-size: 14px;
  font-weight: 600;
  line-height: 20px;
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.is-positive { color: var(--records-positive); }
.is-negative { color: var(--records-negative); }
.is-muted { color: var(--records-muted); }
</style>
