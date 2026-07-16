import { defineStore } from 'pinia'
import { ref } from 'vue'
import { resolveProfileLocale, type PcLocale, type ProfileLocaleSource } from '@/api/backendAdapters'
import { getPlatformBrand } from '@/api/platform'
import { DEFAULT_CHART_PROVIDER, normalizeChartProvider, type PcChartProvider } from '@/utils/chartProvider'

export const useSettingStore = defineStore('setting', () => {
  const theme = ref<'dark' | 'light'>('dark')
  const locale = ref<PcLocale>('en')
  const localeOverridden = ref(false)
  const platformName = ref('Hippo Exchange')
  const brandLogoUrl = ref('')
  const chartProvider = ref<PcChartProvider>(DEFAULT_CHART_PROVIDER)

  function setTheme(newTheme: 'dark' | 'light') {
    theme.value = newTheme
    if (newTheme === 'dark') {
      document.documentElement.classList.add('dark')
    } else {
      document.documentElement.classList.remove('dark')
    }
  }

  function setLocale(newLocale: PcLocale) {
    locale.value = newLocale
  }

  function setManualLocale(newLocale: PcLocale) {
    localeOverridden.value = true
    setLocale(newLocale)
  }

  function applyProfileLocale(profile?: ProfileLocaleSource | null) {
    locale.value = resolveProfileLocale(profile, locale.value, localeOverridden.value)
  }

  async function loadPlatformBrand() {
    try {
      const response = await getPlatformBrand()
      platformName.value = response.data.platform_name || 'Hippo Exchange'
      brandLogoUrl.value = response.data.logo_url || ''
      chartProvider.value = normalizeChartProvider(response.data.chart_provider)
      document.title = platformName.value
    } catch {
      document.title = platformName.value
    }
  }

  return {
    theme,
    locale,
    localeOverridden,
    platformName,
    brandLogoUrl,
    chartProvider,
    setTheme,
    setLocale,
    setManualLocale,
    applyProfileLocale,
    loadPlatformBrand
  }
}, {
  persist: true
})
