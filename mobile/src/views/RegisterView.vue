<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ArrowLeft, AtSign, Check, ChevronDown, Eye, EyeOff, Globe2, KeyRound, Languages, MailCheck } from 'lucide-vue-next'
import { apiErrorMessage } from '@/api/client'
import { fetchCountries, fetchRegisterConfig, registerWithEmail, sendRegistrationCode, type CountryOption } from '@/api/auth'
import { goBackOr } from '@/core/navigation'
import { useSessionStore } from '@/stores/session'

const router = useRouter()
const session = useSessionStore()
const { locale, t } = useI18n()
const step = ref<1 | 2>(1)
const countries = ref<CountryOption[]>([])
const email = ref('')
const countryCode = ref('')
const password = ref('')
const confirmation = ref('')
const code = ref('')
const inviteCode = ref('')
const error = ref('')
const countriesNotice = ref('')
const sent = ref(false)
const sending = ref(false)
const submitting = ref(false)
const remainingSeconds = ref(0)
const acceptedTerms = ref(false)
const showPassword = ref(false)
const emailCodeRequired = ref(true)
const inviteCodeRequired = ref(false)
const emailInput = ref<HTMLInputElement | null>(null)
let timer: number | undefined

const sendLabel = computed(() => remainingSeconds.value ? `${remainingSeconds.value}s` : sent.value ? t('auth.resend') : t('auth.sendCode'))
const passwordLengthValid = computed(() => password.value.length >= 8)
const passwordsMatch = computed(() => Boolean(confirmation.value) && password.value === confirmation.value)
const registrationDescription = computed(() => t(emailCodeRequired.value ? 'auth.registrationDetailsDescription' : 'auth.registrationDetailsDescriptionNoCode'))
const regionNames = computed(() => {
  void locale.value
  try {
    return new Intl.DisplayNames([locale.value], { type: 'region' })
  } catch {
    return null
  }
})

const fallbackCountries: CountryOption[] = [
  { code: 'CN', name: 'China' },
  { code: 'HK', name: 'Hong Kong' },
  { code: 'US', name: 'United States' },
  { code: 'SG', name: 'Singapore' },
  { code: 'JP', name: 'Japan' },
  { code: 'KR', name: 'South Korea' },
  { code: 'GB', name: 'United Kingdom' },
  { code: 'AU', name: 'Australia' },
  { code: 'CA', name: 'Canada' },
  { code: 'DE', name: 'Germany' },
  { code: 'FR', name: 'France' },
  { code: 'AE', name: 'United Arab Emirates' },
]

function countryLabel(country: CountryOption): string {
  return regionNames.value?.of(country.code) || country.name || country.code
}

function handleBack(): void {
  if (step.value === 2) {
    step.value = 1
    error.value = ''
    return
  }
  void goBackOr(router, '/login')
}

function continueRegistration(): void {
  if (!countryCode.value) {
    error.value = t('auth.countryRequired')
    return
  }
  if (!acceptedTerms.value) {
    error.value = t('auth.termsRequired')
    return
  }
  error.value = ''
  step.value = 2
  void nextTick(() => emailInput.value?.focus())
}

function startCountdown() {
  remainingSeconds.value = 60
  if (timer) window.clearInterval(timer)
  timer = window.setInterval(() => {
    remainingSeconds.value = Math.max(0, remainingSeconds.value - 1)
    if (!remainingSeconds.value && timer) window.clearInterval(timer)
  }, 1_000)
}

async function sendCode() {
  if (!emailCodeRequired.value) return
  error.value = ''
  if (!email.value.trim() || !email.value.includes('@')) {
    error.value = t('auth.validEmailRequired')
    return
  }
  sending.value = true
  try {
    await sendRegistrationCode(email.value)
    sent.value = true
    startCountdown()
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('auth.codeSendFailed'))
  } finally {
    sending.value = false
  }
}

async function submit() {
  error.value = ''
  if (!email.value.trim() || !email.value.includes('@') || !countryCode.value || (emailCodeRequired.value && !code.value.trim()) || !password.value) {
    error.value = t('auth.completeRegistration')
    return
  }
  if (inviteCodeRequired.value && !inviteCode.value.trim()) {
    error.value = t('auth.inviteCodeRequiredMessage')
    return
  }
  if (!passwordLengthValid.value) {
    error.value = t('auth.passwordTooShort')
    return
  }
  if (password.value !== confirmation.value) {
    error.value = t('auth.passwordMismatch')
    return
  }
  submitting.value = true
  try {
    await registerWithEmail({ email: email.value, password: password.value, code: code.value, countryCode: countryCode.value, inviteCode: inviteCode.value })
    session.sync()
    await router.replace('/')
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('auth.registerFailed'))
  } finally {
    submitting.value = false
  }
}

