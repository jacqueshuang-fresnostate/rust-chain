export type ClientPlatform = 'ios_app' | 'android_app' | 'mobile_web' | 'desktop_web'

export function detectClientPlatform(userAgent = typeof navigator === 'undefined' ? '' : navigator.userAgent): ClientPlatform {
  const globalWindow = globalThis as typeof globalThis & { __TAURI_INTERNALS__?: unknown }
  const agent = userAgent.toLowerCase()
  const isTauri = Boolean(globalWindow.__TAURI_INTERNALS__)

  if (isTauri && /iphone|ipad|ipod/.test(agent)) return 'ios_app'
  if (isTauri && /android/.test(agent)) return 'android_app'
  if (/android|iphone|ipad|ipod|mobile/.test(agent)) return 'mobile_web'
  return 'desktop_web'
}

