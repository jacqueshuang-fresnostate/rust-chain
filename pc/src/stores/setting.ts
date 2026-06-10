import { defineStore } from 'pinia'
import { ref } from 'vue'

export const useSettingStore = defineStore('setting', () => {
  const theme = ref<'dark' | 'light'>('dark')
  const locale = ref<'en' | 'zh'>('en')

  function setTheme(newTheme: 'dark' | 'light') {
    theme.value = newTheme
    if (newTheme === 'dark') {
      document.documentElement.classList.add('dark')
    } else {
      document.documentElement.classList.remove('dark')
    }
  }

  function setLocale(newLocale: 'en' | 'zh') {
    locale.value = newLocale
  }

  return {
    theme,
    locale,
    setTheme,
    setLocale
  }
}, {
  persist: true
})