onMounted(async () => {
  const [countriesResult, configResult] = await Promise.allSettled([fetchCountries(), fetchRegisterConfig()])
  if (countriesResult.status === 'fulfilled') {
    countries.value = countriesResult.value
  } else {
    countries.value = fallbackCountries
    countriesNotice.value = t('auth.countriesFallback')
  }
  if (configResult.status === 'fulfilled') {
    emailCodeRequired.value = configResult.value.emailCodeRequired
    inviteCodeRequired.value = configResult.value.inviteCodeRequired
  }
  const systemRegion = navigator.language.split('-')[1]?.toUpperCase()
  countryCode.value = countries.value.find((country) => country.code === systemRegion)?.code || countries.value[0]?.code || ''
})

onUnmounted(() => { if (timer) window.clearInterval(timer) })
</script>

<template>
  <main class="register-page">
    <header class="auth-topbar">
      <button class="icon-button" type="button" :aria-label="t('common.back')" @click="handleBack"><ArrowLeft :size="24" /></button>
      <button class="icon-button" type="button" :aria-label="t('language.title')" @click="router.push({ name: 'language' })"><Languages :size="21" /></button>
    </header>

    <form class="register-form" @submit.prevent="submit">
      <div class="register-form__intro">
        <span>{{ t('auth.stepProgress', { current: step, total: 2 }) }}</span>
        <h1>{{ t(step === 1 ? 'auth.residenceTitle' : 'auth.registrationDetailsTitle') }}</h1>
        <p>{{ step === 1 ? t('auth.residenceDescription') : registrationDescription }}</p>
      </div>

      <template v-if="step === 1">
        <label class="auth-label"><span>{{ t('auth.country') }}</span><div class="field-shell field-shell--select"><Globe2 :size="19" /><select v-model="countryCode" autocomplete="country"><option value="" disabled>{{ t('auth.selectCountry') }}</option><option v-for="country in countries" :key="country.code" :value="country.code">{{ countryLabel(country) }} ({{ country.code }})</option></select><ChevronDown :size="18" /></div></label>
        <p v-if="countriesNotice" class="countries-notice">{{ countriesNotice }}</p>
        <label class="terms-row"><input v-model="acceptedTerms" type="checkbox" /><span class="terms-check"><Check :size="15" /></span><span>{{ t('auth.termsAgreement') }}</span></label>
      </template>

      <template v-else>
        <label class="auth-label"><span>{{ t('auth.email') }}</span><div class="field-shell"><AtSign :size="18" /><input ref="emailInput" v-model="email" autocomplete="email" inputmode="email" placeholder="name@example.com" /></div></label>
        <label v-if="emailCodeRequired" class="auth-label"><span>{{ t('auth.emailCode') }}</span><div class="verification-field"><div class="field-shell"><MailCheck :size="18" /><input v-model="code" autocomplete="one-time-code" inputmode="numeric" maxlength="8" :placeholder="t('auth.codePlaceholder')" /></div><button class="button button--secondary" type="button" :disabled="sending || remainingSeconds > 0" @click="sendCode">{{ sending ? t('auth.sending') : sendLabel }}</button></div></label>
        <label class="auth-label"><span>{{ t('auth.loginPassword') }}</span><div class="field-shell"><KeyRound :size="18" /><input v-model="password" :type="showPassword ? 'text' : 'password'" autocomplete="new-password" :placeholder="t('auth.passwordMinimum')" /><button class="password-toggle" type="button" :aria-label="t(showPassword ? 'auth.hidePassword' : 'auth.showPassword')" @click="showPassword = !showPassword"><EyeOff v-if="showPassword" :size="19" /><Eye v-else :size="19" /></button></div></label>
        <label class="auth-label"><span>{{ t('auth.confirmPassword') }}</span><div class="field-shell"><KeyRound :size="18" /><input v-model="confirmation" :type="showPassword ? 'text' : 'password'" autocomplete="new-password" :placeholder="t('auth.reenterPassword')" /></div></label>
        <div class="password-checks" aria-live="polite"><span :class="{ valid: passwordLengthValid }"><Check :size="14" />{{ t('auth.passwordLengthRule') }}</span><span :class="{ valid: passwordsMatch }"><Check :size="14" />{{ t('auth.passwordMatchRule') }}</span></div>
        <label class="auth-label"><span>{{ t(inviteCodeRequired ? 'auth.inviteCodeRequired' : 'auth.inviteCodeOptional') }}</span><div class="field-shell"><input v-model="inviteCode" :placeholder="t('auth.inviteCodePlaceholder')" /></div></label>
      </template>

      <p v-if="error" class="error-message" aria-live="polite">{{ error }}</p>
      <div class="register-form__actions">
        <button v-if="step === 1" class="button button--primary button--full" type="button" @click="continueRegistration">{{ t('auth.next') }}</button>
        <button v-else class="button button--primary button--full" type="submit" :disabled="submitting">{{ submitting ? t('auth.registering') : t('auth.createAccount') }}</button>
        <p>{{ t('auth.alreadyHaveAccount') }} <button type="button" @click="router.replace({ name: 'login' })">{{ t('auth.goLogin') }}</button></p>
      </div>
    </form>
  </main>
