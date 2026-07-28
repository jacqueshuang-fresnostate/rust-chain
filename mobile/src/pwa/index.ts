import { reactive, readonly } from 'vue'
import { isTauriRuntime } from '@/core/platform'
import { isIosBrowser, isStandaloneDisplay, resolveServiceWorkerLocation } from './runtime'

interface BeforeInstallPromptEvent extends Event {
  prompt(): Promise<void>
  userChoice: Promise<{
    outcome: 'accepted' | 'dismissed'
    platform: string
  }>
}

const INSTALL_DISMISS_KEY = 'hippo_pwa_install_dismissed_at'
const INSTALL_DISMISS_DURATION_MS = 7 * 24 * 60 * 60 * 1000
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
  updating: false,
})

export const pwaState = readonly(state)

let installPrompt: BeforeInstallPromptEvent | null = null
let registration: ServiceWorkerRegistration | null = null
let initializePromise: Promise<void> | null = null
let reloadRequested = false
let reloadTriggered = false

function navigatorStandalone(): boolean | undefined {
  return (navigator as Navigator & { standalone?: boolean }).standalone
}

function standaloneDisplay(): boolean {
  return isStandaloneDisplay(
    window.matchMedia('(display-mode: standalone)').matches,
    navigatorStandalone(),
  )
}

function installDismissedRecently(): boolean {
  try {
    const dismissedAt = Number(window.localStorage.getItem(INSTALL_DISMISS_KEY))
    return Number.isFinite(dismissedAt) && Date.now() - dismissedAt < INSTALL_DISMISS_DURATION_MS
  } catch {
    return false
  }
}

function persistInstallDismissal(): void {
  try {
    window.localStorage.setItem(INSTALL_DISMISS_KEY, String(Date.now()))
  } catch {
    // A private browser context may reject storage while install state can still work in memory.
  }
}

function refreshInstallAvailability(): void {
  const dismissed = installDismissedRecently()
  state.isStandalone = standaloneDisplay()
  state.installAvailable = Boolean(installPrompt) && !dismissed && !state.isStandalone
  state.iosInstallAvailable = (
    !installPrompt
    && !dismissed
    && !state.isStandalone
    && isIosBrowser(navigator.userAgent, navigator.maxTouchPoints)
  )
}

function handleBeforeInstallPrompt(event: Event): void {
  const promptEvent = event as BeforeInstallPromptEvent
  promptEvent.preventDefault()
  installPrompt = promptEvent
  state.installError = false
  refreshInstallAvailability()
}

function handleAppInstalled(): void {
  installPrompt = null
  state.installAvailable = false
  state.iosInstallAvailable = false
  state.isStandalone = true
}

function markWorkerInstalled(worker: ServiceWorker): void {
  if (worker.state !== 'installed') return

  if (navigator.serviceWorker.controller) {
    state.needRefresh = true
    state.updateDismissed = false
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

function handleControllerChange(): void {
  if (!reloadRequested || reloadTriggered) return
  reloadTriggered = true
  window.location.reload()
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
  navigator.serviceWorker?.addEventListener('controllerchange', handleControllerChange)
  window.setInterval(() => void checkForUpdate(), UPDATE_INTERVAL_MS)
}

async function initializeBrowserPwa(): Promise<void> {
  state.enabled = true
  state.isOnline = navigator.onLine
  state.initialized = true
  refreshInstallAvailability()
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
    if (choice.outcome === 'dismissed') persistInstallDismissal()
    refreshInstallAvailability()
    return choice.outcome
  } catch {
    state.installError = true
    return 'unavailable'
  }
}

export function dismissPwaInstall(): void {
  persistInstallDismissal()
  state.installAvailable = false
  state.iosInstallAvailable = false
}

export function dismissOfflineReady(): void {
  state.offlineReady = false
}

export function dismissPwaUpdate(): void {
  state.updateDismissed = true
}

export async function applyPwaUpdate(): Promise<boolean> {
  if (!registration) return false

  state.updating = true
  state.registrationError = false
  if (!registration.waiting) {
    await checkForUpdate()
  }

  const waitingWorker = registration.waiting
  if (!waitingWorker) {
    state.updating = false
    return false
  }

  reloadRequested = true
  waitingWorker.postMessage({ type: 'SKIP_WAITING' })
  return true
}

export async function retryPwaRegistration(): Promise<void> {
  state.registrationError = false
  await registerServiceWorker()
}
