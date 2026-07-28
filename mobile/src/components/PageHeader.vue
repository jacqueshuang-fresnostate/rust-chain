<script setup lang="ts">
import { computed } from 'vue'
import { useRoute, useRouter, type RouteLocationRaw } from 'vue-router'
import { ArrowLeft } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { goBackOr } from '@/core/navigation'

const props = defineProps<{
  title: string
  eyebrow?: string
  subtitle?: string
  back?: boolean
  compact?: boolean
  fallback?: RouteLocationRaw
}>()

const route = useRoute()
const router = useRouter()
const { t } = useI18n()
const showBack = computed(() => props.back ?? (Number(route.meta.depth || 0) > 0))

function back(): void {
  void goBackOr(router, props.fallback || route.meta.backFallback || '/')
}
</script>

<template>
  <header
    class="page-header"
    :class="{
      'page-header--root': !showBack,
      'page-header--compact': compact,
    }"
  >
    <button v-if="showBack" class="icon-button" type="button" :aria-label="t('common.back')" @click="back">
      <ArrowLeft :size="25" />
    </button>
    <div class="page-header__copy">
      <span v-if="eyebrow" class="page-header__eyebrow">{{ eyebrow }}</span>
      <h1>{{ title }}</h1>
      <small v-if="subtitle">{{ subtitle }}</small>
    </div>
    <div class="page-header__actions"><slot name="actions" /></div>
  </header>
</template>

<style scoped>
.page-header {
  align-items: center;
  background: var(--surface);
  border-bottom: 1px solid var(--line);
  display: grid;
  gap: 8px;
  grid-template-columns: 44px minmax(0, 1fr) auto;
  isolation: isolate;
  min-height: 66px;
  padding: 8px 16px;
  position: sticky;
  top: 0;
  z-index: var(--layer-sticky-header);
}

.page-header--root {
  grid-template-columns: minmax(0, 1fr) auto;
}

.page-header--compact {
  min-height: 58px;
}

.page-header__copy {
  display: grid;
  gap: 2px;
  min-width: 0;
  text-align: left;
}

.page-header h1 {
  font-size: 18px;
  font-weight: 760;
  line-height: 1.2;
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.page-header__eyebrow {
  color: var(--muted);
  font-family: var(--data-font);
  font-size: 9px;
  font-weight: 750;
  line-height: 1.2;
  overflow: hidden;
  text-overflow: ellipsis;
  text-transform: uppercase;
  white-space: nowrap;
}

.page-header small {
  color: var(--muted);
  font-size: 10px;
  line-height: 1.3;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.page-header__actions {
  align-items: center;
  display: flex;
  gap: 6px;
  justify-content: flex-end;
  min-height: 44px;
  min-width: 44px;
}

.page-header__actions:empty {
  min-width: 0;
}

@media (max-width: 360px) {
  .page-header {
    padding-inline: 12px;
  }
}
</style>
