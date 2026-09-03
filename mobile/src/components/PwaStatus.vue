<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import {
  BellRing,
  CircleAlert,
  CloudCheck,
  Download,
  Info,
  Maximize,
  RefreshCw,
  WifiOff,
  X,
  Zap,
} from 'lucide-vue-next'
import brandLogo from '@/assets/brand/hippo-logo-landscape.png'
import { useModalDialog } from '@/core/modalDialog'
import {
  applyPwaUpdate,
  dismissOfflineReady,
  dismissPwaInstall,
  dismissPwaUpdate,
  markPwaInstallOfferShown,
  markPwaInstallValueAction,
  promptPwaInstall,
  pwaState,
  retryPwaRegistration,
} from '@/pwa'
import { isPwaInstallValueRoute } from '@/pwa/eligibility'

const { t } = useI18n()
const route = useRoute()
const installing = ref(false)
const retrying = ref(false)
const installDialog = ref<HTMLElement | null>(null)
const installHint = ref<HTMLElement | null>(null)

const SAFE_PROMPT_ROUTES = new Set([
  'home',
  'markets',
  'market-detail',
  'news',
  'news-detail',
  'message-center',
  'products',
  'profile',
  'language',
])
const promptSafeRoute = computed(() => SAFE_PROMPT_ROUTES.has(String(route.name || '')))
const showInstall = computed(() => (
  promptSafeRoute.value
  && !pwaState.installError
  && (pwaState.installAvailable || pwaState.iosInstallAvailable)
))
const showUpdate = computed(() => promptSafeRoute.value && pwaState.needRefresh && !pwaState.updateDismissed)
const showOfflineReady = computed(() => promptSafeRoute.value && pwaState.offlineReady)
const showPwaError = computed(() => promptSafeRoute.value && (pwaState.registrationError || pwaState.installError))
const primaryState = computed<'update' | 'install' | 'ready' | 'error' | null>(() => {
  if (showUpdate.value) return 'update'
  if (showPwaError.value && pwaState.installError) return 'error'
  if (showInstall.value) return 'install'
  if (showOfflineReady.value) return 'ready'
  if (showPwaError.value) return 'error'
  return null
})
const showInstallDialog = computed(() => (
  pwaState.enabled
  && pwaState.isOnline
  && primaryState.value === 'install'
))
const showStatusIsland = computed(() => pwaState.enabled && (
  !pwaState.isOnline
  || primaryState.value === 'update'
  || primaryState.value === 'ready'
  || primaryState.value === 'error'
))
const { trapFocus: trapInstallFocus } = useModalDialog(
  showInstallDialog,
  installDialog,
  '[data-pwa-install-close]',
)

watch(() => route.fullPath, (currentPath, previousPath) => {
  if ((previousPath === undefined || currentPath !== previousPath) && isPwaInstallValueRoute(route.name)) {
    markPwaInstallValueAction()
  }
}, { immediate: true })

watch(showInstallDialog, (shown) => {
  if (shown) markPwaInstallOfferShown()
}, { immediate: true })

function closeInstallDialog(): void {
  if (installing.value) return
  dismissPwaInstall()
}

function handleInstallDialogKeydown(event: KeyboardEvent): void {
  trapInstallFocus(event, closeInstallDialog)
}

async function install(): Promise<void> {
  if (installing.value) return
  if (pwaState.iosInstallAvailable && !pwaState.installAvailable) {
    installHint.value?.focus()
    return
  }
  if (!pwaState.installAvailable) return

  installing.value = true
  try {
    await promptPwaInstall()
  } finally {
    installing.value = false
  }
}

async function update(): Promise<void> {
  await applyPwaUpdate()
}

async function retry(): Promise<void> {
  retrying.value = true
  try {
    await retryPwaRegistration()
  } finally {
    retrying.value = false
  }
}
</script>

