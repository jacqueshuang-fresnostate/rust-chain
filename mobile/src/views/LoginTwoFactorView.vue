<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { Check, Copy, KeyRound, LoaderCircle, MailCheck, QrCode, ShieldCheck } from 'lucide-vue-next'
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
import { sanitizeInternalRedirect } from '@/core/navigation'

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
const remainingSeconds = ref(0)
let timer: number | undefined
let copiedTimer: number | undefined

const sendLabel = computed(() => remainingSeconds.value ? `${remainingSeconds.value}s` : sent.value ? t('auth.resend') : t('auth.sendResetCode'))

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
    const redirect = sanitizeInternalRedirect(route.query.redirect)
    await router.replace(redirect)
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
    const redirect = sanitizeInternalRedirect(route.query.redirect)
    await router.replace(redirect)
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
    await router.replace({ name: 'login' })
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('auth.twoFactorResetFailed'))
  } finally {
    resetting.value = false
  }
}

watch(setupChallengeId, () => { void loadSetup() }, { immediate: true })

onUnmounted(() => {
  if (timer) window.clearInterval(timer)
  if (copiedTimer) window.clearTimeout(copiedTimer)
})
</script>

<template>
  <main class="page page--plain login-two-factor-page">
    <PageHeader
      :back="true"
      :eyebrow="t('auth.authenticatorTitle')"
      :subtitle="t('auth.authenticatorDescription')"
      :title="t('auth.securityVerification')"
    />
    <form v-if="challengeId" class="page-content login-two-factor-form" :aria-busy="submitting || resetting" @submit.prevent="submit">
      <div class="two-factor-intro">
        <span><ShieldCheck :size="23" /></span>
        <div>
          <h1>{{ t('auth.authenticatorTitle') }}</h1>
          <p>{{ t('auth.authenticatorDescription') }}</p>
        </div>
      </div>

      <label>
        <span>{{ t('auth.verificationCode') }}</span>
        <div class="field-shell">
          <KeyRound :size="18" />
          <input v-model="code" inputmode="numeric" autocomplete="one-time-code" maxlength="8" :placeholder="t('auth.codePlaceholder')" />
        </div>
      </label>
      <p v-if="error" class="error-message two-factor-feedback" role="alert">{{ error }}</p>
      <button class="button button--primary button--full confirm-button" type="submit" :disabled="submitting">
        {{ submitting ? t('auth.verifying') : t('auth.confirmLogin') }}
      </button>

      <section class="reset-section">
        <strong>{{ t('auth.authenticatorUnavailable') }}</strong>
        <p>{{ t('auth.resetDescription2') }}</p>
        <div class="reset-controls">
          <button class="button button--secondary" type="button" :disabled="sending || remainingSeconds > 0" @click="sendResetCode">
            {{ sending ? t('auth.sendingEllipsis') : sendLabel }}
          </button>
          <div class="field-shell">
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
      class="page-content login-two-factor-form setup-form"
      :aria-busy="setupLoading || setupSubmitting"
      @submit.prevent="confirmSetup"
    >
      <div class="two-factor-intro">
        <span><QrCode :size="23" /></span>
        <div>
          <h1>{{ t('auth.setupRequired') }}</h1>
          <p>{{ t('auth.setupRequiredDescription') }}</p>
        </div>
      </div>

      <div v-if="setupLoading" class="setup-state" role="status">
        <LoaderCircle :size="23" class="spin" />
        <span>{{ t('auth.setupLoading') }}</span>
      </div>

      <template v-else-if="setup">
        <img v-if="setupQr" :src="setupQr" :alt="t('auth.setupQrAlt')" class="setup-qr" />
        <section class="setup-secret">
          <span>{{ t('auth.manualSecret') }}</span>
          <code>{{ setup.secret }}</code>
          <button class="icon-button" type="button" :aria-label="t('auth.copySecret')" @click="copySecret">
            <Check v-if="copied" :size="19" />
            <Copy v-else :size="19" />
          </button>
        </section>
        <p class="setup-hint">{{ t('auth.setupCodeDescription') }}</p>
        <label>
          <span>{{ t('auth.verificationCode') }}</span>
          <div class="field-shell">
            <KeyRound :size="18" />
            <input
              v-model="setupCode"
              inputmode="numeric"
              autocomplete="one-time-code"
              maxlength="8"
              :placeholder="t('auth.codePlaceholder')"
            />
          </div>
        </label>
        <p v-if="error" class="error-message two-factor-feedback" role="alert">{{ error }}</p>
        <button class="button button--primary button--full confirm-button" type="submit" :disabled="setupSubmitting">
          {{ setupSubmitting ? t('auth.setupConfirming') : t('auth.confirmSetupAndLogin') }}
        </button>
      </template>

      <div v-else class="setup-state setup-state--error" role="alert">
        <span>{{ error || t('auth.setupLoadFailed') }}</span>
        <button class="button button--secondary" type="button" :disabled="setupLoading" @click="loadSetup">
          {{ t('common.retry') }}
        </button>
      </div>
    </form>
    <div v-else class="page-content setup-required" role="status">
      <span class="setup-required__icon"><ShieldCheck :size="28" /></span>
      <p>{{ t('auth.challengeExpired') }}</p>
      <button class="button button--secondary" type="button" @click="router.replace({ name: 'login' })">{{ t('auth.returnLogin') }}</button>
    </div>
  </main>
