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
    <div class="page-content language-content">
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
          <span class="language-list__mark">{{ option.code === 'zh-CN' ? 'ZH' : 'EN' }}</span>
          <span><b>{{ t(option.labelKey) }}</b><small>{{ t(option.descriptionKey) }}</small></span>
          <Check v-if="currentLocale === option.code" :size="20" />
        </button>
      </div>
      <p v-if="changed" class="language-feedback" role="status">{{ t('language.changed') }}</p>
    </div>
  </main>
</template>

<style scoped>
.language-page { background: var(--background); }
.language-content { display: grid; gap: 22px; margin: 0 auto; max-width: 520px; padding-bottom: calc(44px + env(safe-area-inset-bottom)); padding-top: 18px; width: 100%; }
.language-intro { align-items: flex-start; border-bottom: 1px solid var(--line); display: flex; gap: 12px; padding: 2px 0 20px; }
.language-intro > span { align-items: center; background: var(--accent-soft); border: 1px solid var(--line); border-radius: var(--radius); color: var(--accent); display: inline-flex; flex: 0 0 44px; height: 44px; justify-content: center; width: 44px; }
.language-intro div { display: grid; gap: 4px; min-width: 0; }
.language-intro strong { font-size: 18px; }
.language-intro p { color: var(--muted-strong); font-size: 12px; line-height: 1.45; margin: 0; }
.language-list { border-bottom: 1px solid var(--line); border-top: 1px solid var(--line); display: grid; }
.language-list button { align-items: center; background: transparent; border-bottom: 1px solid var(--line); border-left: 3px solid transparent; display: grid; gap: 12px; grid-template-columns: 44px minmax(0, 1fr) 24px; min-height: 76px; padding: 10px 12px; text-align: left; width: 100%; }
.language-list button:last-child { border-bottom: 0; }
.language-list button.is-active { background: var(--accent-soft); border-left-color: var(--accent); color: var(--accent); }
.language-list__mark { align-items: center; background: var(--soft); border: 1px solid var(--line); border-radius: 50%; color: var(--ink); display: inline-flex; font-size: 11px; font-weight: 800; height: 40px; justify-content: center; width: 40px; }
.language-list .is-active .language-list__mark { background: var(--accent); border-color: var(--accent); color: var(--on-accent); }
.language-list button > span:nth-child(2) { display: grid; gap: 4px; min-width: 0; }
.language-list b { color: var(--ink); font-size: 15px; }
.language-list small { color: var(--muted); font-size: 12px; line-height: 1.35; }
.language-feedback { background: var(--positive-soft); border: 1px solid currentColor; border-radius: var(--radius); color: var(--positive); font-size: 13px; font-weight: 700; margin: 0; padding: 11px 13px; text-align: center; }
@media (max-width: 340px) {
  .language-content { padding-left: 16px; padding-right: 16px; }
  .language-list button { gap: 10px; padding-inline: 9px; }
}
</style>