<template>
  <Transition name="pwa-status-reveal">
    <aside v-if="showStatusIsland" class="pwa-status" aria-live="polite" aria-atomic="false">
      <span class="pwa-status__ambient" aria-hidden="true" />

      <section
        v-if="!pwaState.isOnline"
        class="pwa-status__card pwa-status__card--offline"
        data-tone="negative"
        role="status"
      >
        <div class="pwa-status__panel">
          <span class="pwa-status__icon" aria-hidden="true">
            <WifiOff :size="22" :stroke-width="1.7" />
          </span>
          <div class="pwa-status__body">
            <span class="pwa-status__kicker"><i aria-hidden="true" />{{ t('pwa.statusLabel') }}</span>
            <strong>{{ t('pwa.offlineTitle') }}</strong>
            <p>{{ t('pwa.offlineDescription') }}</p>
          </div>
        </div>
      </section>

      <section
        v-if="primaryState === 'update'"
        class="pwa-status__card pwa-status__card--update"
        data-tone="accent"
        :role="pwaState.updateError ? 'alert' : 'status'"
        :aria-busy="pwaState.updating"
      >
        <div class="pwa-status__panel">
          <span class="pwa-status__icon" aria-hidden="true">
            <RefreshCw :size="22" :stroke-width="1.7" :class="{ 'is-spinning': pwaState.updating }" />
          </span>
          <div class="pwa-status__body">
            <span class="pwa-status__kicker"><i aria-hidden="true" />{{ t('pwa.statusLabel') }}</span>
            <strong>{{ t('pwa.updateTitle') }}</strong>
            <p>{{ t(pwaState.updateError ? 'pwa.registrationFailed' : 'pwa.updateDescription') }}</p>
            <div class="pwa-status__actions">
              <button class="pwa-status__button pwa-status__button--primary" type="button" :disabled="pwaState.updating" @click="update">
                <span>{{ pwaState.updating ? t('pwa.updating') : t('pwa.updateNow') }}</span>
                <span class="pwa-status__button-icon" aria-hidden="true">
                  <RefreshCw :size="15" :stroke-width="1.8" :class="{ 'is-spinning': pwaState.updating }" />
                </span>
              </button>
              <button class="pwa-status__button pwa-status__button--secondary" type="button" :disabled="pwaState.updating" @click="dismissPwaUpdate">
                {{ t('pwa.updateLater') }}
              </button>
            </div>
          </div>
        </div>
      </section>

      <section
        v-else-if="primaryState === 'ready'"
        class="pwa-status__card pwa-status__card--ready"
        data-tone="positive"
        role="status"
      >
        <div class="pwa-status__panel">
          <span class="pwa-status__icon" aria-hidden="true">
            <CloudCheck :size="22" :stroke-width="1.7" />
          </span>
          <div class="pwa-status__body">
            <span class="pwa-status__kicker"><i aria-hidden="true" />{{ t('pwa.statusLabel') }}</span>
            <strong>{{ t('pwa.offlineReadyTitle') }}</strong>
            <p>{{ t('pwa.offlineReadyDescription') }}</p>
          </div>
          <button class="pwa-status__dismiss" type="button" :aria-label="t('pwa.dismiss')" :title="t('pwa.dismiss')" @click="dismissOfflineReady">
            <X :size="18" :stroke-width="1.8" aria-hidden="true" />
          </button>
        </div>
      </section>

      <section
        v-else-if="primaryState === 'error'"
        class="pwa-status__card pwa-status__card--error"
        data-tone="negative"
        role="alert"
        :aria-busy="retrying"
      >
        <div class="pwa-status__panel">
          <span class="pwa-status__icon" aria-hidden="true">
            <CircleAlert :size="22" :stroke-width="1.7" />
          </span>
          <div class="pwa-status__body">
            <span class="pwa-status__kicker"><i aria-hidden="true" />{{ t('pwa.statusLabel') }}</span>
            <strong>{{ t('pwa.errorTitle') }}</strong>
            <p>{{ t(pwaState.installError ? 'pwa.installFailed' : 'pwa.registrationFailed') }}</p>
            <div v-if="pwaState.registrationError" class="pwa-status__actions">
              <button
                class="pwa-status__button pwa-status__button--secondary"
                type="button"
                :disabled="retrying"
                @click="retry"
              >
                <span>{{ retrying ? t('pwa.retrying') : t('pwa.retry') }}</span>
                <span class="pwa-status__button-icon" aria-hidden="true">
                  <RefreshCw :size="15" :stroke-width="1.8" :class="{ 'is-spinning': retrying }" />
                </span>
              </button>
            </div>
          </div>
        </div>
      </section>
    </aside>
  </Transition>

  <Teleport to="body">
    <Transition name="pwa-install-modal">
      <div
        v-if="showInstallDialog"
        class="pwa-install"
        data-pencil-source="NROQD FwXCx Tcgl6 V04kP"
        @click.self="closeInstallDialog"
      >
        <section
          id="pwa-install-dialog"
          ref="installDialog"
          class="pwa-install__sheet"
          role="dialog"
          aria-modal="true"
          aria-labelledby="pwa-install-title"
          aria-describedby="pwa-install-description pwa-install-hint"
          :aria-busy="installing"
          tabindex="-1"
          @keydown="handleInstallDialogKeydown"
        >
          <span class="pwa-install__grabber" aria-hidden="true" />

          <header class="pwa-install__header">
            <span class="pwa-install__app-icon" aria-hidden="true">
              <img :src="brandLogo" alt="" />
            </span>
            <div class="pwa-install__heading">
              <h2 id="pwa-install-title">{{ t('pwa.installTitle') }}</h2>
              <span>{{ t('pwa.installSubtitle') }}</span>
            </div>
            <button
              class="pwa-install__close"
              type="button"
              data-pwa-install-close
              :disabled="installing"
              :aria-label="t('pwa.installClose')"
              :title="t('pwa.installClose')"
              @click="closeInstallDialog"
            >
              <span class="pwa-install__close-face">
                <X :size="19" :stroke-width="1.8" aria-hidden="true" />
              </span>
            </button>
          </header>

          <p id="pwa-install-description" class="pwa-install__description">
            {{ t('pwa.installDescription') }}
          </p>

          <ul class="pwa-install__benefits" :aria-label="t('pwa.installBenefitsLabel')">
            <li class="pwa-install__benefit">
              <span class="pwa-install__benefit-icon" aria-hidden="true">
                <Zap :size="16" :stroke-width="1.8" />
              </span>
              <span class="pwa-install__benefit-copy">
                <strong>{{ t('pwa.installFastTitle') }}</strong>
                <span>{{ t('pwa.installFastDescription') }}</span>
              </span>
            </li>
            <li class="pwa-install__benefit">
              <span class="pwa-install__benefit-icon" aria-hidden="true">
                <Maximize :size="16" :stroke-width="1.8" />
              </span>
              <span class="pwa-install__benefit-copy">
                <strong>{{ t('pwa.installImmersiveTitle') }}</strong>
                <span>{{ t('pwa.installImmersiveDescription') }}</span>
              </span>
            </li>
            <li class="pwa-install__benefit">
              <span class="pwa-install__benefit-icon" aria-hidden="true">
                <BellRing :size="16" :stroke-width="1.8" />
              </span>
              <span class="pwa-install__benefit-copy">
                <strong>{{ t('pwa.installNotifyTitle') }}</strong>
                <span>{{ t('pwa.installNotifyDescription') }}</span>
              </span>
            </li>
          </ul>

          <p
            id="pwa-install-hint"
            ref="installHint"
            class="pwa-install__hint"
            role="note"
            tabindex="-1"
          >
            <Info :size="16" :stroke-width="1.8" aria-hidden="true" />
            <span>{{ t('pwa.iosInstallDescription') }}</span>
          </p>

          <button
            class="pwa-install__primary"
            type="button"
            :disabled="installing"
            @click="install"
          >
            <Download :size="19" :stroke-width="2" aria-hidden="true" />
            <span>{{ installing ? t('pwa.installing') : t('pwa.installAction') }}</span>
          </button>

          <button
            class="pwa-install__later"
            type="button"
            :disabled="installing"
            @click="closeInstallDialog"
          >
            {{ t('pwa.installLater') }}
          </button>
        </section>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.pwa-status {
  --pwa-card-radius: 28px;
  --pwa-panel-radius: 24px;

  display: grid;
  gap: 8px;
  isolation: isolate;
  left: 50%;
  max-width: var(--app-max-width, 448px);
  padding: 10px 12px 0;
  pointer-events: none;
  position: fixed;
  top: calc(env(safe-area-inset-top, 0px) + 64px);
  transform: translateX(-50%);
  width: 100%;
  z-index: calc(var(--layer-overlay, 80) + 1);
}

