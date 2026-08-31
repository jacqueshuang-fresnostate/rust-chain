<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { CircleAlert, CloudCheck, Download, RefreshCw, WifiOff, X } from 'lucide-vue-next'
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
const showInstall = computed(() => promptSafeRoute.value && (pwaState.installAvailable || pwaState.iosInstallAvailable))
const showUpdate = computed(() => promptSafeRoute.value && pwaState.needRefresh && !pwaState.updateDismissed)
const showOfflineReady = computed(() => promptSafeRoute.value && pwaState.offlineReady)
const showPwaError = computed(() => promptSafeRoute.value && (pwaState.registrationError || pwaState.installError))
const visible = computed(() => pwaState.enabled && (
  !pwaState.isOnline
  || showInstall.value
  || showUpdate.value
  || showOfflineReady.value
  || showPwaError.value
))

watch(() => route.fullPath, (currentPath, previousPath) => {
  if ((previousPath === undefined || currentPath !== previousPath) && isPwaInstallValueRoute(route.name)) {
    markPwaInstallValueAction()
  }
}, { immediate: true })

watch(showInstall, (shown) => {
  if (shown) markPwaInstallOfferShown()
}, { immediate: true })

async function install(): Promise<void> {
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
    <aside v-if="visible" class="pwa-status" aria-live="polite" aria-atomic="false">
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
        v-if="showUpdate"
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
        v-else-if="showInstall"
        class="pwa-status__card pwa-status__card--install"
        data-tone="accent"
        role="status"
        :aria-busy="installing"
      >
        <div class="pwa-status__panel">
          <span class="pwa-status__icon" aria-hidden="true">
            <Download :size="22" :stroke-width="1.7" />
          </span>
          <div class="pwa-status__body">
            <span class="pwa-status__kicker"><i aria-hidden="true" />{{ t('pwa.statusLabel') }}</span>
            <strong>{{ t(pwaState.iosInstallAvailable ? 'pwa.iosInstallTitle' : 'pwa.installTitle') }}</strong>
            <p>{{ t(pwaState.iosInstallAvailable ? 'pwa.iosInstallDescription' : 'pwa.installDescription') }}</p>
            <div v-if="pwaState.installAvailable" class="pwa-status__actions">
              <button
                class="pwa-status__button pwa-status__button--primary"
                type="button"
                :disabled="installing"
                @click="install"
              >
                <span>{{ installing ? t('pwa.installing') : t('pwa.installAction') }}</span>
                <span class="pwa-status__button-icon" aria-hidden="true">
                  <Download :size="15" :stroke-width="1.8" />
                </span>
              </button>
            </div>
          </div>
          <button class="pwa-status__dismiss" type="button" :aria-label="t('pwa.dismiss')" :title="t('pwa.dismiss')" @click="dismissPwaInstall">
            <X :size="18" :stroke-width="1.8" aria-hidden="true" />
          </button>
        </div>
      </section>

      <section
        v-else-if="showOfflineReady"
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
        v-else-if="showPwaError"
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

.pwa-status__card--install,
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
</style>
