<script setup lang="ts">
import { onMounted, ref, watch, defineAsyncComponent } from 'vue'
import { useSettingStore } from '@/stores/setting'
import { useI18n } from 'vue-i18n'
import { useUserStore } from '@/stores/user'

const AppUpdater = defineAsyncComponent(() => import('@/components/common/AppUpdater.vue'))

const settingStore = useSettingStore()
const userStore = useUserStore()
const { locale } = useI18n()
const isTauri = ref(!!(window as any).__TAURI_INTERNALS__)

watch(() => settingStore.locale, (nextLocale) => {
  locale.value = nextLocale
})

watch(() => settingStore.platformName, (platformName) => {
  document.title = platformName
}, { immediate: true })

onMounted(() => {
  settingStore.setTheme(settingStore.theme)
  settingStore.applyProfileLocale(userStore.user)
  locale.value = settingStore.locale
  void settingStore.loadPlatformBrand()

  const loader = document.getElementById('global-loader')
  if (loader) {
    const removeLoader = () => loader.remove()
    loader.classList.add('loader-hidden')
    loader.addEventListener('transitionend', removeLoader, { once: true })
    window.setTimeout(removeLoader, 500)
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
