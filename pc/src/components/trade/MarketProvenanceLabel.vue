<script setup lang="ts">
import { marketSource, type MarketProvenance } from '@/api/marketProvenance'
import { computed, onMounted, onUnmounted, ref } from 'vue'
const props = defineProps<{ provenance?: MarketProvenance; observedAt?: number; live?: boolean }>()
const now = ref(Date.now())
let timer: ReturnType<typeof setInterval> | undefined
onMounted(() => { if (props.live) timer = setInterval(() => { now.value = Date.now() }, 1000) })
onUnmounted(() => { if (timer) clearInterval(timer) })
const validTime = computed(() => Number.isFinite(props.observedAt) && Number(props.observedAt) > 0)
const stale = computed(() => props.live && validTime.value && (now.value - Number(props.observedAt) > 60_000 || Number(props.observedAt) > now.value))
const timeText = computed(() => validTime.value ? new Date(Number(props.observedAt)).toLocaleTimeString() : '')
</script>

<template>
  <small
    class="block text-[10px] font-normal text-muted-foreground whitespace-normal break-words leading-tight"
    :title="[provenance?.source, provenance?.provider].filter(Boolean).join(' / ')"
  >{{ $t(`marketProvenance.${marketSource(provenance)}`) }}<template v-if="live || observedAt !== undefined">
    · {{ timeText || $t('marketProvenance.timeUnknown') }}<span v-if="stale"> · {{ $t('marketProvenance.stale') }}</span>
  </template></small>
</template>
