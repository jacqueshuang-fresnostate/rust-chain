<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

const props = withDefaults(defineProps<{
  symbol: string
  src?: string
  size?: number
}>(), {
  size: 38,
})

const { t } = useI18n()

const initial = computed(() => props.symbol.trim().replace(/[^a-z0-9]/gi, '').slice(0, 1).toUpperCase() || '?')
const imageFailed = ref(false)
const tone = computed(() => props.symbol.split('').reduce((total, char) => total + char.charCodeAt(0), 0) % 5)
const markStyle = computed(() => ({
  height: `${props.size}px`,
  width: `${props.size}px`,
}))

watch(() => props.src, () => {
  imageFailed.value = false
})
</script>

<template>
  <span
    class="asset-mark"
    :class="`asset-mark--tone-${tone}`"
    :style="markStyle"
    role="img"
    :aria-label="t('common.assetIcon', { symbol })"
  >
    <img
      v-if="src && !imageFailed"
      :src="src"
      alt=""
      loading="lazy"
      @error="imageFailed = true"
    />
    <b v-else aria-hidden="true">{{ initial }}</b>
  </span>
</template>

<style scoped>
.asset-mark {
  --asset-color: var(--positive);
  --asset-ink: var(--on-positive);
  align-items: center;
  background:
    linear-gradient(145deg, color-mix(in srgb, var(--asset-color) 72%, var(--surface)), var(--asset-color));
  border: 1px solid color-mix(in srgb, var(--asset-color) 58%, var(--line));
  border-radius: 50%;
  box-shadow:
    inset 0 1px 0 color-mix(in srgb, var(--surface) 42%, transparent),
    0 4px 12px color-mix(in srgb, var(--asset-color) 20%, transparent);
  color: var(--asset-ink);
  display: inline-flex;
  flex: 0 0 auto;
  font-size: 15px;
  justify-content: center;
  overflow: hidden;
}

.asset-mark--tone-1 {
  --asset-color: var(--focus);
  --asset-ink: var(--surface);
}

.asset-mark--tone-2 {
  --asset-color: var(--accent);
  --asset-ink: var(--on-accent);
}

.asset-mark--tone-3 {
  --asset-color: var(--negative);
  --asset-ink: var(--on-negative);
}

.asset-mark--tone-4 {
  --asset-color: var(--muted-strong);
  --asset-ink: var(--surface);
}

.asset-mark img {
  background: var(--surface-elevated);
  height: 100%;
  object-fit: cover;
  width: 100%;
}

.asset-mark b {
  font-weight: 800;
  letter-spacing: 0;
}
</style>
