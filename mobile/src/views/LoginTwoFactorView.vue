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
  <main class="page page--plain login-two-factor-page"><PageHeader :title="t('auth.securityVerification')" /><form v-if="challengeId" class="page-content login-two-factor-form" @submit.prevent="submit"><div class="two-factor-intro"><span><ShieldCheck :size="23" /></span><div><h1>{{ t('auth.authenticatorTitle') }}</h1><p>{{ t('auth.authenticatorDescription') }}</p></div></div><label><span>{{ t('auth.verificationCode') }}</span><div class="field-shell"><KeyRound :size="18" /><input v-model="code" inputmode="numeric" autocomplete="one-time-code" maxlength="8" :placeholder="t('auth.codePlaceholder')" /></div></label><p v-if="error" class="error-message">{{ error }}</p><button class="button button--primary button--full" type="submit" :disabled="submitting">{{ submitting ? t('auth.verifying') : t('auth.confirmLogin') }}</button><section class="reset-section"><strong>{{ t('auth.authenticatorUnavailable') }}</strong><p>{{ t('auth.resetDescription2') }}</p><div><button class="button button--secondary" type="button" :disabled="sending || remainingSeconds > 0" @click="sendResetCode">{{ sending ? t('auth.sendingEllipsis') : sendLabel }}</button><input v-model="resetCode" class="input" inputmode="numeric" :placeholder="t('auth.emailResetCode')" /></div><button class="reset-link" type="button" :disabled="resetting" @click="resetTwoFactor">{{ resetting ? t('auth.resetting') : t('auth.resetAndLogin') }}</button></section></form><div v-else-if="setupChallengeId" class="page-content setup-required"><ShieldCheck :size="28" /><h1>{{ t('auth.setupRequired') }}</h1><p>{{ t('auth.setupRequiredDescription') }}</p><button class="button button--secondary" type="button" @click="router.replace({ name: 'login' })">{{ t('auth.returnLogin') }}</button></div><div v-else class="page-content setup-required"><p>{{ t('auth.challengeExpired') }}</p><button class="button button--secondary" type="button" @click="router.replace({ name: 'login' })">{{ t('auth.returnLogin') }}</button></div></main>
</template>

<style scoped>
.login-two-factor-page .page-content { padding-bottom: 42px; padding-top: 18px; }.login-two-factor-form { display: grid; gap: 17px; }.two-factor-intro { align-items: center; display: flex; gap: 12px; padding: 4px 0 10px; }.two-factor-intro > span { align-items: center; background: var(--positive-soft); border-radius: var(--radius); color: var(--positive); display: inline-flex; height: 46px; justify-content: center; width: 46px; }.two-factor-intro h1 { font-size: 21px; margin: 0; }.two-factor-intro p { color: var(--muted); font-size: 13px; line-height: 1.45; margin: 4px 0 0; }.login-two-factor-form label { display: grid; gap: 8px; }.login-two-factor-form label > span { color: var(--muted); font-size: 13px; }.field-shell { align-items: center; background: var(--soft); border: 1px solid transparent; border-radius: var(--radius); color: var(--muted); display: flex; gap: 10px; min-height: 50px; padding: 0 14px; }.field-shell:focus-within { background: white; border-color: var(--accent); box-shadow: 0 0 0 3px rgb(22 124 103 / 9%); }.field-shell input { background: transparent; border: 0; color: var(--ink); font-size: 16px; min-width: 0; outline: 0; width: 100%; }.reset-section { border-top: 1px solid var(--line); display: grid; gap: 10px; margin-top: 7px; padding-top: 18px; }.reset-section strong { font-size: 15px; }.reset-section p { color: var(--muted); font-size: 12px; line-height: 1.5; margin: 0; }.reset-section > div { display: grid; gap: 9px; grid-template-columns: 120px 1fr; }.reset-section .button { font-size: 12px; min-height: 44px; padding: 0 8px; }.reset-link { background: transparent; color: var(--accent); font-size: 13px; font-weight: 700; padding: 6px 0; text-align: left; }.setup-required { align-items: center; display: flex; flex-direction: column; gap: 13px; padding-top: 54px !important; text-align: center; }.setup-required svg { color: var(--accent); }.setup-required h1 { font-size: 22px; margin: 3px 0 0; }.setup-required p { color: var(--muted); font-size: 14px; line-height: 1.6; margin: 0; max-width: 320px; }
</style>
