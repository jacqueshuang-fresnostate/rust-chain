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
  <main class="page page--plain auth-page"><PageHeader :title="t('auth.forgotTitle')" /><form class="page-content auth-form" @submit.prevent="submit"><div class="auth-form__intro"><span><KeyRound :size="20" /></span><div><h1>{{ t('auth.resetTitle') }}</h1><p>{{ t('auth.resetDescription') }}</p></div></div><label><span>{{ t('auth.registeredEmail') }}</span><div class="field-shell"><MailCheck :size="18" /><input v-model="email" autocomplete="email" inputmode="email" placeholder="name@example.com" /></div></label><label><span>{{ t('auth.emailCode') }}</span><div class="verification-field"><div class="field-shell"><MailCheck :size="18" /><input v-model="code" inputmode="numeric" maxlength="8" :placeholder="t('auth.codePlaceholder')" /></div><button class="button button--secondary" type="button" :disabled="sending || remainingSeconds > 0" @click="sendCode">{{ sending ? t('auth.sending') : sendLabel }}</button></div></label><label><span>{{ t('auth.newPassword') }}</span><div class="field-shell"><KeyRound :size="18" /><input v-model="password" type="password" autocomplete="new-password" :placeholder="t('auth.newPasswordPlaceholder')" /></div></label><label><span>{{ t('auth.confirmNewPassword') }}</span><div class="field-shell"><KeyRound :size="18" /><input v-model="confirmation" type="password" autocomplete="new-password" :placeholder="t('auth.reenterNewPassword')" /></div></label><p v-if="error" class="error-message">{{ error }}</p><p v-if="success" class="success-message">{{ success }}</p><button class="button button--primary button--full" type="submit" :disabled="submitting">{{ submitting ? t('common.submitting') : t('auth.updatePassword') }}</button></form></main>
</template>

<style scoped>
.auth-page { background: var(--background); }.auth-form { display: grid; gap: 17px; padding-bottom: 42px; }.auth-form__intro { align-items: center; display: flex; gap: 13px; padding: 16px 0 12px; }.auth-form__intro > span { align-items: center; background: var(--positive-soft); border-radius: var(--radius); color: var(--positive); display: inline-flex; height: 44px; justify-content: center; width: 44px; }.auth-form h1 { font-size: 22px; margin: 0; }.auth-form p { color: var(--muted); font-size: 13px; margin: 4px 0 0; }.auth-form label { display: grid; gap: 8px; }.auth-form label > span { font-size: 13px; font-weight: 720; }.field-shell { align-items: center; background: var(--surface); border: 1px solid var(--line); border-radius: var(--radius); color: var(--muted); display: flex; gap: 10px; min-height: 50px; padding: 0 14px; }.field-shell:focus-within { border-color: var(--accent); box-shadow: 0 0 0 3px rgb(22 124 103 / 8%); }.field-shell input { background: transparent; border: 0; color: var(--ink); font-size: 15px; min-width: 0; outline: 0; width: 100%; }.verification-field { display: grid; gap: 10px; grid-template-columns: minmax(0, 1fr) 112px; }.verification-field .button { font-size: 12px; min-height: 50px; padding: 0 8px; }.success-message { color: var(--positive) !important; font-weight: 650; margin: 0 !important; }
</style>
