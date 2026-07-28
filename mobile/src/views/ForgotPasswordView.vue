<script setup lang="ts">
import { computed, onUnmounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { KeyRound, MailCheck } from 'lucide-vue-next'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { resetPasswordWithCode, sendPasswordResetCode } from '@/api/auth'

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
let timer: number | undefined

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
  try { await sendPasswordResetCode(email.value); startCountdown() } catch (reason) { error.value = apiErrorMessage(reason, t('auth.codeSendFailed')) } finally { sending.value = false }
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
    window.setTimeout(() => { void router.replace({ name: 'login' }) }, 900)
  } catch (reason) { error.value = apiErrorMessage(reason, t('auth.resetFailed')) } finally { submitting.value = false }
}

onUnmounted(() => { if (timer) window.clearInterval(timer) })
</script>

<template>
  <main class="page page--plain auth-page">
    <PageHeader
      :back="true"
      :eyebrow="t('auth.resetTitle')"
      :subtitle="t('auth.resetDescription')"
      :title="t('auth.forgotTitle')"
    />
    <form class="page-content auth-form" :aria-busy="submitting" @submit.prevent="submit">
      <div class="auth-form__intro">
        <span><KeyRound :size="20" /></span>
        <div>
          <h1>{{ t('auth.resetTitle') }}</h1>
          <p>{{ t('auth.resetDescription') }}</p>
        </div>
      </div>

      <label>
        <span>{{ t('auth.registeredEmail') }}</span>
        <div class="field-shell">
          <MailCheck :size="18" />
          <input v-model="email" autocomplete="email" inputmode="email" placeholder="name@example.com" />
        </div>
      </label>
      <label>
        <span>{{ t('auth.emailCode') }}</span>
        <div class="verification-field">
          <div class="field-shell">
            <MailCheck :size="18" />
            <input v-model="code" autocomplete="one-time-code" inputmode="numeric" maxlength="8" :placeholder="t('auth.codePlaceholder')" />
          </div>
          <button class="button button--secondary" type="button" :disabled="sending || remainingSeconds > 0" @click="sendCode">
            {{ sending ? t('auth.sending') : sendLabel }}
          </button>
        </div>
      </label>
      <label>
        <span>{{ t('auth.newPassword') }}</span>
        <div class="field-shell">
          <KeyRound :size="18" />
          <input v-model="password" type="password" autocomplete="new-password" :placeholder="t('auth.newPasswordPlaceholder')" />
        </div>
      </label>
      <label>
        <span>{{ t('auth.confirmNewPassword') }}</span>
        <div class="field-shell">
          <KeyRound :size="18" />
          <input v-model="confirmation" type="password" autocomplete="new-password" :placeholder="t('auth.reenterNewPassword')" />
        </div>
      </label>

      <p v-if="error" class="error-message auth-feedback" role="alert">{{ error }}</p>
      <p v-if="success" class="success-message auth-feedback" role="status">{{ success }}</p>
      <button class="button button--primary button--full auth-submit" type="submit" :disabled="submitting">
        {{ submitting ? t('common.submitting') : t('auth.updatePassword') }}
      </button>
    </form>
  </main>
</template>

<style scoped>
.auth-page { background: var(--background); }
.auth-form { display: grid; gap: 18px; margin: 0 auto; max-width: 448px; padding-bottom: calc(44px + env(safe-area-inset-bottom)); padding-top: 20px; width: 100%; }
.auth-form__intro { align-items: flex-start; border-bottom: 1px solid var(--line); display: flex; gap: 13px; margin-bottom: 4px; padding: 2px 0 20px; }
.auth-form__intro > span { align-items: center; background: var(--accent-soft); border: 1px solid var(--line); border-radius: var(--radius); color: var(--accent); display: inline-flex; flex: 0 0 44px; height: 44px; justify-content: center; width: 44px; }
.auth-form__intro > div { min-width: 0; }
.auth-form h1 { font-size: 24px; letter-spacing: 0; line-height: 1.2; margin: 0; }
.auth-form__intro p { color: var(--muted-strong); font-size: 13px; line-height: 1.5; margin: 5px 0 0; }
.auth-form label { display: grid; gap: 8px; }
.auth-form label > span { font-size: 13px; font-weight: 720; }
.field-shell { align-items: center; background: var(--field-surface); border: 1px solid var(--line); border-radius: var(--radius); color: var(--muted); display: flex; gap: 10px; min-height: 52px; padding: 0 13px; transition: background-color var(--motion-fast) var(--motion-ease), border-color var(--motion-fast) var(--motion-ease), box-shadow var(--motion-fast) var(--motion-ease); }
.field-shell:focus-within { background: var(--surface-elevated); border-color: var(--focus); box-shadow: 0 0 0 3px var(--focus-ring); }
.field-shell input { background: transparent; border: 0; color: var(--ink); font-size: 15px; min-height: 44px; min-width: 0; outline: 0; width: 100%; }
.verification-field { display: grid; gap: 10px; grid-template-columns: minmax(0, 1fr) 112px; }
.verification-field .button { font-size: 12px; min-height: 52px; padding: 0 8px; }
.auth-feedback { border: 1px solid currentColor; border-radius: var(--radius); font-size: 13px; line-height: 1.45; margin: 0; padding: 11px 13px; }
.error-message.auth-feedback { background: var(--negative-soft); }
.success-message { background: var(--positive-soft); color: var(--positive); font-weight: 680; }
.auth-submit { min-height: 52px; }
@media (max-width: 340px) {
  .auth-form { padding-left: 16px; padding-right: 16px; }
  .verification-field { gap: 8px; grid-template-columns: minmax(0, 1fr) 96px; }
}
</style>
