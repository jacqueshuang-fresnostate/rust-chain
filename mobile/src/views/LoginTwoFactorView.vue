<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { Check, Copy, LoaderCircle, MailCheck, ShieldCheck } from 'lucide-vue-next'
import { toDataURL } from 'qrcode'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import {
  confirmLoginTwoFactorSetup,
  resetLoginTwoFactor,
  sendLoginTwoFactorResetCode,
  setupLoginTwoFactor,
  submitLoginTwoFactor,
  type LoginTwoFactorSetup,
} from '@/api/auth'
import { useSessionStore } from '@/stores/session'
import {
  createLoginRedirectTarget,
  replaceAuthStep,
  sanitizeInternalRedirect,
} from '@/core/navigation'

const route = useRoute()
const router = useRouter()
const session = useSessionStore()
const { t } = useI18n()
const challengeId = computed(() => typeof route.query.challenge === 'string' ? route.query.challenge : '')
const setupChallengeId = computed(() => typeof route.query.setup === 'string' ? route.query.setup : '')
const code = ref('')
const setup = ref<LoginTwoFactorSetup | null>(null)
const setupQr = ref('')
const setupCode = ref('')
const setupLoading = ref(false)
const setupSubmitting = ref(false)
const copied = ref(false)
const resetCode = ref('')
const resetting = ref(false)
const sending = ref(false)
const submitting = ref(false)
const error = ref('')
const sent = ref(false)
const showReset = ref(false)
const remainingSeconds = ref(0)
let timer: number | undefined
let copiedTimer: number | undefined

const safeRedirect = computed(() => sanitizeInternalRedirect(route.query.redirect))
const loginTarget = computed(() => createLoginRedirectTarget(safeRedirect.value))
const sendLabel = computed(() => remainingSeconds.value ? `${remainingSeconds.value}s` : sent.value ? t('auth.resend') : t('auth.sendResetCode'))
const codeDigits = computed(() => Array.from({ length: 6 }, (_, index) => code.value[index] || ''))
const setupCodeDigits = computed(() => Array.from({ length: 6 }, (_, index) => setupCode.value[index] || ''))

function updateOtp(event: Event, target: 'challenge' | 'setup'): void {
  const input = event.target instanceof HTMLInputElement ? event.target : null
  const value = (input?.value || '').replace(/\D/g, '').slice(0, 6)
  if (target === 'challenge') code.value = value
  else setupCode.value = value
}

function startCountdown(): void {
  remainingSeconds.value = 60
  if (timer) window.clearInterval(timer)
  timer = window.setInterval(() => {
    remainingSeconds.value = Math.max(0, remainingSeconds.value - 1)
    if (!remainingSeconds.value && timer) window.clearInterval(timer)
  }, 1_000)
}

async function submit(): Promise<void> {
  if (!challengeId.value || !code.value.trim()) {
    error.value = t('auth.challengeExpired')
    return
  }
  submitting.value = true
  error.value = ''
  try {
    await submitLoginTwoFactor(challengeId.value, code.value)
    session.sync()
    await replaceAuthStep(router, safeRedirect.value)
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('auth.twoFactorFailed'))
  } finally {
    submitting.value = false
  }
}

async function loadSetup(): Promise<void> {
  if (!setupChallengeId.value) return
  setupLoading.value = true
  setup.value = null
  setupQr.value = ''
  error.value = ''
  try {
    const nextSetup = await setupLoginTwoFactor(setupChallengeId.value)
    setup.value = nextSetup
    setupQr.value = await toDataURL(nextSetup.otpAuthUri, { width: 220, margin: 1 })
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('auth.setupLoadFailed'))
  } finally {
    setupLoading.value = false
  }
}

async function confirmSetup(): Promise<void> {
  if (!setupChallengeId.value || !setup.value || !setupCode.value.trim()) {
    error.value = t('auth.setupCodeRequired')
    return
  }
  setupSubmitting.value = true
  error.value = ''
  try {
    await confirmLoginTwoFactorSetup(setupChallengeId.value, setupCode.value)
    session.sync()
    await replaceAuthStep(router, safeRedirect.value)
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('auth.setupConfirmFailed'))
  } finally {
    setupSubmitting.value = false
  }
}

