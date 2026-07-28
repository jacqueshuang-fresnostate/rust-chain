<script setup lang="ts">
import { computed, ref } from 'vue'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { CircleAlert, CloudCheck, Download, RefreshCw, WifiOff, X } from 'lucide-vue-next'
import {
  applyPwaUpdate,
  dismissOfflineReady,
  dismissPwaInstall,
  dismissPwaUpdate,
  promptPwaInstall,
  pwaState,
  retryPwaRegistration,
} from '@/pwa'

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
  <aside v-if="visible" class="pwa-status" aria-live="polite">
    <section v-if="!pwaState.isOnline" class="pwa-status__card pwa-status__card--offline" role="status">
      <WifiOff :size="20" aria-hidden="true" />
      <div class="pwa-status__body">
        <strong>{{ t('pwa.offlineTitle') }}</strong>
        <p>{{ t('pwa.offlineDescription') }}</p>
      </div>
    </section>

    <section v-if="showUpdate" class="pwa-status__card pwa-status__card--update" role="status">
      <RefreshCw :size="20" aria-hidden="true" />
      <div class="pwa-status__body">
        <strong>{{ t('pwa.updateTitle') }}</strong>
        <p>{{ t('pwa.updateDescription') }}</p>
        <div class="pwa-status__actions">
          <button class="pwa-status__button pwa-status__button--primary" type="button" :disabled="pwaState.updating" @click="update">
            {{ pwaState.updating ? t('pwa.updating') : t('pwa.updateNow') }}
          </button>
          <button class="pwa-status__button" type="button" :disabled="pwaState.updating" @click="dismissPwaUpdate">
            {{ t('pwa.updateLater') }}
          </button>
        </div>
      </div>
    </section>

    <section v-else-if="showInstall" class="pwa-status__card pwa-status__card--install" role="status">
      <Download :size="20" aria-hidden="true" />
      <div class="pwa-status__body">
        <strong>{{ t(pwaState.iosInstallAvailable ? 'pwa.iosInstallTitle' : 'pwa.installTitle') }}</strong>
        <p>{{ t(pwaState.iosInstallAvailable ? 'pwa.iosInstallDescription' : 'pwa.installDescription') }}</p>
        <button
          v-if="pwaState.installAvailable"
          class="pwa-status__button pwa-status__button--primary"
          type="button"
          :disabled="installing"
          @click="install"
        >
          {{ installing ? t('pwa.installing') : t('pwa.installAction') }}
        </button>
      </div>
      <button class="pwa-status__dismiss" type="button" :aria-label="t('pwa.dismiss')" :title="t('pwa.dismiss')" @click="dismissPwaInstall">
        <X :size="18" aria-hidden="true" />
      </button>
    </section>

    <section v-else-if="showOfflineReady" class="pwa-status__card pwa-status__card--ready" role="status">
      <CloudCheck :size="20" aria-hidden="true" />
      <div class="pwa-status__body">
        <strong>{{ t('pwa.offlineReadyTitle') }}</strong>
        <p>{{ t('pwa.offlineReadyDescription') }}</p>
      </div>
      <button class="pwa-status__dismiss" type="button" :aria-label="t('pwa.dismiss')" :title="t('pwa.dismiss')" @click="dismissOfflineReady">
        <X :size="18" aria-hidden="true" />
      </button>
    </section>

    <section
      v-else-if="showPwaError"
      class="pwa-status__card pwa-status__card--error"
      role="alert"
    >
      <CircleAlert :size="20" aria-hidden="true" />
      <div class="pwa-status__body">
        <strong>{{ t('pwa.errorTitle') }}</strong>
        <p>{{ t(pwaState.installError ? 'pwa.installFailed' : 'pwa.registrationFailed') }}</p>
        <button
          v-if="pwaState.registrationError"
          class="pwa-status__button"
          type="button"
          :disabled="retrying"
          @click="retry"
        >
          {{ retrying ? t('pwa.retrying') : t('pwa.retry') }}
        </button>
      </div>
    </section>
  </aside>
</template>

<style scoped>
.pwa-status {
  display: grid;
  gap: 0;
  left: 50%;
  max-width: var(--app-max-width, 448px);
  pointer-events: none;
  position: fixed;
  top: calc(env(safe-area-inset-top, 0px) + 64px);
  transform: translateX(-50%);
  width: 100%;
  z-index: calc(var(--layer-overlay, 80) + 1);
}

.pwa-status__card {
  align-items: flex-start;
  background: var(--surface-elevated);
  border: 1px solid var(--line);
  border-left-width: 3px;
  border-radius: 0;
  box-shadow: none;
  color: var(--ink);
  display: grid;
  gap: 10px;
  grid-template-columns: 20px minmax(0, 1fr) auto;
  padding: 12px;
  pointer-events: auto;
}

.pwa-status__card--offline,
.pwa-status__card--error {
  border-color: color-mix(in srgb, var(--negative) 52%, var(--line));
}

.pwa-status__card--offline > svg,
.pwa-status__card--error > svg {
  color: var(--negative);
}

.pwa-status__card--install > svg,
.pwa-status__card--update > svg {
  color: var(--accent);
}

.pwa-status__card--ready > svg {
  color: var(--positive);
}

.pwa-status__card--install,
.pwa-status__card--update {
  border-left-color: var(--accent);
}

.pwa-status__card--ready {
  border-left-color: var(--positive);
}

.pwa-status__body {
  min-width: 0;
}

.pwa-status__body strong {
  display: block;
  font-size: 14px;
  line-height: 1.35;
}

.pwa-status__body p {
  color: var(--muted);
  font-size: 12px;
  line-height: 1.5;
  margin: 3px 0 0;
}

.pwa-status__actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 10px;
}

.pwa-status__button,
.pwa-status__dismiss {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--line-strong);
  border-radius: var(--radius);
  color: var(--ink);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 13px;
  font-weight: 700;
  justify-content: center;
  min-height: 44px;
}

.pwa-status__button {
  margin-top: 10px;
  padding: 0 14px;
}

.pwa-status__actions .pwa-status__button {
  margin-top: 0;
}

.pwa-status__button--primary {
  background: var(--accent);
  border-color: var(--accent);
  color: var(--on-accent);
}

.pwa-status__dismiss {
  min-width: 44px;
  padding: 0;
}

.pwa-status__button:focus-visible,
.pwa-status__dismiss:focus-visible {
  outline: 2px solid var(--focus);
  outline-offset: 2px;
}

.pwa-status__button:disabled {
  cursor: wait;
  opacity: .62;
}

@media (max-width: 340px) {
  .pwa-status__card {
    grid-template-columns: 20px minmax(0, 1fr);
  }

  .pwa-status__dismiss {
    grid-column: 2;
    justify-self: end;
  }
}
</style>