.pwa-status__ambient {
  background:
    radial-gradient(circle at 20% 16%, color-mix(in srgb, var(--accent) 18%, transparent), transparent 42%),
    radial-gradient(circle at 84% 70%, color-mix(in srgb, var(--signal-blue) 12%, transparent), transparent 48%);
  inset: -14px 0 -24px;
  opacity: .72;
  pointer-events: none;
  position: absolute;
  z-index: -1;
}

.pwa-status__card {
  --pwa-tone: var(--accent);
  --pwa-tone-soft: var(--accent-soft);

  background:
    linear-gradient(145deg, color-mix(in srgb, white 58%, transparent), transparent 38%),
    color-mix(in srgb, var(--pwa-tone) 16%, var(--surface-elevated));
  border-radius: var(--pwa-card-radius);
  box-shadow:
    0 18px 48px color-mix(in srgb, var(--dark-surface) 24%, transparent),
    0 4px 14px color-mix(in srgb, var(--pwa-tone) 12%, transparent),
    inset 0 1px 0 color-mix(in srgb, white 64%, transparent),
    0 0 0 1px color-mix(in srgb, var(--pwa-tone) 22%, var(--line));
  color: var(--ink);
  overflow: hidden;
  padding: 3px;
  pointer-events: auto;
  position: relative;
}