async function copySecret(): Promise<void> {
  if (!setup.value) return
  try {
    await navigator.clipboard.writeText(setup.value.secret)
  } catch {
    const field = document.createElement('textarea')
    field.value = setup.value.secret
    document.body.appendChild(field)
    field.select()
    document.execCommand('copy')
    field.remove()
  }
  copied.value = true
  if (copiedTimer) window.clearTimeout(copiedTimer)
  copiedTimer = window.setTimeout(() => { copied.value = false }, 1_600)
}

async function sendResetCode(): Promise<void> {
  if (!challengeId.value) {
    error.value = t('auth.challengeExpired')
    return
  }
  sending.value = true
  error.value = ''
  try {
    await sendLoginTwoFactorResetCode(challengeId.value)
    sent.value = true
    showReset.value = true
    startCountdown()
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('auth.resetCodeFailed'))
  } finally {
    sending.value = false
  }
}

async function resetTwoFactor(): Promise<void> {
  if (!challengeId.value || !resetCode.value.trim()) {
    error.value = t('auth.resetCodeRequired')
    return
  }
  resetting.value = true
  error.value = ''
  try {
    await resetLoginTwoFactor(challengeId.value, resetCode.value)
    await returnToLogin()
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('auth.twoFactorResetFailed'))
  } finally {
    resetting.value = false
  }
}

async function returnToLogin(): Promise<void> {
  await replaceAuthStep(router, loginTarget.value)
}

watch(setupChallengeId, () => { void loadSetup() }, { immediate: true })

onUnmounted(() => {
  if (timer) window.clearInterval(timer)
  if (copiedTimer) window.clearTimeout(copiedTimer)
})
</script>

