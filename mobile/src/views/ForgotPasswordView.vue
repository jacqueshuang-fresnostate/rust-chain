<script setup lang="ts">
import { computed, onUnmounted, ref } from 'vue'
import { useRoute, useRouter, type RouteLocationRaw } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { KeyRound, MailCheck } from 'lucide-vue-next'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { resetPasswordWithCode, sendPasswordResetCode } from '@/api/auth'
import {
  createLoginRedirectTarget,
  replaceAuthStep,
  sanitizeInternalRedirect,
} from '@/core/navigation'

const route = useRoute()
const router = useRouter()
const { t } = useI18n()
const email = ref('')
const code = ref('')
const password = ref('')
const confirmation = ref('')
const remainingSeconds = ref(0)
const sending = ref(false)
const submitting = ref(false)
const error = ref('')
const success = ref('')
const currentStep = ref<1 | 2 | 3>(1)
let timer: number | undefined

const safeRedirect = computed(() => sanitizeInternalRedirect(route.query.redirect))
const loginTarget = computed<RouteLocationRaw>(() => createLoginRedirectTarget(safeRedirect.value))
const sendLabel = computed(() => remainingSeconds.value ? `${remainingSeconds.value}s` : t('auth.sendCode'))

function startCountdown() {
  remainingSeconds.value = 60
  if (timer) window.clearInterval(timer)
  timer = window.setInterval(() => {
    remainingSeconds.value = Math.max(0, remainingSeconds.value - 1)
    if (!remainingSeconds.value && timer) window.clearInterval(timer)
  }, 1_000)
}

async function sendCode() {
  error.value = ''
  if (!email.value.trim()) { error.value = t('auth.emailRequired'); return }
  sending.value = true
  try {
    await sendPasswordResetCode(email.value)
    startCountdown()
    currentStep.value = 2
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('auth.codeSendFailed'))
  } finally {
    sending.value = false
  }
}

async function submit() {
  error.value = ''
  success.value = ''
  if (!email.value.trim() || !code.value.trim() || !password.value) { error.value = t('auth.completeReset'); return }
  if (password.value !== confirmation.value) { error.value = t('auth.passwordMismatch'); return }
  submitting.value = true
  try {
    await resetPasswordWithCode({ email: email.value, code: code.value, password: password.value })
    success.value = t('auth.passwordUpdated')
    window.setTimeout(() => { void replaceAuthStep(router, loginTarget.value) }, 900)
  } catch (reason) { error.value = apiErrorMessage(reason, t('auth.resetFailed')) } finally { submitting.value = false }
}

async function advance(): Promise<void> {
  error.value = ''
  if (currentStep.value === 1) {
    await sendCode()
    return
  }
  if (currentStep.value === 2) {
    if (!code.value.trim()) {
      error.value = t('auth.completeReset')
      return
    }
    currentStep.value = 3
    return
  }
  await submit()
}

onUnmounted(() => { if (timer) window.clearInterval(timer) })
</script>