.pwa-status__card::before {
  background: radial-gradient(circle, color-mix(in srgb, var(--pwa-tone) 28%, transparent), transparent 66%);
  content: '';
  height: 170px;
  pointer-events: none;
  position: absolute;
  right: -54px;
  top: -82px;
  width: 170px;
}

.pwa-status__card::after {
  background-image:
    linear-gradient(color-mix(in srgb, var(--pwa-tone) 8%, transparent) 1px, transparent 1px),
    linear-gradient(90deg, color-mix(in srgb, var(--pwa-tone) 8%, transparent) 1px, transparent 1px);
  background-size: 13px 13px;
  content: '';
  inset: 0;
  -webkit-mask-image: linear-gradient(110deg, transparent 18%, black 74%, transparent);
  mask-image: linear-gradient(110deg, transparent 18%, black 74%, transparent);
  opacity: .7;
  pointer-events: none;
  position: absolute;
}

.pwa-status__card--offline,
.pwa-status__card--error {
  --pwa-tone: var(--negative);
  --pwa-tone-soft: var(--negative-soft);
}

.pwa-status__card--ready {
  --pwa-tone: var(--positive);
  --pwa-tone-soft: var(--positive-soft);
}

.pwa-status__card--update {
  --pwa-tone: var(--accent);
  --pwa-tone-soft: var(--accent-soft);
}

.pwa-status__panel {
  -webkit-backdrop-filter: blur(22px) saturate(145%);
  align-items: start;
  backdrop-filter: blur(22px) saturate(145%);
  background:
    linear-gradient(142deg, color-mix(in srgb, white 8%, transparent), transparent 42%),
    color-mix(in srgb, var(--surface-elevated) 88%, transparent);
  border: 1px solid color-mix(in srgb, white 24%, var(--line));
  border-radius: var(--pwa-panel-radius);
  box-shadow:
    inset 0 1px 0 color-mix(in srgb, white 34%, transparent),
    inset 0 -1px 0 color-mix(in srgb, var(--pwa-tone) 11%, transparent);
  display: grid;
  gap: 11px;
  grid-template-columns: 46px minmax(0, 1fr) auto;
  overflow: hidden;
  padding: 12px;
  position: relative;
  z-index: 1;
}

.pwa-status__icon {
  align-items: center;
  background:
    radial-gradient(circle at 30% 22%, color-mix(in srgb, white 46%, transparent), transparent 35%),
    linear-gradient(145deg, color-mix(in srgb, var(--pwa-tone-soft) 92%, white), color-mix(in srgb, var(--pwa-tone) 16%, var(--surface)));
  border: 1px solid color-mix(in srgb, var(--pwa-tone) 30%, transparent);
  border-radius: 16px;
  box-shadow:
    inset 0 1px 0 color-mix(in srgb, white 56%, transparent),
    inset 0 -2px 4px color-mix(in srgb, var(--pwa-tone) 12%, transparent),
    0 8px 20px color-mix(in srgb, var(--pwa-tone) 12%, transparent);
  color: var(--pwa-tone);
  display: inline-flex;
  height: 46px;
  justify-content: center;
  position: relative;
  width: 46px;
}