<template>
  <main
    class="page page--plain pencil-page login-two-factor-page"
    data-pencil-source="qmNDA kp9wV"
  >
    <PageHeader
      :back="true"
      :fallback="loginTarget"
      :pencil="true"
      :prefer-fallback="true"
      :title="t('security.twoFactor')"
    />
    <form v-if="challengeId" class="pencil-content login-two-factor-form" :aria-busy="submitting || resetting" @submit.prevent="submit">
      <h1>{{ t('auth.authenticatorTitle') }}</h1>
      <p class="two-factor-subtitle">{{ t('auth.authenticatorDescription') }}</p>

      <label class="otp-control">
        <span class="sr-only">{{ t('auth.verificationCode') }}</span>
        <input :value="code" inputmode="numeric" autocomplete="one-time-code" maxlength="6" :aria-label="t('auth.verificationCode')" @input="updateOtp($event, 'challenge')" />
        <span v-for="(digit, index) in codeDigits" :key="index" :class="{ 'is-current': index === Math.min(code.length, 5) }">{{ digit }}</span>
      </label>

      <div class="two-factor-state-row">
        <span>{{ t('auth.securityNote') }}</span>
        <button type="button" :disabled="sending || remainingSeconds > 0" @click="sendResetCode">
          {{ sending ? t('auth.sendingEllipsis') : sendLabel }}
        </button>
      </div>
      <p v-if="error" class="pencil-message pencil-message--error two-factor-feedback" role="alert">{{ error }}</p>
      <button class="pencil-primary confirm-button" type="submit" :disabled="submitting || code.length !== 6">
        {{ submitting ? t('auth.verifying') : t('auth.confirmLogin') }}
      </button>

      <button class="backup-action" type="button" @click="showReset = !showReset">{{ t('auth.authenticatorUnavailable') }}</button>
      <p class="two-factor-note">{{ t('auth.resetDescription2') }}</p>

      <section v-if="showReset" class="reset-section">
        <strong>{{ t('auth.authenticatorUnavailable') }}</strong>
        <p>{{ t('auth.resetDescription2') }}</p>
        <div class="reset-controls">
          <button class="pencil-secondary" type="button" :disabled="sending || remainingSeconds > 0" @click="sendResetCode">
            {{ sending ? t('auth.sendingEllipsis') : sendLabel }}
          </button>
          <div class="two-factor-field">
            <MailCheck :size="18" />
            <input v-model="resetCode" inputmode="numeric" autocomplete="one-time-code" :placeholder="t('auth.emailResetCode')" />
          </div>
        </div>
        <button class="reset-link" type="button" :disabled="resetting" @click="resetTwoFactor">
          {{ resetting ? t('auth.resetting') : t('auth.resetAndLogin') }}
        </button>
      </section>
    </form>

    <form
      v-else-if="setupChallengeId"
      class="pencil-content login-two-factor-form setup-form"
      :aria-busy="setupLoading || setupSubmitting"
      @submit.prevent="confirmSetup"
    >
      <h1>{{ t('auth.setupRequired') }}</h1>
      <p class="two-factor-subtitle">{{ t('auth.setupRequiredDescription') }}</p>

      <div v-if="setupLoading" class="setup-state" role="status">
        <LoaderCircle :size="23" class="spin" />
        <span>{{ t('auth.setupLoading') }}</span>
      </div>

      <template v-else-if="setup">
        <img v-if="setupQr" :src="setupQr" :alt="t('auth.setupQrAlt')" class="setup-qr" />
        <section class="setup-secret">
          <span>{{ t('auth.manualSecret') }}</span>
          <code>{{ setup.secret }}</code>
          <button type="button" :aria-label="t('auth.copySecret')" @click="copySecret">
            <Check v-if="copied" :size="19" />
            <Copy v-else :size="19" />
          </button>
        </section>
        <p class="setup-hint">{{ t('auth.setupCodeDescription') }}</p>
        <label class="otp-control">
          <span class="sr-only">{{ t('auth.verificationCode') }}</span>
          <input :value="setupCode" inputmode="numeric" autocomplete="one-time-code" maxlength="6" :aria-label="t('auth.verificationCode')" @input="updateOtp($event, 'setup')" />
          <span v-for="(digit, index) in setupCodeDigits" :key="index" :class="{ 'is-current': index === Math.min(setupCode.length, 5) }">{{ digit }}</span>
        </label>
        <p v-if="error" class="pencil-message pencil-message--error two-factor-feedback" role="alert">{{ error }}</p>
        <button class="pencil-primary confirm-button" type="submit" :disabled="setupSubmitting || setupCode.length !== 6">
          {{ setupSubmitting ? t('auth.setupConfirming') : t('auth.confirmSetupAndLogin') }}
        </button>
      </template>

      <div v-else class="setup-state setup-state--error" role="alert">
        <span>{{ error || t('auth.setupLoadFailed') }}</span>
        <button class="pencil-secondary" type="button" :disabled="setupLoading" @click="loadSetup">
          {{ t('common.retry') }}
        </button>
      </div>
    </form>
    <div v-else class="pencil-content setup-required" role="status">
      <span class="setup-required__icon"><ShieldCheck :size="28" /></span>
      <p>{{ t('auth.challengeExpired') }}</p>
      <button class="pencil-secondary" type="button" @click="returnToLogin">{{ t('auth.returnLogin') }}</button>
    </div>
  </main>
</template>

