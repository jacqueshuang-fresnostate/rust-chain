const IOS_DEVICE_PATTERN = /iphone|ipad|ipod/i
const MAC_DESKTOP_PATTERN = /macintosh/i

export function isIosBrowser(userAgent: string, maxTouchPoints = 0): boolean {
  return IOS_DEVICE_PATTERN.test(userAgent) || (MAC_DESKTOP_PATTERN.test(userAgent) && maxTouchPoints > 1)
}

export function isStandaloneDisplay(
  matchesDisplayMode: boolean,
  navigatorStandalone: boolean | undefined,
): boolean {
  return matchesDisplayMode || navigatorStandalone === true
}

export function resolveServiceWorkerLocation(base: string, origin: string): {
  scope: string
  scriptUrl: string
} {
  const baseUrl = new URL(base, origin)
  return {
    scope: baseUrl.pathname,
    scriptUrl: new URL('sw.js', baseUrl).toString(),
  }
}
