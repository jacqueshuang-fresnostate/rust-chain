import { reactive, readonly } from 'vue'
import { isTauriRuntime } from '@/core/platform'
import { createPwaInstallEligibilitySession } from './eligibility'
import { isIosBrowser, isStandaloneDisplay, resolveServiceWorkerLocation } from './runtime'
import { runPwaUpdate } from './update'

interface BeforeInstallPromptEvent extends Event {
  prompt(): Promise<void>
  userChoice: Promise<{
    outcome: 'accepted' | 'dismissed'
    platform: string
  }>
}

const INSTALL_DISMISS_KEY = 'hippo_pwa_install_dismissed_at'
const INSTALL_SHOWN_KEY = 'hippo_pwa_install_shown_at'
const UPDATE_INTERVAL_MS = 60 * 60 * 1000

const state = reactive({
  enabled: false,
  initialized: false,
  installAvailable: false,
  installError: false,
  iosInstallAvailable: false,
  isOnline: true,
  isStandalone: false,
  needRefresh: false,
  offlineReady: false,
  registrationError: false,
  updateDismissed: false,
  updateError: false,
  updating: false,
})

export const pwaState = readonly(state)

let installPrompt: BeforeInstallPromptEvent | null = null
let installEligibility = createPwaInstallEligibilitySession()
let installEligibilityTimer: number | null = null
let registration: ServiceWorkerRegistration | null = null
let initializePromise: Promise<void> | null = null
let reloadTriggered = false
let updatePromise: Promise<boolean> | null = null
let updateGeneration = 0

function navigatorStandalone(): boolean | undefined {
  return (navigator as Navigator & { standalone?: boolean }).standalone
}

function standaloneDisplay(): boolean {
  return isStandaloneDisplay(
    window.matchMedia('(display-mode: standalone)').matches,
    navigatorStandalone(),
  )
}

function readInstallTimestamp(key: string): number | undefined {
  try {
    const value = Number(window.localStorage.getItem(key))
    return Number.isFinite(value) && value > 0 ? value : undefined
  } catch {
    return undefined
  }
}

function persistInstallTimestamp(key: string): void {
  try {
    window.localStorage.setItem(key, String(Date.now()))
  } catch {
    // A private browser context may reject storage while install state can still work in memory.
  }
}

function refreshInstallAvailability(): void {
  state.isStandalone = standaloneDisplay()
  const iosInstallSurface = !installPrompt
    && isIosBrowser(navigator.userAgent, navigator.maxTouchPoints)
  const eligibility = installEligibility.evaluate({
    now: Date.now(),
    hasInstallSurface: Boolean(installPrompt) || iosInstallSurface,
    isStandalone: state.isStandalone,
    lastDismissedAt: readInstallTimestamp(INSTALL_DISMISS_KEY),
    lastShownAt: readInstallTimestamp(INSTALL_SHOWN_KEY),
  })

  state.installAvailable = Boolean(installPrompt) && eligibility.eligible
  state.iosInstallAvailable = iosInstallSurface && eligibility.eligible
}

function scheduleInstallEligibilityRefresh(): void {
  if (installEligibilityTimer !== null) {
    window.clearTimeout(installEligibilityTimer)
    installEligibilityTimer = null
  }

  const remainingDelay = installEligibility.remainingDelay(Date.now())
  if (remainingDelay <= 0) return
  installEligibilityTimer = window.setTimeout(() => {
    installEligibilityTimer = null
    refreshInstallAvailability()
  }, Math.max(1, remainingDelay))
}

function handleBeforeInstallPrompt(event: Event): void {
  const promptEvent = event as BeforeInstallPromptEvent
  promptEvent.preventDefault()
  installPrompt = promptEvent
  state.installError = false
  refreshInstallAvailability()
  scheduleInstallEligibilityRefresh()
}

function handleAppInstalled(): void {
  installPrompt = null
  installEligibility.closeOffer()
  state.installAvailable = false
  state.iosInstallAvailable = false
  state.isStandalone = true
}

function markWorkerInstalled(worker: ServiceWorker): void {
  if (worker.state !== 'installed') return

  if (navigator.serviceWorker.controller) {
    state.needRefresh = true
    state.updateDismissed = false
    state.updateError = false
  } else {
    state.offlineReady = true
  }
}

function observeInstallingWorker(worker: ServiceWorker | null): void {
  if (!worker) return
  markWorkerInstalled(worker)
  worker.addEventListener('statechange', () => markWorkerInstalled(worker))
}

function observeRegistration(currentRegistration: ServiceWorkerRegistration): void {
  if (currentRegistration.waiting) {
    state.needRefresh = true
    state.updateDismissed = false
    state.updateError = false
  } else if (currentRegistration.active && !navigator.serviceWorker.controller) {
    state.offlineReady = true
  }

  observeInstallingWorker(currentRegistration.installing)
  currentRegistration.addEventListener('updatefound', () => {
    observeInstallingWorker(currentRegistration.installing)
  })
}