<style scoped>
.page.pencil-page.login-two-factor-page { background: var(--page); background-image: none; min-height: 100dvh; }
.login-two-factor-form { display: flex; flex-direction: column; gap: 14px; padding-bottom: calc(20px + env(safe-area-inset-bottom)); padding-top: 10px; }
.login-two-factor-form h1 { color: var(--ink); font-size: 22px; font-weight: 400; line-height: 31px; margin: 0; }
.two-factor-subtitle { color: var(--muted-strong); font-size: 12px; line-height: 17px; margin: 0; }
.otp-control { display: grid; gap: 8px; grid-template-columns: repeat(6, minmax(0, 1fr)); position: relative; }
.otp-control > input { caret-color: transparent; inset: 0; opacity: 0; position: absolute; width: 100%; z-index: 1; }
.otp-control > span:not(.sr-only) { align-items: center; background: var(--surface); border: 1px solid var(--line); border-radius: 10px; color: var(--ink); display: flex; font-family: var(--font-geist-mono), var(--data-font); font-size: 20px; font-weight: 700; height: 52px; justify-content: center; min-width: 0; }
.otp-control:focus-within > span.is-current { border-color: var(--accent); box-shadow: 0 0 0 2px var(--focus-ring); }
.two-factor-state-row { align-items: center; display: flex; gap: 10px; justify-content: space-between; min-height: 15px; }
.two-factor-state-row > span { color: var(--muted); font-size: 11px; font-weight: 500; line-height: 15px; }
.two-factor-state-row > button { background: transparent; color: var(--positive); flex: 0 0 auto; font-size: 11px; min-height: 15px; padding: 0; position: relative; }
.two-factor-state-row > button::before { content: ''; inset: -14px -4px; position: absolute; }
.two-factor-feedback { margin: 0; }
.confirm-button { font-size: 15px; height: 48px; min-height: 48px; width: 100%; }
.backup-action { background: transparent; color: var(--ink); font-size: 12px; font-weight: 600; min-height: 15px; padding: 0; position: relative; }
.backup-action::before { content: ''; inset: -14px -8px; position: absolute; }
.two-factor-note { color: var(--muted); font-size: 11px; line-height: 16px; margin: 0; text-align: center; }
.reset-section { border-top: 1px solid var(--hairline); display: grid; gap: 11px; padding-top: 14px; }
.reset-section strong { font-size: 15px; }
.reset-section p { color: var(--muted); font-size: 12px; line-height: 1.5; margin: 0; }
.reset-controls { display: grid; gap: 9px; grid-template-columns: 120px minmax(0, 1fr); }
.reset-controls .pencil-secondary { font-size: 11px; min-height: 48px; padding: 0 8px; }
.two-factor-field { align-items: center; background: var(--surface-2); border: 1px solid transparent; border-radius: 8px; color: var(--muted); display: flex; gap: 8px; min-height: 48px; padding: 0 10px; }
.two-factor-field:focus-within { border-color: var(--positive); box-shadow: 0 0 0 2px var(--focus-ring); }
.two-factor-field input { background: transparent; border: 0; color: var(--ink); min-height: 44px; min-width: 0; outline: 0; width: 100%; }
.reset-link { align-items: center; background: transparent; color: var(--accent); display: inline-flex; font-size: 13px; font-weight: 720; justify-self: start; min-height: 44px; padding: 0; text-align: left; }
.setup-required { align-items: center; display: flex; flex-direction: column; gap: 13px; padding-top: 54px !important; text-align: center; }
.setup-required p { color: var(--muted-strong); font-size: 14px; line-height: 1.6; margin: 0; max-width: 320px; }
.setup-required .pencil-secondary { min-height: 48px; min-width: 148px; }
.setup-required__icon { align-items: center; background: var(--accent-soft); border-radius: 50%; color: var(--positive); display: inline-flex; height: 46px; justify-content: center; width: 46px; }
.setup-form { align-content: start; }
.setup-state { align-items: center; color: var(--muted); display: flex; flex-direction: column; gap: 10px; justify-content: center; min-height: 190px; text-align: center; }
.setup-state--error { background: var(--negative-soft); border: 1px solid var(--negative); color: var(--negative); min-height: 150px; padding: 16px; }
.setup-state .pencil-secondary { min-height: 44px; min-width: 132px; }
.setup-qr { align-self: center; background: var(--surface-elevated); border: 1px solid var(--line); display: block; height: 220px; image-rendering: crisp-edges; padding: 8px; width: 220px; }
.setup-secret { align-items: center; background: var(--field-surface); border: 1px solid var(--line); display: grid; gap: 6px 10px; grid-template-columns: minmax(0, 1fr) 44px; padding: 9px 9px 9px 12px; }
.setup-secret > span { color: var(--muted); font-size: 11px; grid-column: 1; }
.setup-secret code { color: var(--ink); font-size: 13px; font-weight: 750; grid-column: 1; overflow-wrap: anywhere; }
.setup-secret button { background: transparent; color: var(--positive); display: grid; grid-column: 2; grid-row: 1 / 3; min-height: 44px; min-width: 44px; place-items: center; }
.setup-hint { color: var(--muted-strong); font-size: 12px; line-height: 1.55; margin: 0; }
.login-two-factor-page button:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
.spin { animation: spin .8s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }
@media (max-width: 340px) {
  .login-two-factor-form,
  .setup-required { padding-inline: 16px; }
  .otp-control { gap: 5px; }
  .reset-controls { grid-template-columns: 1fr; }
  .setup-qr { height: 196px; width: 196px; }
}
@media (prefers-reduced-motion: reduce) {
  .spin { animation: none; }
}
</style>