.pwa-status__icon::after {
  animation: pwa-status-breathe 2.8s cubic-bezier(.32, .72, 0, 1) infinite;
  background: var(--pwa-tone);
  border: 2px solid color-mix(in srgb, var(--surface-elevated) 82%, transparent);
  border-radius: 50%;
  bottom: 4px;
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--pwa-tone) 10%, transparent);
  content: '';
  height: 7px;
  position: absolute;
  right: 4px;
  width: 7px;
}

.pwa-status__icon > svg,
.pwa-status__button-icon > svg {
  display: block;
}

.pwa-status__body {
  min-width: 0;
}

.pwa-status__kicker {
  align-items: center;
  color: color-mix(in srgb, var(--pwa-tone) 74%, var(--muted-strong));
  display: inline-flex;
  font-size: 9px;
  font-weight: 760;
  gap: 6px;
  letter-spacing: .12em;
  line-height: 1;
  margin-bottom: 7px;
  text-transform: uppercase;
}

.pwa-status__kicker i {
  background: var(--pwa-tone);
  border-radius: 999px;
  box-shadow: 0 0 8px color-mix(in srgb, var(--pwa-tone) 54%, transparent);
  height: 4px;
  width: 12px;
}

.pwa-status__body strong {
  display: block;
  font-size: 16px;
  font-weight: 760;
  letter-spacing: -.01em;
  line-height: 1.22;
  text-wrap: balance;
}

.pwa-status__body p {
  color: var(--muted);
  font-size: 12px;
  line-height: 1.55;
  margin: 6px 0 0;
  max-width: 36ch;
  text-wrap: pretty;
}

.pwa-status__actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 13px;
}

.pwa-status__button,
.pwa-status__dismiss {
  align-items: center;
  border: 1px solid transparent;
  color: var(--ink);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 12px;
  font-weight: 760;
  justify-content: center;
  min-height: 44px;
  transition:
    transform 420ms cubic-bezier(.32, .72, 0, 1),
    box-shadow 420ms cubic-bezier(.32, .72, 0, 1),
    filter 420ms cubic-bezier(.32, .72, 0, 1),
    opacity 260ms cubic-bezier(.32, .72, 0, 1);
}

.pwa-status__button {
  border-radius: 15px;
  gap: 10px;
  padding: 0 14px;
}

.pwa-status__button--secondary {
  background: color-mix(in srgb, var(--surface) 72%, transparent);
  border-color: color-mix(in srgb, var(--pwa-tone) 18%, var(--line));
  box-shadow:
    inset 0 1px 0 color-mix(in srgb, white 22%, transparent),
    0 5px 14px color-mix(in srgb, var(--dark-surface) 9%, transparent);
}

.pwa-status__button--primary {
  background:
    radial-gradient(circle at 18% 16%, color-mix(in srgb, white 28%, transparent), transparent 36%),
    var(--pwa-tone);
  border-color: color-mix(in srgb, var(--pwa-tone) 74%, var(--line));
  box-shadow:
    inset 0 1px 0 color-mix(in srgb, white 34%, transparent),
    inset 0 -2px 0 color-mix(in srgb, var(--dark-surface) 16%, transparent),
    0 9px 22px color-mix(in srgb, var(--pwa-tone) 22%, transparent);
  color: var(--on-accent);
  justify-content: space-between;
  min-width: 142px;
  padding: 0 7px 0 16px;
}

.pwa-status__button-icon {
  align-items: center;
  background: color-mix(in srgb, var(--surface) 15%, transparent);
  border: 1px solid color-mix(in srgb, white 20%, transparent);
  border-radius: 11px;
  display: inline-flex;
  height: 30px;
  justify-content: center;
  width: 30px;
}

.pwa-status__dismiss {
  background:
    linear-gradient(145deg, color-mix(in srgb, white 12%, transparent), transparent),
    color-mix(in srgb, var(--surface) 70%, transparent);
  border-color: color-mix(in srgb, var(--pwa-tone) 15%, var(--line));
  border-radius: 14px;
  box-shadow:
    inset 0 1px 0 color-mix(in srgb, white 22%, transparent),
    0 6px 16px color-mix(in srgb, var(--dark-surface) 10%, transparent);
  height: 44px;
  min-width: 44px;
  padding: 0;
  width: 44px;
}

