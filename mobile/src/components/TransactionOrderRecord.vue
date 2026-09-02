<script setup lang="ts">
import { Share2 } from 'lucide-vue-next'

interface TransactionOrderMetric {
  label: string
  value: string
  title?: string
  tone?: 'positive' | 'negative' | 'muted'
}

interface TransactionOrderRecordModel {
  id: string
  market: 'spot' | 'margin'
  variant: 'current' | 'history'
  symbol: string
  perpetual?: string
  status: string
  statusTone?: 'positive' | 'negative' | 'warning' | 'muted'
  chips: Array<{ label: string; tone?: 'positive' | 'negative' | 'neutral' }>
  time: string
  metrics: TransactionOrderMetric[]
  secondaryMetrics?: TransactionOrderMetric[]
}

defineProps<{
  record: TransactionOrderRecordModel
  cancelLabel?: string
  modifyLabel?: string
  shareLabel?: string
  processing?: boolean
}>()

defineEmits<{
  cancel: [event: MouseEvent]
  share: []
}>()
</script>

<template>
  <article
    class="transaction-order-record"
    :class="[
      `transaction-order-record--${record.variant}`,
      `transaction-order-record--${record.market}`,
    ]"
    role="listitem"
  >
    <header class="transaction-order-record__heading">
      <div class="transaction-order-record__title">
        <strong>{{ record.symbol }}</strong>
        <small v-if="record.perpetual">{{ record.perpetual }}</small>
      </div>
      <div class="transaction-order-record__status-actions">
        <span class="transaction-order-record__status" :class="`is-${record.statusTone || 'muted'}`">{{ record.status }}</span>
        <button v-if="record.variant === 'history' && record.market === 'margin'" type="button" :aria-label="shareLabel" @click="$emit('share')"><Share2 :size="20" aria-hidden="true" /></button>
      </div>
    </header>

    <div class="transaction-order-record__meta">
      <span
        v-for="(chip, index) in record.chips"
        :key="`${chip.label}-${index}`"
        class="transaction-order-record__chip"
        :class="`is-${chip.tone || 'neutral'}`"
      >{{ chip.label }}</span>
      <time>{{ record.time }}</time>
    </div>

    <dl class="transaction-order-record__metrics">
      <div v-for="metric in record.metrics" :key="metric.label">
        <dt :title="metric.label">{{ metric.label }}</dt>
        <dd :class="metric.tone ? `is-${metric.tone}` : undefined" :title="metric.title || metric.value">
          {{ metric.value }}
        </dd>
      </div>
    </dl>

    <dl v-if="record.secondaryMetrics" class="transaction-order-record__secondary-metrics">
      <div v-for="metric in record.secondaryMetrics" :key="metric.label">
        <dt :title="metric.label">{{ metric.label }}</dt>
        <dd :class="metric.tone ? `is-${metric.tone}` : undefined" :title="metric.title || metric.value">{{ metric.value }}</dd>
      </div>
      <div class="is-placeholder" aria-hidden="true" />
    </dl>

    <footer v-if="record.variant === 'current'" class="transaction-order-record__actions">
      <button class="is-modify" type="button" disabled :aria-label="modifyLabel">{{ modifyLabel }}</button>
      <button class="is-cancel" type="button" :disabled="processing" @click="$emit('cancel', $event)">
        {{ processing ? '…' : cancelLabel }}
      </button>
    </footer>
  </article>
</template>

<style scoped>
.transaction-order-record {
  background: var(--records-canvas);
  border-bottom: 1px solid var(--records-divider);
  box-sizing: border-box;
  color: var(--records-ink);
  display: grid;
  min-width: 0;
  width: 100%;
}

.transaction-order-record--current {
  gap: 12px;
  min-height: 238px;
  padding: 14px 18px;
}

.transaction-order-record--history {
  gap: 12px;
  min-height: 174px;
  padding: 12px 18px;
}

.transaction-order-record--history.transaction-order-record--margin {
  min-height: 214px;
}

.transaction-order-record__heading {
  align-items: center;
  display: flex;
  gap: 12px;
  justify-content: space-between;
  min-width: 0;
}

.transaction-order-record__title {
  align-items: baseline;
  display: flex;
  gap: 6px;
  min-width: 0;
}

