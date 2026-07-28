export type ClientPlatform = 'ios_app' | 'android_app' | 'mobile_web' | 'desktop_web'

type TauriGlobal = {
  __TAURI_INTERNALS__?: unknown
}

export function isTauriRuntime(globalObject: object = globalThis): boolean {
  return Boolean((globalObject as TauriGlobal).__TAURI_INTERNALS__)
}

export function detectClientPlatform(
  userAgent = typeof navigator === 'undefined' ? '' : navigator.userAgent,
  globalObject: object = globalThis,
): ClientPlatform {
  const agent = userAgent.toLowerCase()
  const isTauri = isTauriRuntime(globalObject)

  if (isTauri && /iphone|ipad|ipod/.test(agent)) return 'ios_app'
  if (isTauri && /android/.test(agent)) return 'android_app'
  if (/android|iphone|ipad|ipod|mobile/.test(agent)) return 'mobile_web'
  return 'desktop_web'
}