.pwa-status__button:not(:disabled):hover,
.pwa-status__dismiss:not(:disabled):hover {
  filter: brightness(1.035) saturate(1.04);
  transform: translateY(-1px);
}

.pwa-status__button:not(:disabled):active,
.pwa-status__dismiss:not(:disabled):active {
  filter: brightness(.985);
  transform: translateY(1px) scale(.985);
}

.pwa-status__button:focus-visible,
.pwa-status__dismiss:focus-visible {
  outline: 2px solid var(--focus);
  outline-offset: 3px;
}

.pwa-status__button:disabled,
.pwa-status__dismiss:disabled {
  box-shadow: none;
  cursor: wait;
  filter: grayscale(.25) saturate(.64);
  opacity: .54;
  transform: none;
}

.is-spinning {
  animation: pwa-status-spin 1.1s cubic-bezier(.45, 0, .55, 1) infinite;
}

.pwa-status-reveal-enter-active {
  transition:
    opacity 460ms cubic-bezier(.32, .72, 0, 1),
    transform 560ms cubic-bezier(.16, 1, .3, 1);
}

.pwa-status-reveal-leave-active {
  transition:
    opacity 260ms cubic-bezier(.4, 0, 1, 1),
    transform 300ms cubic-bezier(.4, 0, 1, 1);
}

.pwa-status-reveal-enter-from,
.pwa-status-reveal-leave-to {
  opacity: 0;
  transform: translate(-50%, -16px) scale(.965);
}

@keyframes pwa-status-breathe {
  0%, 100% {
    opacity: .7;
    transform: scale(.86);
  }
  50% {
    opacity: 1;
    transform: scale(1);
  }
}

@keyframes pwa-status-spin {
  to {
    transform: rotate(360deg);
  }
}

@media (max-width: 340px) {
  .pwa-status {
    padding-inline: 8px;
  }

  .pwa-status__card {
    --pwa-card-radius: 24px;
    --pwa-panel-radius: 21px;
  }

  .pwa-status__panel {
    gap: 9px;
    grid-template-columns: 42px minmax(0, 1fr) auto;
    padding: 10px;
  }

  .pwa-status__icon {
    border-radius: 14px;
    height: 42px;
    width: 42px;
  }

  .pwa-status__body strong {
    font-size: 15px;
  }

  .pwa-status__actions {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
  }

  .pwa-status__button {
    width: 100%;
  }

  .pwa-status__card {
    max-width: 100%;
  }
}

@media (prefers-reduced-motion: reduce) {
  .pwa-status,
  .pwa-status__card,
  .pwa-status__icon::after,
  .pwa-status__button,
  .pwa-status__dismiss,
  .is-spinning,
  .pwa-status-reveal-enter-active,
  .pwa-status-reveal-leave-active {
    animation: none;
    transition: none;
  }

  .pwa-status-reveal-enter-from,
  .pwa-status-reveal-leave-to {
    opacity: 0;
    transform: translateX(-50%);
  }
}

.pwa-install {
  --pwa-install-overlay: #07110c80;
  --pwa-install-sheet: #ffffff;
  --pwa-install-grabber: #cdd7d1;
  --pwa-install-app-icon: #e3faef;
  --pwa-install-app-icon-line: #bcebd6;
  --pwa-install-title: #102018;
  --pwa-install-muted: #64736b;
  --pwa-install-accent: #18d38d;
  --pwa-install-close: #eff8f3;
  --pwa-install-benefits: #eff8f3;
  --pwa-install-benefit-icon: #d8f7e9;
  --pwa-install-hint: #f7faf8;
  --pwa-install-hint-line: #dce9e2;
  --pwa-install-primary-text: #082a1d;

  align-items: flex-end;
  background: var(--pwa-install-overlay);
  box-sizing: border-box;
  display: flex;
  height: 100dvh;
  inset-block: 0;
  left: 50%;
  max-width: var(--app-max-width, 448px);
  overflow: hidden;
  position: fixed;
  transform: translateX(-50%);
  width: 100%;
  z-index: calc(var(--layer-overlay, 80) + 2);
}

