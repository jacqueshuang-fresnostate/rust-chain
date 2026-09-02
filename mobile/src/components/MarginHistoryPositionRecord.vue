<script setup lang="ts">
import { ChevronRight, Share2 } from 'lucide-vue-next'

interface MarginHistoryMetric {
  label: string
  value: string
  title?: string
  tone?: 'positive' | 'negative' | 'muted'
}

interface MarginHistoryChip {
  label: string
  tone?: 'positive' | 'negative' | 'neutral'
}

defineProps<{
  contractTitle: string
  status: string
  statusTone: 'positive' | 'negative' | 'muted'
  chips: MarginHistoryChip[]
  metrics: MarginHistoryMetric[]
  openedLabel: string
  openedAt: string
  closedLabel: string
  closedAt: string
  associatedLabel: string
  shareLabel: string
}>()

defineEmits<{ associated: []; share: [] }>()
</script>

<template>
  <article class="margin-history-record" role="listitem">
    <header class="margin-history-record__heading">
      <div class="margin-history-record__title">
        <strong :title="contractTitle">{{ contractTitle }}</strong>
        <ChevronRight :size="20" aria-hidden="true" />
      </div>
      <div class="margin-history-record__actions">
        <span :class="`is-${statusTone}`">{{ status }}</span>
        <button type="button" :aria-label="shareLabel" @click="$emit('share')">
          <Share2 :size="20" aria-hidden="true" />
        </button>
      </div>
    </header>

    <div class="margin-history-record__chips">
      <span
        v-for="(chip, index) in chips"
        :key="`${chip.label}-${index}`"
        :class="`is-${chip.tone || 'neutral'}`"
      >{{ chip.label }}</span>
    </div>

    <dl class="margin-history-record__metrics">
      <div v-for="metric in metrics" :key="metric.label">
        <dt :title="metric.label">{{ metric.label }}</dt>
        <dd :class="metric.tone ? `is-${metric.tone}` : undefined" :title="metric.title || metric.value">{{ metric.value }}</dd>
      </div>
    </dl>

    <dl class="margin-history-record__times">
      <div><dt>{{ openedLabel }}</dt><dd>{{ openedAt }}</dd></div>
      <div><dt>{{ closedLabel }}</dt><dd>{{ closedAt }}</dd></div>
    </dl>

    <button class="margin-history-record__associated" type="button" @click="$emit('associated')">
      {{ associatedLabel }}
    </button>
  </article>
</template>

<style scoped>
.margin-history-record {
  background: var(--records-canvas);
  border-bottom: 1px solid var(--records-divider);
  box-sizing: border-box;
  color: var(--records-ink);
  display: grid;
  gap: 16px;
  min-height: 398px;
  min-width: 0;
  padding: 10px 18px 24px;
}

.margin-history-record__heading {
  align-items: center;
  display: flex;
  gap: 12px;
  justify-content: space-between;
  min-width: 0;
}

.margin-history-record__title { align-items: center; display: flex; gap: 7px; min-width: 0; }
.margin-history-record__title strong { font-size: 20px; font-weight: 700; line-height: 28px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.margin-history-record__title svg { color: var(--records-muted); flex: 0 0 auto; }
.margin-history-record__actions { align-items: center; display: flex; flex: 0 0 auto; gap: 12px; }
.margin-history-record__actions > span { font-size: 14px; font-weight: 500; white-space: nowrap; }
.margin-history-record__actions button { background: transparent; border: 0; color: var(--records-ink); display: grid; height: 20px; padding: 0; place-items: center; position: relative; width: 20px; }
.margin-history-record__actions button::before { content: ''; inset: -12px; position: absolute; }
.is-positive { color: var(--records-positive); }
.is-negative { color: var(--records-negative); }
.is-muted { color: var(--records-muted); }

.margin-history-record__chips { display: flex; flex-wrap: wrap; gap: 7px; }
.margin-history-record__chips span { background: var(--records-chip); border-radius: 6px; font-size: 13px; font-weight: 400; line-height: 18px; padding: 5px 9px; }
.margin-history-record__chips span.is-positive { background: var(--records-chip-positive); color: var(--records-positive); }
.margin-history-record__chips span.is-negative { background: var(--records-chip-negative); color: var(--records-negative); }

.margin-history-record__metrics {
  display: grid;
  gap: 16px;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  margin: 0;
  min-width: 0;
}

.margin-history-record__metrics > div { display: grid; gap: 5px; min-width: 0; }
.margin-history-record__metrics > div:nth-child(3n + 2) { text-align: center; }
.margin-history-record__metrics > div:nth-child(3n) { text-align: right; }
.margin-history-record dt { color: var(--records-muted); font-size: 12px; line-height: 18px; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.margin-history-record dd { font-family: var(--font-geist-mono), var(--data-font); font-size: 16px; font-weight: 600; line-height: 22px; margin: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

.margin-history-record__times { display: grid; gap: 8px; margin: 0; }
.margin-history-record__times > div { align-items: center; display: flex; justify-content: space-between; min-width: 0; }
.margin-history-record__times dd { color: var(--records-ink); font-family: var(--font-geist-mono), var(--data-font); font-size: 14px; font-weight: 500; line-height: 20px; }

.margin-history-record__associated {
  align-self: end;
  background: var(--records-button);
  border: 0;
  border-radius: 14px;
  color: var(--records-ink);
  font-size: 16px;
  font-weight: 400;
  height: 52px;
  min-height: 52px;
  width: 100%;
}

.margin-history-record__associated:focus-visible, .margin-history-record__actions button:focus-visible { box-shadow: 0 0 0 2px var(--focus-ring); outline: 0; }
</style>
