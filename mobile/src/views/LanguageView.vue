<script setup lang="ts">
import { computed, ref } from 'vue'
import { Check, Languages } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import PageHeader from '@/components/PageHeader.vue'
import { normalizeMobileLocale, setAppLocale, SUPPORTED_LOCALES, type MobileLocale } from '@/i18n'

const { locale, t } = useI18n()
const changed = ref(false)
const currentLocale = computed(() => normalizeMobileLocale(locale.value) || 'zh-CN')

function selectLocale(nextLocale: MobileLocale): void {
  if (nextLocale === currentLocale.value) return
  setAppLocale(nextLocale)
  changed.value = true
  window.setTimeout(() => { changed.value = false }, 1_600)
}
</script>

<template>
  <main class="page page--plain language-page">
    <PageHeader :title="t('language.title')" />
    <div class="page-content">
      <section class="language-intro">
        <span><Languages :size="23" /></span>
        <div><strong>{{ t('language.entry') }}</strong><p>{{ t('language.description') }}</p></div>
      </section>
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
          <span class="language-list__mark">{{ option.code === 'zh-CN' ? '中' : 'EN' }}</span>
          <span><b>{{ t(option.labelKey) }}</b><small>{{ t(option.descriptionKey) }}</small></span>
          <Check v-if="currentLocale === option.code" :size="20" />
        </button>
      </div>
      <p v-if="changed" class="language-feedback">{{ t('language.changed') }}</p>
    </div>
  </main>
</template>

<style scoped>
.language-page { background: var(--background); }
.language-page .page-content { display: grid; gap: 18px; padding-bottom: 36px; padding-top: 18px; }
.language-intro { align-items: center; background: var(--surface); border: 1px solid var(--line); border-radius: var(--radius); box-shadow: var(--shadow-soft); display: flex; gap: 12px; padding: 16px; }
.language-intro > span { align-items: center; background: var(--positive-soft); border-radius: var(--radius); color: var(--positive); display: inline-flex; height: 44px; justify-content: center; width: 44px; }
.language-intro div { display: grid; gap: 4px; }.language-intro strong { font-size: 17px; }.language-intro p { color: var(--muted); font-size: 12px; line-height: 1.45; margin: 0; }
.language-list { background: var(--surface); border: 1px solid var(--line); border-radius: var(--radius); display: grid; overflow: hidden; }
.language-list button { align-items: center; background: transparent; border-bottom: 1px solid var(--line); display: grid; gap: 12px; grid-template-columns: 44px minmax(0, 1fr) 24px; min-height: 74px; padding: 10px 14px; text-align: left; width: 100%; }.language-list button:last-child { border-bottom: 0; }.language-list button.is-active { background: color-mix(in srgb, var(--positive-soft) 66%, white); color: var(--positive); }
.language-list__mark { align-items: center; background: var(--soft); border-radius: 50%; color: var(--ink); display: inline-flex; font-size: 12px; font-weight: 780; height: 38px; justify-content: center; width: 38px; }.language-list .is-active .language-list__mark { background: var(--positive); color: white; }
.language-list button > span:nth-child(2) { display: grid; gap: 4px; min-width: 0; }.language-list b { color: var(--ink); font-size: 15px; }.language-list small { color: var(--muted); font-size: 12px; }
.language-feedback { color: var(--positive); font-size: 13px; font-weight: 700; margin: 0; text-align: center; }
</style>
