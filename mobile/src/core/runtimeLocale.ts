let activeIntlLocale = 'zh-CN'

export function setRuntimeIntlLocale(locale: string): void {
  activeIntlLocale = locale || 'zh-CN'
}

export function currentRuntimeIntlLocale(): string {
  return activeIntlLocale
}