<template>
  <main
    class="page page--plain pencil-page auth-page"
    data-pencil-source="mgAF7 HrPy2"
  >
    <PageHeader
      :back="true"
      :fallback="loginTarget"
      :pencil="true"
      :prefer-fallback="true"
      :title="t('auth.forgotTitle')"
    />
    <form class="pencil-content auth-form" :aria-busy="sending || submitting" @submit.prevent="advance">
      <h1>{{ t('auth.resetTitle') }}</h1>
      <p class="auth-subtitle">{{ t('auth.resetDescription') }}</p>

      <ol class="reset-steps" :aria-label="t('auth.stepProgress', { current: currentStep, total: 3 })">
        <li v-for="step in 3" :key="step" :class="{ 'is-active': currentStep === step, 'is-complete': currentStep > step }">
          <b class="pencil-numeric">{{ String(step).padStart(2, '0') }}</b>
          <span>{{ step === 1 ? t('auth.registeredEmail') : step === 2 ? t('auth.securityVerification') : t('auth.newPassword') }}</span>
          <i aria-hidden="true" />
        </li>
      </ol>

      <label v-if="currentStep === 1" class="auth-pencil-field">
        <span>{{ t('auth.registeredEmail') }}</span>
        <span class="auth-pencil-field__shell"><input v-model="email" autocomplete="email" inputmode="email" :placeholder="t('auth.emailPlaceholder')" /></span>
      </label>

      <div v-if="currentStep === 1" class="auth-pencil-field auth-pencil-field--status">
        <span>{{ t('auth.securityVerification') }}</span>
        <strong>{{ t('auth.resetDescription') }}</strong>
      </div>

      <template v-else-if="currentStep === 2">
        <label class="auth-pencil-field">
          <span>{{ t('auth.registeredEmail') }}</span>
          <span class="auth-pencil-field__shell"><MailCheck :size="17" /><input :value="email" readonly /></span>
        </label>
        <label class="auth-pencil-field">
          <span>{{ t('auth.emailCode') }}</span>
          <span class="auth-pencil-field__shell">
            <MailCheck :size="17" />
            <input v-model="code" autocomplete="one-time-code" inputmode="numeric" maxlength="8" :placeholder="t('auth.codePlaceholder')" />
            <button type="button" :disabled="sending || remainingSeconds > 0" @click="sendCode">{{ sending ? t('auth.sending') : sendLabel }}</button>
          </span>
        </label>
      </template>

      <template v-else>
        <label class="auth-pencil-field">
          <span>{{ t('auth.newPassword') }}</span>
          <span class="auth-pencil-field__shell"><KeyRound :size="17" /><input v-model="password" type="password" autocomplete="new-password" :placeholder="t('auth.newPasswordPlaceholder')" /></span>
        </label>
        <label class="auth-pencil-field">
          <span>{{ t('auth.confirmNewPassword') }}</span>
          <span class="auth-pencil-field__shell"><KeyRound :size="17" /><input v-model="confirmation" type="password" autocomplete="new-password" :placeholder="t('auth.reenterNewPassword')" /></span>
        </label>
      </template>

      <p v-if="error" class="pencil-message pencil-message--error auth-feedback" role="alert">{{ error }}</p>
      <p v-if="success" class="pencil-message pencil-message--success auth-feedback" role="status">{{ success }}</p>
      <button class="pencil-primary auth-submit" type="submit" :disabled="sending || submitting">
        {{ submitting ? t('common.submitting') : currentStep === 3 ? t('auth.updatePassword') : t('auth.next') }}
      </button>
      <p class="auth-note">{{ t('auth.securityNote') }}</p>
    </form>
  </main>
</template>

<style scoped>
.page.pencil-page.auth-page { background: var(--page); background-image: none; min-height: 100dvh; }
.auth-form { display: flex; flex-direction: column; gap: 14px; padding-bottom: calc(20px + env(safe-area-inset-bottom)); padding-top: 10px; }
.auth-form h1 { color: var(--ink); font-size: 22px; font-weight: 400; line-height: 31px; margin: 0; }
.auth-subtitle { color: var(--muted-strong); font-size: 12px; line-height: 17px; margin: 0; }
.reset-steps { display: flex; gap: 8px; list-style: none; margin: 0; padding: 0; }
.reset-steps li { align-items: center; color: var(--muted); display: flex; flex: 0 0 auto; flex-direction: column; gap: 5px; min-width: 68px; }
.reset-steps b { font-size: 13px; font-weight: 700; line-height: 18px; }
.reset-steps span { font-size: 11px; font-weight: 500; line-height: 15px; white-space: nowrap; }
.reset-steps i { background: var(--line); border-radius: 1px; height: 2px; width: 30px; }
.reset-steps li.is-active,
.reset-steps li.is-complete { color: var(--positive); }
.reset-steps li.is-active span { color: var(--ink); font-weight: 400; }
.reset-steps li.is-active i,
.reset-steps li.is-complete i { background: var(--accent); }
.auth-pencil-field { border-radius: 8px; display: grid; gap: 5px; min-width: 0; padding: 4px 0; }
.auth-pencil-field:focus-within { box-shadow: 0 0 0 2px var(--focus-ring); outline: 1px solid var(--positive); outline-offset: -1px; }
.auth-pencil-field > span:first-child { color: var(--muted); font-size: 11px; font-weight: 500; line-height: 15px; }
.auth-pencil-field__shell { align-items: center; color: var(--muted); display: flex; gap: 8px; min-height: 20px; min-width: 0; padding: 0; }
.auth-pencil-field__shell input { background: transparent; border: 0; color: var(--ink); font-size: 14px; font-weight: 600; line-height: 20px; min-height: 20px; min-width: 0; outline: 0; padding: 0; width: 100%; }
.auth-pencil-field__shell > button { background: transparent; color: var(--positive); flex: 0 0 auto; font-size: 11px; font-weight: 600; min-height: 44px; padding: 0; }
.auth-pencil-field--status strong { color: var(--ink); font-size: 14px; font-weight: 600; line-height: 20px; }
.auth-feedback { margin: 0; }
.auth-submit { font-size: 15px; height: 48px; min-height: 48px; width: 100%; }
.auth-note { color: var(--muted); font-size: 11px; line-height: 16px; margin: 0; }
.auth-page button:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
@media (max-width: 340px) {
  .auth-form { padding-inline: 16px; }
  .reset-steps { gap: 4px; }
  .reset-steps li { min-width: 62px; }
}
</style>
