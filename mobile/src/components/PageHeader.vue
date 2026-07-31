<script setup lang="ts">
import { computed, useSlots } from 'vue'
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
  preferFallback?: boolean
}>()

const route = useRoute()
const router = useRouter()
const slots = useSlots()
const { t } = useI18n()
const showBack = computed(() => props.back ?? (Number(route.meta.depth || 0) > 0))
const hasActions = computed(() => Boolean(slots.actions))

function back(): void {
  void goBackOr(
    router,
    props.fallback || route.meta.backFallback || '/',
    { preferFallback: props.preferFallback },
  )
}
</script>

<template>
  <header
    class="secondary-header page-header"
    :class="{
      'page-header--root': !showBack,
      'page-header--compact': compact,
    }"
  >
    <button
      class="icon-button page-header__back"
      type="button"
      :data-empty="showBack ? 'false' : 'true'"
      :aria-hidden="showBack ? undefined : 'true'"
      :aria-label="showBack ? t('common.back') : undefined"
      :tabindex="showBack ? undefined : -1"
      @click="back"
    >
      <ArrowLeft :size="20" />
    </button>
    <div class="page-header__copy">
      <span class="secondary-scene page-header__eyebrow">{{ eyebrow || '' }}</span>
      <strong class="page-header__title">{{ title }}</strong>
      <small>{{ subtitle || '' }}</small>
    </div>
    <span
      class="secondary-header-action page-header__actions"
      :data-empty="hasActions ? 'false' : 'true'"
      :aria-hidden="hasActions ? undefined : 'true'"
    >
      <slot name="actions" />
    </span>
    <i class="secondary-header-rail" aria-hidden="true" />
  </header>
</template>

<style scoped>
.page-header__back[data-empty='true'] {
  pointer-events: none;
  visibility: hidden;
}
</style>
