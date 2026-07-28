<script setup lang="ts">
import { computed } from 'vue'
import { useRoute, useRouter, type RouteLocationRaw } from 'vue-router'
import { ArrowLeft } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { goBackOr } from '@/core/navigation'

const props = defineProps<{
  title: string
  back?: boolean
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
  <header class="page-header">
    <button v-if="showBack" class="icon-button" type="button" :aria-label="t('common.back')" @click="back">
      <ArrowLeft :size="25" />
    </button>
    <span v-else class="page-header__placeholder" />
    <h1>{{ title }}</h1>
    <div class="page-header__actions"><slot name="actions" /></div>
  </header>
</template>

<style scoped>
.page-header {
  align-items: center;
  background: var(--surface);
  border-bottom: 1px solid var(--line);
  display: grid;
  grid-template-columns: minmax(44px, 1fr) minmax(0, auto) minmax(44px, 1fr);
  isolation: isolate;
  min-height: 56px;
  padding: 0 12px;
  position: sticky;
  top: 0;
  z-index: var(--layer-sticky-header);
}

.page-header h1 {
  font-size: 19px;
  font-weight: 760;
  line-height: 1;
  margin: 0;
  overflow: hidden;
  text-align: center;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.page-header__actions {
  align-items: center;
  display: flex;
  justify-content: flex-end;
  min-height: 44px;
  min-width: 44px;
}

.page-header__placeholder {
  height: 44px;
  width: 44px;
}
</style>
