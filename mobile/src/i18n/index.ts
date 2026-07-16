import { createI18n } from 'vue-i18n'
import en from './messages/en.ts'
import zhCN from './messages/zh-CN.ts'
import { currentRuntimeIntlLocale, setRuntimeIntlLocale } from '../core/runtimeLocale.ts'

export type MobileLocale = 'zh-CN' | 'en'

const LOCALE_STORAGE_KEY = 'hippo_mobile_locale'

export const SUPPORTED_LOCALES = [
  { code: 'zh-CN' as const, labelKey: 'language.zhCN', descriptionKey: 'language.zhCNDescription', apiLocale: 'zh-CN' },
  { code: 'en' as const, labelKey: 'language.en', descriptionKey: 'language.enDescription', apiLocale: 'en-US' },
]

export function normalizeMobileLocale(value: unknown): MobileLocale | null {
  const locale = String(value || '').trim().replace('_', '-').toLowerCase()
  if (locale === 'zh' || locale.startsWith('zh-')) return 'zh-CN'
  if (locale === 'en' || locale.startsWith('en-')) return 'en'
  return null
}

export function resolveInitialLocale(): MobileLocale {
  try {
    const stored = normalizeMobileLocale(globalThis.localStorage?.getItem(LOCALE_STORAGE_KEY))
    if (stored) return stored
  } catch {
    // H5 隐私模式或原生 WebView 禁用存储时退回系统语言。
  }
  return normalizeMobileLocale(globalThis.navigator?.language) || 'zh-CN'
}

export const i18n = createI18n({
  legacy: false,
  locale: resolveInitialLocale(),
  fallbackLocale: 'zh-CN',
  messages: {
    'zh-CN': zhCN,
    en,
  },
})

export function setAppLocale(locale: MobileLocale): void {
  i18n.global.locale.value = locale
  setRuntimeIntlLocale(locale === 'en' ? 'en-US' : 'zh-CN')
  if (typeof document !== 'undefined') document.documentElement.lang = locale
  try {
    globalThis.localStorage?.setItem(LOCALE_STORAGE_KEY, locale)
  } catch {
    // 语言仍在当前进程内生效，不因持久化失败阻断页面。
  }
}

export function currentApiLocale(): string {
  const current = normalizeMobileLocale(i18n.global.locale.value) || 'zh-CN'
  return SUPPORTED_LOCALES.find((locale) => locale.code === current)?.apiLocale || 'zh-CN'
}

export function currentIntlLocale(): string {
  return currentRuntimeIntlLocale()
}

setAppLocale(normalizeMobileLocale(i18n.global.locale.value) || 'zh-CN')

export default i18n
