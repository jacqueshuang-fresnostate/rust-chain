<script setup lang="ts">
import { computed, ref } from 'vue'
import { Check } from 'lucide-vue-next'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import PageHeader from '@/components/PageHeader.vue'
import { sanitizeInternalRedirect } from '@/core/navigation'
import { normalizeMobileLocale, setAppLocale, SUPPORTED_LOCALES, type MobileLocale } from '@/i18n'

const route = useRoute()
const { locale, t } = useI18n()
const changed = ref(false)
const currentLocale = computed(() => normalizeMobileLocale(locale.value) || 'zh-CN')
const backTarget = computed(() => sanitizeInternalRedirect(route.query.back, '/profile'))

function selectLocale(nextLocale: MobileLocale): void {
  if (nextLocale === currentLocale.value) return
  setAppLocale(nextLocale)
  changed.value = true
  window.setTimeout(() => { changed.value = false }, 1_600)
}
</script>

<template>
  <main
    class="page page--plain pencil-page language-page"
    data-pencil-source="kwFEy yPf6O"
  >
    <PageHeader
      :back="true"
      :fallback="backTarget"
      :pencil="true"
      :title="t('language.entry')"
    />
    <div class="pencil-content language-content">
      <div class="language-list" role="radiogroup" :aria-label="t('language.title')">
        <button
          v-for="option in SUPPORTED_LOCALES"
          :key="option.code"
          type="button"
          role="radio"
          :aria-checked="currentLocale === option.code"
          :class="{ 'is-active': currentLocale === option.code }"
          @click="selectLocale(option.code)"
        >
          <span><b>{{ t(option.labelKey) }}</b><small class="pencil-numeric">{{ option.code }}</small></span>
          <Check v-if="currentLocale === option.code" :size="18" />
        </button>
      </div>
      <p v-if="changed" class="language-feedback" role="status">{{ t('language.changed') }}</p>
      <p class="language-note">{{ t('language.description') }}</p>
    </div>
  </main>
</template>

<style scoped>
.page.pencil-page.language-page { background: var(--page); background-image: none; min-height: 100dvh; }
.language-content { display: flex; flex-direction: column; gap: 8px; padding-bottom: calc(20px + env(safe-area-inset-bottom)); padding-top: 6px; }
.language-list { display: grid; gap: 8px; }
.language-list button { align-items: center; background: transparent; color: var(--ink); display: grid; gap: 12px; grid-template-columns: minmax(0, 1fr) 18px; height: 52px; min-height: 52px; padding: 0; text-align: left; width: 100%; }
.language-list button > span { display: grid; gap: 3px; min-width: 0; }
.language-list b { color: var(--ink); font-size: 14px; font-weight: 700; line-height: 20px; }
.language-list small { color: var(--muted); font-size: 11px; font-weight: 500; line-height: 15px; }
.language-list svg { color: var(--positive); }
.language-list button:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
.language-feedback { background: var(--positive-soft); border-radius: 8px; color: var(--positive); font-size: 11px; font-weight: 600; margin: 0; min-height: 44px; padding: 14px 12px; }
.language-note { color: var(--muted); font-size: 11px; line-height: 16px; margin: 0; }
@media (max-width: 340px) {
  .language-content { padding-inline: 16px; }
}
</style>
