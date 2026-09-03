<script setup lang="ts">
import { ChevronRight } from 'lucide-vue-next'

interface MarginPositionRecordMetric {
  label: string
  value: string
  title?: string
}

interface MarginPositionRecordChip {
  label: string
  tone?: 'positive' | 'negative' | 'neutral'
}

defineProps<{
  id: string
  contractTitle: string
  pnlLabel: string
  pnl: string
  returnRate: string
  pnlTone: 'positive' | 'negative' | 'muted'
  chips: MarginPositionRecordChip[]
  metrics: MarginPositionRecordMetric[]
  tpSlLabel: string
  closeLabel: string
  closeAllLabel: string
  processing?: boolean
  valuesHidden?: boolean
}>()

defineEmits<{
  close: [event: MouseEvent]
  closeAll: [event: MouseEvent]
}>()
</script>

<template>
  <article class="margin-position-record" role="listitem">
    <header class="margin-position-record__heading">
      <div class="margin-position-record__title"><strong :title="contractTitle">{{ contractTitle }}</strong><ChevronRight :size="20" aria-hidden="true" /></div>
      <div class="margin-position-record__pnl" :class="`is-${pnlTone}`">
        <span>{{ pnlLabel }}</span>
        <div :title="`${pnl} (${returnRate})`">
          <strong>{{ valuesHidden ? '••••' : pnl }}</strong>
          <small v-if="!valuesHidden">({{ returnRate }})</small>
        </div>
      </div>
    </header>

    <div class="margin-position-record__chips">
      <span
        v-for="(chip, index) in chips"
        :key="`${chip.label}-${index}`"
        class="margin-position-record__chip"
        :class="`is-${chip.tone || 'neutral'}`"
      >{{ chip.label }}</span>
    </div>

    <dl class="margin-position-record__metrics">
      <div v-for="metric in metrics" :key="metric.label">
        <dt :title="metric.label">{{ metric.label }}</dt>
        <dd :title="metric.title || metric.value">{{ valuesHidden ? '••••' : metric.value }}</dd>
      </div>
    </dl>

    <footer class="margin-position-record__actions">
      <button type="button" disabled>{{ tpSlLabel }}</button>
      <button type="button" :disabled="processing" @click="$emit('close', $event)">{{ closeLabel }}</button>
      <button type="button" :disabled="processing" @click="$emit('closeAll', $event)">{{ closeAllLabel }}</button>
    </footer>
  </article>
</template>

<style scoped>
.margin-position-record {
  background: var(--records-canvas);
  border-bottom: 1px solid var(--records-divider);
  box-sizing: border-box;
  color: var(--records-ink);
  display: grid;
  gap: 12px;
  min-height: 334px;
  min-width: 0;
  padding: 12px 18px;
}

.margin-position-record__heading {
  align-items: flex-start;
  display: flex;
  gap: 12px;
  justify-content: space-between;
  min-width: 0;
}

.margin-position-record__title {
  align-items: center;
  display: flex;
  flex: 0 0 auto;
  gap: 7px;
  max-width: 70%;
  min-width: 0;
}

.margin-position-record__title strong {
  font-size: 20px;
  font-weight: 700;
  line-height: 28px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.margin-position-record__title svg {
  color: var(--records-muted);
  flex: 0 0 auto;
}

.margin-position-record__pnl {
  align-items: end;
  display: grid;
  flex: 1 1 0;
  justify-items: end;
  min-width: 0;
}

.margin-position-record__pnl span {
  color: var(--records-muted);
  font-size: 12px;
  line-height: 18px;
}

.margin-position-record__pnl > div {
  align-items: baseline;
  display: flex;
  gap: 4px;
  max-width: 100%;
  min-width: 0;
  white-space: nowrap;
}

.margin-position-record__pnl strong,
.margin-position-record__pnl small {
  font-family: var(--font-geist-mono), var(--data-font);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.margin-position-record__pnl strong {
  font-size: 15px;
  line-height: 21px;
}

.margin-position-record__pnl small { font-size: 15px; font-weight: 700; line-height: 21px; }
.is-positive { color: var(--records-positive); }
.is-negative { color: var(--records-negative); }
.is-muted { color: var(--records-muted); }

.margin-position-record__chips {
  align-items: flex-start;
  display: flex;
  flex-wrap: wrap;
  gap: 7px;
  max-width: 100%;
  min-width: 0;
}

.margin-position-record__chip {
  align-items: center;
  background: var(--records-chip);
  border-radius: 6px;
  box-sizing: border-box;
  display: inline-flex;
  flex: 0 1 auto;
  font-family: "Noto Sans SC", "PingFang SC", "Hiragino Sans GB", "Microsoft YaHei", sans-serif;
  font-size: 13px;
  font-weight: 650;
  justify-content: center;
  line-height: 18px;
  max-width: 100%;
  min-width: 0;
  overflow: hidden;
  padding: 4px 7px;
  text-align: center;
  text-overflow: ellipsis;
  white-space: nowrap;
  width: fit-content;
}

.margin-position-record__chip.is-positive {
  background: var(--records-chip-positive);
  color: var(--records-positive);
}

.margin-position-record__chip.is-negative {
  background: var(--records-chip-negative);
  color: var(--records-negative);
}

.margin-position-record__metrics {
  display: grid;
  gap: 12px 16px;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  margin: 0;
  min-width: 0;
}

.margin-position-record__metrics > div { display: grid; gap: 3px; min-width: 0; }
.margin-position-record__metrics > div:nth-child(3n + 2) { text-align: center; }
.margin-position-record__metrics > div:nth-child(3n) { text-align: right; }
.margin-position-record__metrics dt { color: var(--records-muted); font-size: 12px; line-height: 18px; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.margin-position-record__metrics dd {
  font-family: var(--font-geist-mono), var(--data-font);
  font-size: 14px;
  font-weight: 600;
  line-height: 20px;
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.margin-position-record__actions {
  align-self: end;
  display: grid;
  gap: 10px;
  grid-template-columns: repeat(3, minmax(0, 1fr));
}

.margin-position-record__actions button {
  background: var(--records-button);
  border: 0;
  border-radius: 12px;
  color: var(--records-ink);
  font-size: 13px;
  font-weight: 600;
  height: 44px;
  min-height: 44px;
  min-width: 0;
  padding: 0 5px;
}

.margin-position-record__actions button:disabled { color: var(--records-muted); }
.margin-position-record__actions button:focus-visible { box-shadow: 0 0 0 2px var(--focus-ring); outline: 0; }
</style>