.pwa-install *,
.pwa-install *::before,
.pwa-install *::after {
  box-sizing: border-box;
}

.pwa-install__sheet {
  -webkit-overflow-scrolling: touch;
  background: var(--pwa-install-sheet);
  border-radius: 26px 26px 0 0;
  box-shadow: 0 -8px 28px #00000024;
  color: var(--pwa-install-title);
  display: grid;
  flex: 0 0 auto;
  font-family: "Noto Sans SC", "PingFang SC", "Hiragino Sans GB", "Microsoft YaHei", sans-serif;
  gap: 14px;
  grid-template-rows: 82px 44px 146px 42px 54px 38px;
  height: min(540px, 100dvh);
  max-height: 100dvh;
  outline: none;
  overflow-x: hidden;
  overflow-y: auto;
  overscroll-behavior: contain;
  padding: 12px 20px max(22px, env(safe-area-inset-bottom, 0px));
  position: relative;
  scrollbar-width: none;
  width: 100%;
}

.pwa-install__sheet::-webkit-scrollbar {
  display: none;
}

.pwa-install__grabber {
  background: var(--pwa-install-grabber);
  border-radius: 2px;
  height: 4px;
  left: 50%;
  position: absolute;
  top: 6px;
  transform: translateX(-50%);
  width: 42px;
}

.pwa-install__header {
  height: 82px;
  min-width: 0;
  position: relative;
}

.pwa-install__app-icon {
  align-items: center;
  background: var(--pwa-install-app-icon);
  border: 1px solid var(--pwa-install-app-icon-line);
  border-radius: 18px;
  display: flex;
  height: 64px;
  justify-content: center;
  left: 0;
  overflow: hidden;
  position: absolute;
  top: 14px;
  width: 64px;
}

.pwa-install__app-icon img {
  display: block;
  height: 32px;
  object-fit: contain;
  width: 50px;
}

.pwa-install__heading {
  display: grid;
  gap: 4px;
  grid-template-rows: 32px 19px;
  height: 55px;
  left: 78px;
  min-width: 0;
  position: absolute;
  right: 50px;
  top: 18.5px;
}