</template>

<style scoped>
.register-page { background: var(--surface); display: grid; grid-template-rows: auto minmax(0, 1fr); min-height: 100dvh; padding-top: env(safe-area-inset-top); }
.auth-topbar { align-items: center; display: flex; justify-content: space-between; min-height: 68px; padding: 8px 16px; }
.register-form { display: flex; flex-direction: column; gap: 20px; margin: 0 auto; max-width: 430px; padding: 24px 24px calc(24px + env(safe-area-inset-bottom)); width: 100%; }
.register-form__intro { margin-bottom: 12px; }
.register-form__intro > span { color: var(--positive); display: block; font-size: 12px; font-weight: 750; margin-bottom: 15px; }
.register-form h1 { font-size: 36px; line-height: 1.16; margin: 0; overflow-wrap: anywhere; }
.register-form__intro p { color: var(--muted-strong); font-size: 15px; line-height: 1.65; margin: 13px 0 0; }
.auth-label { display: grid; gap: 9px; }
.auth-label > span { font-size: 14px; font-weight: 720; }
.field-shell { align-items: center; background: var(--soft); border: 1px solid transparent; border-radius: var(--radius); color: var(--muted-strong); display: flex; gap: 11px; min-height: 58px; padding: 0 15px; transition: background-color 160ms ease, border-color 160ms ease, box-shadow 160ms ease; }
.field-shell:focus-within { background: white; border-color: var(--accent); box-shadow: 0 0 0 3px rgb(22 124 103 / 9%); }
.field-shell input,.field-shell select { background: transparent; border: 0; color: var(--ink); font-size: 15px; min-width: 0; outline: 0; width: 100%; }
.field-shell input::placeholder { color: #a3a8ad; }
.field-shell select { appearance: none; }
.field-shell--select > svg:last-child { flex: 0 0 auto; pointer-events: none; }
.password-toggle { align-items: center; background: transparent; color: var(--muted-strong); display: inline-flex; flex: 0 0 40px; height: 40px; justify-content: center; margin-right: -8px; padding: 0; }
.verification-field { display: grid; gap: 10px; grid-template-columns: minmax(0, 1fr) 112px; }
.verification-field .button { font-size: 12px; min-height: 58px; padding: 0 8px; }
.terms-row { align-items: flex-start; cursor: pointer; display: grid; gap: 11px; grid-template-columns: 22px minmax(0, 1fr); margin-top: 6px; position: relative; }
.terms-row input { height: 22px; inset: 0 auto auto 0; margin: 0; opacity: 0; position: absolute; width: 22px; }
.terms-check { align-items: center; background: white; border: 1px solid var(--muted); border-radius: 4px; color: transparent; display: inline-flex; height: 22px; justify-content: center; transition: background-color 160ms ease, border-color 160ms ease; width: 22px; }
.terms-row input:checked + .terms-check { background: var(--ink); border-color: var(--ink); color: white; }
.terms-row input:focus-visible + .terms-check { outline: 2px solid color-mix(in srgb, var(--accent) 70%, white); outline-offset: 2px; }
.terms-row > span:last-child { color: var(--muted-strong); font-size: 13px; line-height: 1.65; }
.countries-notice { color: var(--muted); font-size: 11px; line-height: 1.5; margin: -12px 0 0; }
.password-checks { display: flex; flex-wrap: wrap; gap: 8px 14px; margin-top: -8px; }
.password-checks span { align-items: center; color: var(--muted); display: inline-flex; font-size: 11px; gap: 4px; }
.password-checks span.valid { color: var(--positive); }
.register-form__actions { margin-top: auto; padding-top: 28px; }
.register-form__actions > .button { min-height: 54px; }
.register-form__actions p { color: var(--muted-strong); font-size: 14px; margin: 18px 0 0; text-align: center; }
.register-form__actions p button { background: transparent; color: var(--ink); font-weight: 750; padding: 4px; text-decoration: underline; text-underline-offset: 3px; }
@media (max-width: 360px) {
  .register-form { padding-left: 18px; padding-right: 18px; }
  .register-form h1 { font-size: 32px; }
  .verification-field { grid-template-columns: minmax(0, 1fr) 96px; }
}
@media (max-height: 720px) {
  .register-form { padding-top: 10px; }
  .register-form__intro { margin-bottom: 2px; }
  .register-form__actions { padding-top: 16px; }
}
</style>
