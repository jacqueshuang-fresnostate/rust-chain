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
  pencil?: boolean
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
    :class="[
      pencil ? 'pencil-page-header' : 'secondary-header',
      'page-header',
      {
        'page-header--root': !showBack,
        'page-header--compact': compact,
        'page-header--pencil': pencil,
      },
    ]"
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
      <ArrowLeft :size="pencil ? 22 : 20" />
    </button>
    <div class="page-header__copy">
      <slot name="center">
        <slot name="copy">
          <span class="secondary-scene page-header__eyebrow">{{ eyebrow || '' }}</span>
          <strong class="page-header__title">{{ title }}</strong>
          <small>{{ subtitle || '' }}</small>
        </slot>
      </slot>
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

.pencil-page-header {
  align-items: center;
  background: var(--surface);
  border: 0;
  box-shadow: none;
  display: grid;
  gap: 0;
  grid-template-columns: 44px minmax(0, 1fr) 44px;
  height: 60px;
  isolation: isolate;
  min-height: 60px;
  padding: 8px var(--pencil-header-inline, 18px);
  position: sticky;
  top: 0;
  z-index: var(--layer-sticky-header);
}

.pencil-page-header.page-header--root {
  grid-template-columns: minmax(0, 1fr);
  height: 48px;
  margin-top: var(--pencil-root-header-margin, 10px);
  min-height: 48px;
  padding: 12px 8px 4px 20px;
}

.pencil-page-header.page-header--root .page-header__back {
  display: none;
}

.pencil-page-header.page-header--root .page-header__actions {
  position: absolute;
  right: 8px;
  top: 6px;
}

.pencil-page-header .page-header__copy {
  display: grid;
  gap: 0;
  min-width: 0;
  text-align: center;
}

.pencil-page-header.page-header--root .page-header__copy {
  text-align: left;
}

.pencil-page-header .page-header__eyebrow,
.pencil-page-header .page-header__copy small,
.pencil-page-header .secondary-header-rail {
  display: none;
}

.pencil-page-header .page-header__title {
  color: var(--ink);
  font-size: 18px;
  font-weight: 750;
  line-height: 26px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pencil-page-header.page-header--root .page-header__title {
  font-size: 22px;
  font-weight: 750;
  line-height: 31px;
}

.pencil-page-header.page-header--compact .page-header__title {
  font-size: 15px;
  font-weight: 650;
  line-height: 22px;
}

.pencil-page-header .page-header__actions {
  align-items: center;
  background: transparent;
  border: 0;
  box-shadow: none;
  display: flex;
  gap: 0;
  height: 44px;
  justify-content: center;
  min-width: 44px;
  width: 44px;
}

.pencil-page-header :deep(.icon-button) {
  background: transparent !important;
  border: 0 !important;
  box-shadow: none !important;
  color: var(--ink);
  height: 44px !important;
  min-height: 44px !important;
  padding: 0 !important;
  transform: none !important;
  width: 44px !important;
}

.pencil-page-header :deep(.icon-button:hover),
.pencil-page-header :deep(.icon-button:active) {
  background: transparent !important;
  border: 0 !important;
  box-shadow: none !important;
  transform: none !important;
}

@media (max-width: 340px) {
  .pencil-page-header {
    padding-inline: var(--pencil-header-inline-compact, 14px);
  }

  .pencil-page-header.page-header--root {
    padding-left: 16px;
    padding-right: 6px;
  }
}

@media (prefers-reduced-motion: reduce) {
  .pencil-page-header {
    scroll-behavior: auto;
  }
}
</style>
