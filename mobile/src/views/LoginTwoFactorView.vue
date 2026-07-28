<script setup lang="ts">
import { computed, onUnmounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { KeyRound, MailCheck, ShieldCheck } from 'lucide-vue-next'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { resetLoginTwoFactor, sendLoginTwoFactorResetCode, submitLoginTwoFactor } from '@/api/auth'
import { useSessionStore } from '@/stores/session'
import { sanitizeInternalRedirect } from '@/core/navigation'

const route = useRoute()
const router = useRouter()
const session = useSessionStore()
const { t } = useI18n()
const challengeId = computed(() => typeof route.query.challenge === 'string' ? route.query.challenge : '')
const setupChallengeId = computed(() => typeof route.query.setup === 'string' ? route.query.setup : '')
const code = ref('')
const resetCode = ref('')
const resetting = ref(false)
const sending = ref(false)
const submitting = ref(false)
const error = ref('')
const sent = ref(false)
const remainingSeconds = ref(0)
let timer: number | undefined

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

onUnmounted(() => { if (timer) window.clearInterval(timer) })
</script>

<template>
  <main class="page page--plain login-two-factor-page">
    <PageHeader :title="t('auth.securityVerification')" />
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

    <div v-else-if="setupChallengeId" class="page-content setup-required" role="status">
      <span class="setup-required__icon"><ShieldCheck :size="28" /></span>
      <h1>{{ t('auth.setupRequired') }}</h1>
      <p>{{ t('auth.setupRequiredDescription') }}</p>
      <button class="button button--secondary" type="button" @click="router.replace({ name: 'login' })">{{ t('auth.returnLogin') }}</button>
    </div>
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
@media (max-width: 340px) {
  .login-two-factor-page .page-content { padding-left: 16px; padding-right: 16px; }
  .reset-controls { grid-template-columns: 1fr; }
}
</style>