.pwa-install__heading h2 {
  color: var(--pwa-install-title);
  font-size: 22px;
  font-weight: 700;
  letter-spacing: 0;
  line-height: 32px;
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pwa-install__heading > span {
  color: var(--pwa-install-accent);
  font-size: 13px;
  font-weight: 500;
  line-height: 19px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pwa-install__close {
  align-items: center;
  background: transparent;
  border: 0;
  color: var(--pwa-install-muted);
  cursor: pointer;
  display: flex;
  height: 44px;
  justify-content: center;
  padding: 0;
  position: absolute;
  right: -4px;
  top: 24px;
  width: 44px;
}

.pwa-install__close-face {
  align-items: center;
  background: var(--pwa-install-close);
  border-radius: 18px;
  display: flex;
  height: 36px;
  justify-content: center;
  width: 36px;
}

.pwa-install__description {
  color: var(--pwa-install-muted);
  font-size: 14px;
  font-weight: 500;
  height: 44px;
  line-height: 1.55;
  margin: 0;
}

.pwa-install__benefits {
  background: var(--pwa-install-benefits);
  border-radius: 16px;
  display: grid;
  grid-template-rows: repeat(3, 43px);
  height: 146px;
  list-style: none;
  margin: 0;
  padding: 8px 14px;
}

.pwa-install__benefit {
  height: 43px;
  min-width: 0;
  position: relative;
}

.pwa-install__benefit-icon {
  align-items: center;
  background: var(--pwa-install-benefit-icon);
  border-radius: 9px;
  color: var(--pwa-install-accent);
  display: flex;
  height: 30px;
  justify-content: center;
  left: 0;
  position: absolute;
  top: 6.5px;
  width: 30px;
}

.pwa-install__benefit-copy {
  display: grid;
  gap: 1px;
  grid-template-rows: 20px 17px;
  height: 38px;
  left: 42px;
  min-width: 0;
  position: absolute;
  right: 0;
  top: 2.5px;
}

.pwa-install__benefit-copy strong {
  color: var(--pwa-install-title);
  font-size: 14px;
  font-weight: 650;
  line-height: 20px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pwa-install__benefit-copy > span {
  color: var(--pwa-install-muted);
  font-size: 12px;
  font-weight: 500;
  line-height: 17px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pwa-install__hint {
  align-items: center;
  background: var(--pwa-install-hint);
  border: 1px solid var(--pwa-install-hint-line);
  border-radius: 12px;
  color: var(--pwa-install-muted);
  display: flex;
  font-size: 12px;
  font-weight: 500;
  gap: 8px;
  height: 42px;
  line-height: 18px;
  margin: 0;
  min-width: 0;
  padding: 0 12px;
}

.pwa-install__hint > svg {
  flex: 0 0 auto;
}

.pwa-install__hint > span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pwa-install__primary,
.pwa-install__later {
  -webkit-tap-highlight-color: transparent;
  align-items: center;
  border: 0;
  cursor: pointer;
  display: flex;
  font: inherit;
  justify-content: center;
  padding: 0;
}

.pwa-install__primary {
  background: var(--pwa-install-accent);
  border-radius: 16px;
  box-shadow: 0 6px 16px #18d38d2e;
  color: var(--pwa-install-primary-text);
  font-size: 16px;
  font-weight: 700;
  gap: 8px;
  height: 54px;
  line-height: 22px;
  width: 100%;
}

.pwa-install__later {
  background: transparent;
  color: var(--pwa-install-muted);
  font-size: 14px;
  font-weight: 600;
  height: 38px;
  line-height: 20px;
  min-height: 38px;
  position: relative;
  width: 100%;
}

.pwa-install__later::before {
  content: '';
  inset: -3px 0;
  position: absolute;
}

.pwa-install__close:focus-visible,
.pwa-install__primary:focus-visible,
.pwa-install__later:focus-visible,
.pwa-install__hint:focus {
  outline: 2px solid var(--pwa-install-accent);
  outline-offset: 2px;
}

.pwa-install__close:not(:disabled):active,
.pwa-install__primary:not(:disabled):active,
.pwa-install__later:not(:disabled):active {
  transform: scale(.985);
}

.pwa-install__close:disabled,
.pwa-install__primary:disabled,
.pwa-install__later:disabled {
  cursor: wait;
  opacity: .58;
}

:global(html[data-theme='dark'] .pwa-install) {
  --pwa-install-overlay: #000000b8;
  --pwa-install-sheet: #101a15;
  --pwa-install-grabber: #526159;
  --pwa-install-app-icon: #183d2f;
  --pwa-install-app-icon-line: #2e7057;
  --pwa-install-title: #f4faf7;
  --pwa-install-muted: #9ba8a1;
  --pwa-install-close: #18261f;
  --pwa-install-benefits: #18261f;
  --pwa-install-benefit-icon: #214235;
  --pwa-install-hint: #151f1a;
  --pwa-install-hint-line: #293b32;
}

.pwa-install-modal-enter-active,
.pwa-install-modal-leave-active {
  transition: opacity 240ms cubic-bezier(.32, .72, 0, 1);
}

.pwa-install-modal-enter-active .pwa-install__sheet,
.pwa-install-modal-leave-active .pwa-install__sheet {
  transition: transform 320ms cubic-bezier(.32, .72, 0, 1);
}

.pwa-install-modal-enter-from,
.pwa-install-modal-leave-to {
  opacity: 0;
}

.pwa-install-modal-enter-from .pwa-install__sheet,
.pwa-install-modal-leave-to .pwa-install__sheet {
  transform: translateY(100%);
}

@media (max-width: 340px) {
  .pwa-install__sheet {
    padding-inline: 16px;
  }

  .pwa-install__heading h2 {
    font-size: 20px;
  }

  .pwa-install__close {
    right: 0;
  }

  .pwa-install__description {
    font-size: 13px;
  }

  .pwa-install__benefits {
    padding-inline: 12px;
  }
}

@media (prefers-reduced-motion: reduce) {
  .pwa-install-modal-enter-active,
  .pwa-install-modal-leave-active,
  .pwa-install-modal-enter-active .pwa-install__sheet,
  .pwa-install-modal-leave-active .pwa-install__sheet {
    transition: none;
  }

  .pwa-install-modal-enter-from .pwa-install__sheet,
  .pwa-install-modal-leave-to .pwa-install__sheet {
    transform: none;
  }
}
</style>