async function waitForWindowLoad(): Promise<void> {
  if (document.readyState === 'complete') return
  await new Promise<void>((resolve) => {
    window.addEventListener('load', () => resolve(), { once: true })
  })
}

async function registerServiceWorker(): Promise<void> {
  if (!__PWA_ENABLED__ || isTauriRuntime() || !('serviceWorker' in navigator)) return

  await waitForWindowLoad()
  const location = resolveServiceWorkerLocation(import.meta.env.BASE_URL, window.location.origin)

  try {
    registration = await navigator.serviceWorker.register(location.scriptUrl, {
      scope: location.scope,
      updateViaCache: 'none',
    })
    state.registrationError = false
    observeRegistration(registration)
  } catch {
    registration = null
    state.registrationError = true
  }
}

async function checkForUpdate(): Promise<void> {
  if (!state.isOnline || !registration) return
  try {
    await registration.update()
  } catch {
    // Connectivity checks are advisory; the offline banner remains the source of truth.
  }
}

function bindRuntimeEvents(): void {
  window.addEventListener('beforeinstallprompt', handleBeforeInstallPrompt)
  window.addEventListener('appinstalled', handleAppInstalled)
  window.addEventListener('online', () => {
    state.isOnline = true
    void checkForUpdate()
  })
  window.addEventListener('offline', () => {
    state.isOnline = false
  })
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'visible') {
      refreshInstallAvailability()
      void checkForUpdate()
    }
  })
  window.setInterval(() => void checkForUpdate(), UPDATE_INTERVAL_MS)
}

async function initializeBrowserPwa(): Promise<void> {
  installEligibility = createPwaInstallEligibilitySession(Date.now())
  state.enabled = true
  state.isOnline = navigator.onLine
  state.initialized = true
  refreshInstallAvailability()
  scheduleInstallEligibilityRefresh()
  bindRuntimeEvents()
  await registerServiceWorker()
}

export function initializePwa(): Promise<void> {
  if (!__PWA_ENABLED__ || isTauriRuntime() || typeof window === 'undefined' || typeof navigator === 'undefined') {
    return Promise.resolve()
  }

  initializePromise ||= initializeBrowserPwa()
  return initializePromise
}

export async function promptPwaInstall(): Promise<'accepted' | 'dismissed' | 'unavailable'> {
  if (!installPrompt || state.isStandalone) return 'unavailable'

  const currentPrompt = installPrompt
  state.installError = false
  try {
    await currentPrompt.prompt()
    const choice = await currentPrompt.userChoice
    installPrompt = null
    installEligibility.closeOffer()
    if (choice.outcome === 'dismissed') persistInstallTimestamp(INSTALL_DISMISS_KEY)
    refreshInstallAvailability()
    return choice.outcome
  } catch {
    state.installError = true
    return 'unavailable'
  }
}

export function dismissPwaInstall(): void {
  installEligibility.closeOffer()
  persistInstallTimestamp(INSTALL_DISMISS_KEY)
  state.installAvailable = false
  state.iosInstallAvailable = false
}

export function dismissOfflineReady(): void {
  state.offlineReady = false
}

export function dismissPwaUpdate(): void {
  state.updateDismissed = true
  state.updateError = false
}

export function markPwaInstallValueAction(): void {
  installEligibility.recordValueAction()
  if (typeof window === 'undefined' || typeof navigator === 'undefined') return
  refreshInstallAvailability()
  scheduleInstallEligibilityRefresh()
}

/** Frequency-cap only an offer that reached a prompt-safe visible surface. */
export function markPwaInstallOfferShown(): void {
  if (!state.installAvailable && !state.iosInstallAvailable) return
  if (installEligibility.markOfferShown()) persistInstallTimestamp(INSTALL_SHOWN_KEY)
}

export function applyPwaUpdate(): Promise<boolean> {
  if (updatePromise) return updatePromise
  if (!registration || !navigator.serviceWorker) {
    state.updating = false
    state.updateError = true
    return Promise.resolve(false)
  }

  state.registrationError = false
  const currentRegistration = registration
  const attempt = runPwaUpdate({
    registration: currentRegistration,
    controllerTarget: navigator.serviceWorker,
    onBusyChange: (busy) => {
      state.updating = busy
    },
    onErrorChange: (error) => {
      state.updateError = error
    },
    onFailure: () => {
      state.needRefresh = true
      state.updateDismissed = false
    },
    reload: () => {
      if (reloadTriggered) return
      reloadTriggered = true
      window.location.reload()
    },
  })
  const attemptGeneration = ++updateGeneration
  updatePromise = attempt.finally(() => {
    if (updateGeneration === attemptGeneration) updatePromise = null
  })
  return updatePromise
}

export async function retryPwaRegistration(): Promise<void> {
  state.registrationError = false
  await registerServiceWorker()
}
