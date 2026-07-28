import { computed, ref } from 'vue'
import { defineStore } from 'pinia'

export type AppTheme = 'light' | 'dark'

export const THEME_STORAGE_KEY = 'hippo_mobile_theme'
export const THEME_META_COLORS: Record<AppTheme, string> = {
  light: '#f1f4f8',
  dark: '#07090c',
}

export function normalizeAppTheme(value: unknown): AppTheme | null {
  return value === 'light' || value === 'dark' ? value : null
}

export function resolveAppTheme(storedValue: unknown, prefersDark = false): AppTheme {
  return normalizeAppTheme(storedValue) || (prefersDark ? 'dark' : 'light')
}

function readInitialTheme(): AppTheme {
  let storedValue: unknown
  try {
    storedValue = globalThis.localStorage?.getItem(THEME_STORAGE_KEY)
  } catch {
    storedValue = null
  }

  const prefersDark = typeof globalThis.matchMedia === 'function'
    && globalThis.matchMedia('(prefers-color-scheme: dark)').matches

  return resolveAppTheme(storedValue, prefersDark)
}

export function applyAppTheme(theme: AppTheme): void {
  if (typeof document === 'undefined') return

  document.documentElement.dataset.theme = theme
  document.documentElement.style.colorScheme = theme

  document.querySelectorAll<HTMLMetaElement>('meta[name="theme-color"]').forEach((themeColor) => {
    themeColor.content = THEME_META_COLORS[theme]
  })
}

export const useThemeStore = defineStore('mobile-theme', () => {
  const theme = ref<AppTheme>(readInitialTheme())
  const isDark = computed(() => theme.value === 'dark')

  function initializeTheme(): void {
    applyAppTheme(theme.value)
  }

  function setTheme(nextTheme: AppTheme): void {
    theme.value = nextTheme
    applyAppTheme(nextTheme)
    try {
      globalThis.localStorage?.setItem(THEME_STORAGE_KEY, nextTheme)
    } catch {
      // Storage may be unavailable in hardened browser and WebView contexts.
    }
  }

  function toggleTheme(): void {
    setTheme(theme.value === 'dark' ? 'light' : 'dark')
  }

  return { theme, isDark, initializeTheme, setTheme, toggleTheme }
})
