export const PWA_INSTALL_SESSION_DELAY_MS = 60_000
export const PWA_INSTALL_FREQUENCY_CAP_MS = 7 * 24 * 60 * 60 * 1000

const PWA_INSTALL_VALUE_ROUTES = new Set([
  'assets',
  'earn',
  'market-detail',
  'message-center',
  'new-coin-detail',
  'new-coins',
  'news-detail',
  'orders',
  'products',
  'seconds',
  'support-chat',
  'swap',
  'trade',
  'wallet-ledger',
])

export interface PwaInstallEligibilityInput {
  now: number
  hasInstallSurface: boolean
  isStandalone: boolean
  lastDismissedAt?: number
  lastShownAt?: number
}

export interface PwaInstallEligibilityResult {
  eligible: boolean
  newlyGranted: boolean
}

export interface PwaInstallEligibilitySession {
  closeOffer(): void
  evaluate(input: PwaInstallEligibilityInput): PwaInstallEligibilityResult
  markOfferShown(): boolean
  recordValueAction(): void
  remainingDelay(now: number): number
}

function isFrequencyCapped(timestamp: number | undefined, now: number): boolean {
  if (!Number.isFinite(timestamp) || Number(timestamp) <= 0) return false
  return now - Number(timestamp) < PWA_INSTALL_FREQUENCY_CAP_MS
}

export function isPwaInstallValueRoute(routeName: unknown): boolean {
  return PWA_INSTALL_VALUE_ROUTES.has(String(routeName || ''))
}

export function createPwaInstallEligibilitySession(
  sessionStartedAt = Date.now(),
): PwaInstallEligibilitySession {
  const startedAt = Number.isFinite(sessionStartedAt) ? sessionStartedAt : Date.now()
  let valueActionCount = 0
  let offerGranted = false
  let offerShown = false

  return {
    closeOffer() {
      offerGranted = false
      offerShown = false
    },

    evaluate(input) {
      if (!input.hasInstallSurface || input.isStandalone) {
        return { eligible: false, newlyGranted: false }
      }

      if (offerGranted) {
        return { eligible: true, newlyGranted: false }
      }

      const delayed = input.now - startedAt >= PWA_INSTALL_SESSION_DELAY_MS
      const frequencyCapped = isFrequencyCapped(input.lastDismissedAt, input.now)
        || isFrequencyCapped(input.lastShownAt, input.now)

      if (!delayed || valueActionCount === 0 || frequencyCapped) {
        return { eligible: false, newlyGranted: false }
      }

      offerGranted = true
      return { eligible: true, newlyGranted: true }
    },

    markOfferShown() {
      if (!offerGranted || offerShown) return false
      offerShown = true
      return true
    },

    recordValueAction() {
      valueActionCount += 1
    },

    remainingDelay(now) {
      return Math.max(0, PWA_INSTALL_SESSION_DELAY_MS - (now - startedAt))
    },
  }
}
