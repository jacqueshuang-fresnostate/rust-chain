<template>
  <span class="inline-flex shrink-0 items-center justify-center overflow-hidden rounded-lg border border-border bg-background/60 text-primary">
    <img
      v-if="normalizedSrc"
      class="h-full w-full object-cover"
      :src="normalizedSrc"
      :alt="altText"
      loading="lazy"
    />
    <span v-else class="text-[11px] font-black leading-none">
      {{ fallbackInitial }}
    </span>
  </span>
</template>

<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{
  symbol: string
  src?: string | null
}>()

const normalizedSrc = computed(() => {
  return typeof props.src === 'string' ? props.src.trim() : ''
})

const fallbackInitial = computed(() => {
  const baseSymbol = (props.symbol || '').split('/')[0] || props.symbol || ''
  const initial = baseSymbol.trim().replace(/[^A-Za-z0-9]/g, '').slice(0, 1)
  return initial ? initial.toUpperCase() : '-'
})

const altText = computed(() => `${props.symbol} logo`)
</script>
