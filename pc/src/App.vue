<script setup lang="ts">
import { onMounted, ref, defineAsyncComponent } from 'vue'
import { useSettingStore } from '@/stores/setting'
import { useI18n } from 'vue-i18n'

const AppUpdater = defineAsyncComponent(() => import('@/components/common/AppUpdater.vue'))

const settingStore = useSettingStore()
const { locale } = useI18n()
const isTauri = ref(!!(window as any).__TAURI_INTERNALS__)

onMounted(() => {
  settingStore.setTheme(settingStore.theme)
  locale.value = settingStore.locale

  const loader = document.getElementById('global-loader')
  if (loader) {
    loader.classList.add('loader-hidden')
    loader.addEventListener('transitionend', () => loader.remove(), { once: true })
  }
})
</script>

<template>
  <div class="min-h-screen bg-background text-foreground font-sans antialiased transition-colors duration-300">
    <AppUpdater v-if="isTauri" />
    <router-view></router-view>
  </div>
</template>

<style>
/* Global styles are handled by tailwind base in style.css */
</style>
