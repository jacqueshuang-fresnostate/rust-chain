<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

const props = withDefaults(defineProps<{
  symbol: string
  src?: string
  size?: number
}>(), {
  size: 38,
})

const { t } = useI18n()

const colors = ['#0f766e', '#2563eb', '#b45309', '#7c3aed', '#be123c', '#0369a1']
const initial = computed(() => props.symbol.trim().replace(/[^a-z0-9]/gi, '').slice(0, 1).toUpperCase() || '?')
const color = computed(() => colors[props.symbol.split('').reduce((total, char) => total + char.charCodeAt(0), 0) % colors.length])
</script>

<template>
  <span class="asset-mark" :style="{ '--asset-color': color, width: `${size}px`, height: `${size}px` }">
    <img v-if="src" :src="src" :alt="t('common.assetIcon', { symbol })" loading="lazy" />
    <b v-else>{{ initial }}</b>
  </span>
</template>

<style scoped>
.asset-mark {
  align-items: center;
  background: var(--asset-color);
  border-radius: 50%;
  color: white;
  display: inline-flex;
  flex: 0 0 auto;
  font-size: 15px;
  justify-content: center;
  overflow: hidden;
  box-shadow: inset 0 0 0 1px rgb(255 255 255 / 22%), 0 2px 5px rgb(15 23 42 / 9%);
}

.asset-mark img { height: 100%; object-fit: cover; width: 100%; }
.asset-mark b { font-weight: 750; }
</style>