.transaction-order-record__heading strong {
  font-size: 19px;
  font-weight: 700;
  line-height: 26px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.transaction-order-record__heading small {
  color: var(--records-muted);
  font-size: 12px;
  line-height: 18px;
}

.transaction-order-record__status-actions { align-items: center; display: flex; flex: 0 0 auto; gap: 12px; }
.transaction-order-record__status-actions button {
  background: transparent;
  border: 0;
  color: var(--records-ink);
  display: grid;
  height: 20px;
  min-height: 20px;
  min-width: 20px;
  padding: 0;
  place-items: center;
  position: relative;
  width: 20px;
}
.transaction-order-record__status-actions button::before { content: ''; inset: -12px; position: absolute; }

.transaction-order-record__status {
  flex: 0 0 auto;
  font-size: 14px;
  font-weight: 600;
  line-height: 20px;
}

.transaction-order-record--history .transaction-order-record__status { font-weight: 500; }

.transaction-order-record--current .transaction-order-record__status {
  align-items: center;
  display: inline-flex;
  font-size: 13px;
  gap: 7px;
}

.transaction-order-record--current .transaction-order-record__status::before {
  background: currentColor;
  border-radius: 50%;
  content: '';
  flex: 0 0 7px;
  height: 7px;
  width: 7px;
}

.is-positive { color: var(--records-positive); }
.is-negative { color: var(--records-negative); }
.is-warning { color: #e8b348; }
.is-muted { color: var(--records-muted); }

.transaction-order-record__meta {
  align-items: center;
  display: flex;
  gap: 7px;
  min-width: 0;
  overflow: hidden;
}

.transaction-order-record__meta time {
  color: var(--records-muted);
  font-family: var(--font-geist-mono), var(--data-font);
  font-size: 13px;
  flex: 1 1 auto;
  line-height: 20px;
  margin-left: 0;
  min-width: 0;
  overflow: hidden;
  text-align: left;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.transaction-order-record__chip {
  background: var(--records-chip);
  border-radius: 6px;
  color: var(--records-ink);
  flex: 0 0 auto;
  font-size: 13px;
  font-weight: 400;
  line-height: 18px;
  padding: 4px 7px;
}

.transaction-order-record__chip.is-positive {
  background: var(--records-chip-positive);
  color: var(--records-positive);
}

.transaction-order-record__chip.is-negative {
  background: var(--records-chip-negative);
  color: var(--records-negative);
}

.transaction-order-record__metrics,
.transaction-order-record__secondary-metrics {
  display: grid;
  gap: 12px;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  margin: 0;
  min-width: 0;
}

.transaction-order-record__metrics > div,
.transaction-order-record__secondary-metrics > div {
  display: grid;
  gap: 4px;
  min-width: 0;
}

.transaction-order-record__metrics > div:nth-child(2),
.transaction-order-record__secondary-metrics > div:nth-child(2) {
  text-align: center;
}

.transaction-order-record__metrics > div:nth-child(3),
.transaction-order-record__secondary-metrics > div:nth-child(3) {
  text-align: right;
}

.transaction-order-record dt {
  color: var(--records-muted);
  font-size: 12px;
  font-weight: 500;
  line-height: 18px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.transaction-order-record dd {
  font-family: var(--font-geist-mono), var(--data-font);
  font-size: 16px;
  font-weight: 600;
  line-height: 22px;
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.transaction-order-record--history.transaction-order-record--margin .transaction-order-record__metrics > div:nth-child(3) dd { font-size: 12px; }

.transaction-order-record__secondary-metrics .is-placeholder { height: 1px; }

.transaction-order-record__actions {
  display: grid;
  gap: 10px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.transaction-order-record__actions button {
  background: transparent;
  border: 0;
  border-radius: 12px;
  color: var(--records-ink);
  font-size: 14px;
  font-weight: 600;
  height: 44px;
  min-height: 44px;
  min-width: 0;
  position: relative;
  z-index: 0;
}

.transaction-order-record__actions button::before {
  background: var(--records-button);
  border-radius: 12px;
  content: '';
  inset: 1px 0;
  position: absolute;
  z-index: -1;
}

.transaction-order-record__actions button.is-cancel {
  color: var(--records-negative);
  font-weight: 400;
}

.transaction-order-record__actions button.is-cancel::before {
  background: var(--records-chip-negative);
}

.transaction-order-record__actions button.is-modify:disabled { color: var(--records-ink); }
.transaction-order-record__actions button.is-cancel:disabled { color: var(--records-negative); opacity: .56; }

.transaction-order-record__actions button:focus-visible,
.transaction-order-record__status-actions button:focus-visible {
  box-shadow: 0 0 0 2px var(--focus-ring);
  outline: 0;
}
</style>