</template>

<style scoped>
.login-two-factor-page { background: var(--background); }
.login-two-factor-page .page-content { margin: 0 auto; max-width: 448px; padding-bottom: calc(44px + env(safe-area-inset-bottom)); padding-top: 20px; width: 100%; }
.login-two-factor-form { display: grid; gap: 18px; }
.two-factor-intro { align-items: flex-start; border-bottom: 1px solid var(--line); display: flex; gap: 12px; margin-bottom: 4px; padding: 2px 0 20px; }
.two-factor-intro > span,
.setup-required__icon { align-items: center; background: var(--accent-soft); border: 1px solid var(--line); border-radius: var(--radius); color: var(--accent); display: inline-flex; flex: 0 0 46px; height: 46px; justify-content: center; width: 46px; }
.two-factor-intro > div { min-width: 0; }
.two-factor-intro h1 { font-size: 23px; letter-spacing: 0; line-height: 1.2; margin: 0; }
.two-factor-intro p { color: var(--muted-strong); font-size: 13px; line-height: 1.5; margin: 5px 0 0; }
.login-two-factor-form label { display: grid; gap: 8px; }
.login-two-factor-form label > span { font-size: 13px; font-weight: 720; }
.field-shell { align-items: center; background: var(--field-surface); border: 1px solid var(--line); border-radius: var(--radius); color: var(--muted); display: flex; gap: 10px; min-height: 52px; padding: 0 13px; transition: background-color var(--motion-fast) var(--motion-ease), border-color var(--motion-fast) var(--motion-ease), box-shadow var(--motion-fast) var(--motion-ease); }
.field-shell:focus-within { background: var(--surface-elevated); border-color: var(--focus); box-shadow: 0 0 0 3px var(--focus-ring); }
.field-shell input { background: transparent; border: 0; color: var(--ink); font-size: 16px; min-height: 44px; min-width: 0; outline: 0; width: 100%; }
.two-factor-feedback { background: var(--negative-soft); border: 1px solid currentColor; border-radius: var(--radius); margin: 0; padding: 11px 13px; }
.confirm-button { min-height: 52px; }
.reset-section { border-top: 1px solid var(--line); display: grid; gap: 11px; margin-top: 6px; padding-top: 20px; }
.reset-section strong { font-size: 15px; }
.reset-section p { color: var(--muted); font-size: 12px; line-height: 1.5; margin: 0; }
.reset-controls { display: grid; gap: 9px; grid-template-columns: 120px minmax(0, 1fr); }
.reset-controls .button { font-size: 12px; min-height: 52px; padding: 0 8px; }
.reset-link { align-items: center; background: transparent; color: var(--accent); display: inline-flex; font-size: 13px; font-weight: 720; justify-self: start; min-height: 44px; padding: 0; text-align: left; }
.setup-required { align-items: center; display: flex; flex-direction: column; gap: 13px; padding-top: 54px !important; text-align: center; }
.setup-required h1 { font-size: 23px; letter-spacing: 0; margin: 3px 0 0; }
.setup-required p { color: var(--muted-strong); font-size: 14px; line-height: 1.6; margin: 0; max-width: 320px; }
.setup-required .button { min-height: 48px; min-width: 148px; }
.setup-form { align-content: start; }
.setup-state { align-items: center; color: var(--muted); display: flex; flex-direction: column; gap: 10px; justify-content: center; min-height: 190px; text-align: center; }
.setup-state--error { background: var(--negative-soft); border: 1px solid var(--negative); color: var(--negative); min-height: 150px; padding: 16px; }
.setup-state .button { min-height: 44px; min-width: 132px; }
.setup-qr { align-self: center; background: var(--surface-elevated); border: 1px solid var(--line); display: block; height: 220px; image-rendering: crisp-edges; padding: 8px; width: 220px; }
.setup-secret { align-items: center; background: var(--field-surface); border: 1px solid var(--line); display: grid; gap: 6px 10px; grid-template-columns: minmax(0, 1fr) 44px; padding: 9px 9px 9px 12px; }
.setup-secret > span { color: var(--muted); font-size: 11px; grid-column: 1; }
.setup-secret code { color: var(--ink); font-size: 13px; font-weight: 750; grid-column: 1; overflow-wrap: anywhere; }
.setup-secret .icon-button { grid-column: 2; grid-row: 1 / 3; }
.setup-hint { color: var(--muted-strong); font-size: 12px; line-height: 1.55; margin: 0; }
.spin { animation: spin .8s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }
@media (max-width: 340px) {
  .login-two-factor-page .page-content { padding-left: 16px; padding-right: 16px; }
  .reset-controls { grid-template-columns: 1fr; }
  .setup-qr { height: 196px; width: 196px; }
}
@media (prefers-reduced-motion: reduce) {
  .spin { animation: none; }
}
</style>
