<script setup lang="ts">
import { ReceiptText } from 'lucide-vue-next'

defineProps<{
  title: string
  description: string
}>()
</script>

<template>
  <div class="records-empty" role="status">
    <span class="records-empty__plate" aria-hidden="true"><ReceiptText :size="30" /></span>
    <strong>{{ title }}</strong>
    <span>{{ description }}</span>
    <slot />
  </div>
</template>

<style scoped>
.records-empty {
  align-items: center;
  color: var(--records-muted);
  display: flex;
  flex-direction: column;
  font-size: 11px;
  gap: 12px;
  justify-content: center;
  min-height: 330px;
  padding: 48px 20px;
  text-align: center;
}

.records-empty__plate {
  align-items: center;
  background: var(--records-button);
  border-radius: 50%;
  color: var(--records-muted);
  display: flex;
  height: 64px;
  justify-content: center;
  margin-bottom: 5px;
  width: 64px;
}

.records-empty strong {
  color: var(--records-ink);
  font-size: 18px;
  font-weight: 400;
  line-height: 25px;
}

.records-empty > span:not(.records-empty__plate) {
  font-size: 13px;
  font-weight: 400;
  line-height: 19px;
  max-width: 300px;
}
</style>
