<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { assetMarkImageSourceAt, buildAssetMarkImageSources } from '@/core/assetMark'

const props = withDefaults(defineProps<{
  symbol: string
  src?: string
  fallbackSrc?: string
  size?: number
}>(), {
  size: 38,
})

const { t } = useI18n()

const initial = computed(() => props.symbol.trim().replace(/[^a-z0-9]/gi, '').slice(0, 1).toUpperCase() || '?')
const imageIndex = ref(0)
const imageSources = computed(() => buildAssetMarkImageSources(props.src, props.fallbackSrc))
const imageSource = computed(() => assetMarkImageSourceAt(imageSources.value, imageIndex.value))
const tone = computed(() => props.symbol.split('').reduce((total, char) => total + char.charCodeAt(0), 0) % 5)
const markStyle = computed(() => ({
  height: `${props.size}px`,
  width: `${props.size}px`,
}))

watch([() => props.src, () => props.fallbackSrc], () => {
  imageIndex.value = 0
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
      v-if="imageSource"
      :key="imageSource"
      :src="imageSource"
      alt=""
      loading="lazy"
      @error="imageIndex += 1"
    />
    <b v-else aria-hidden="true">{{ initial }}</b>
  </span>
</template>

<style scoped>
.asset-mark {
  --asset-color: var(--positive);
  --asset-ink: var(--positive);
  align-items: center;
  background: color-mix(in srgb, var(--asset-color) 10%, var(--surface));
  border: 1px solid color-mix(in srgb, var(--asset-color) 46%, var(--line-strong));
  border-radius: 50%;
  box-shadow:
    inset 0 0 0 3px var(--surface),
    inset 0 0 0 4px color-mix(in srgb, var(--asset-color) 22%, transparent);
  color: var(--asset-ink);
  display: inline-flex;
  flex: 0 0 auto;
  font-size: 15px;
  justify-content: center;
  overflow: hidden;
}

.asset-mark--tone-1 {
  --asset-color: var(--focus);
  --asset-ink: var(--focus);
}

.asset-mark--tone-2 {
  --asset-color: var(--accent);
  --asset-ink: var(--accent);
}

.asset-mark--tone-3 {
  --asset-color: var(--negative);
  --asset-ink: var(--negative);
}

.asset-mark--tone-4 {
  --asset-color: var(--muted-strong);
  --asset-ink: var(--muted-strong);
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
