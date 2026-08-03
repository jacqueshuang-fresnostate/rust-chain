<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useRoute, useRouter, type RouteLocationRaw } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { Check, ChevronDown, Eye, EyeOff, MailCheck } from 'lucide-vue-next'
import { apiErrorMessage } from '@/api/client'
import { fetchCountries, fetchRegisterConfig, registerWithEmail, sendRegistrationCode, type CountryOption } from '@/api/auth'
import {
  createLoginRedirectTarget,
  replaceAuthStep,
  sanitizeInternalRedirect,
} from '@/core/navigation'
import { useSessionStore } from '@/stores/session'
import logo from '@/assets/logo.png'

const route = useRoute()
const router = useRouter()
const session = useSessionStore()
const { locale, t } = useI18n()
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

const safeRedirect = computed(() => sanitizeInternalRedirect(route.query.redirect))
const loginTarget = computed<RouteLocationRaw>(() => createLoginRedirectTarget(safeRedirect.value))
const sendLabel = computed(() => remainingSeconds.value ? `${remainingSeconds.value}s` : sent.value ? t('auth.resend') : t('auth.sendCode'))
const passwordLengthValid = computed(() => password.value.length >= 8)
const passwordsMatch = computed(() => Boolean(confirmation.value) && password.value === confirmation.value)
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

function returnToLogin(): void {
  void replaceAuthStep(router, loginTarget.value)
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
  if (!acceptedTerms.value) {
    error.value = t('auth.termsRequired')
    return
  }
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
    await replaceAuthStep(router, safeRedirect.value)
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
  <main class="auth-pencil-page register-pencil" data-pencil-source="MCuqb RGYGj">
    <form class="auth-pencil-canvas" :aria-busy="submitting" @submit.prevent="submit">
      <header class="auth-brand-row">
        <img :src="logo" alt="Hippo" />
      </header>

      <div class="auth-pencil-title">
        <h1>{{ t('auth.pencilRegisterTitle') }}</h1>
        <p>{{ t('auth.pencilRegisterDescription') }}</p>
      </div>

      <div class="register-fields">
        <label class="pencil-field__shell auth-pencil-field auth-pencil-field--action">
          <span>{{ t('auth.country') }}</span>
          <select v-model="countryCode" autocomplete="country">
            <option value="" disabled>{{ t('auth.selectCountry') }}</option>
            <option v-for="country in countries" :key="country.code" :value="country.code">{{ countryLabel(country) }}</option>
          </select>
          <i class="auth-pencil-field__action"><ChevronDown :size="16" /></i>
        </label>

        <label class="pencil-field__shell auth-pencil-field" :class="{ 'auth-pencil-field--action': emailCodeRequired && email.includes('@') }">
          <span>{{ t('auth.email') }}</span>
          <input ref="emailInput" v-model="email" autocomplete="email" inputmode="email" placeholder="name@example.com" />
          <button v-if="emailCodeRequired && email.includes('@')" type="button" :disabled="sending || remainingSeconds > 0" @click="sendCode">
            {{ sending ? t('auth.sending') : sendLabel }}
          </button>
        </label>

        <label v-if="emailCodeRequired && sent" class="pencil-field__shell auth-pencil-field auth-pencil-field--with-icon">
          <span>{{ t('auth.emailCode') }}</span>
          <input v-model="code" autocomplete="one-time-code" inputmode="numeric" maxlength="8" :placeholder="t('auth.codePlaceholder')" />
          <i class="auth-pencil-field__action"><MailCheck :size="16" /></i>
        </label>

        <label class="pencil-field__shell auth-pencil-field auth-pencil-field--action">
          <span>{{ t('auth.loginPassword') }}</span>
          <input
            v-model="password"
            :aria-invalid="Boolean(password) && !passwordLengthValid"
            :type="showPassword ? 'text' : 'password'"
            autocomplete="new-password"
            :placeholder="t('auth.passwordMinimum')"
          />
          <button type="button" :aria-label="t(showPassword ? 'auth.hidePassword' : 'auth.showPassword')" @click="showPassword = !showPassword">
            <EyeOff v-if="showPassword" :size="18" /><Eye v-else :size="18" />
          </button>
        </label>

        <label class="register-confirm-field">
          <span class="pencil-field__shell auth-pencil-field auth-pencil-field--action">
            <span>{{ t('auth.confirmPassword') }}</span>
            <input
              v-model="confirmation"
              :aria-invalid="Boolean(confirmation) && !passwordsMatch"
              :type="showPassword ? 'text' : 'password'"
              autocomplete="new-password"
              :placeholder="t('auth.reenterPassword')"
            />
            <button type="button" :aria-label="t(showPassword ? 'auth.hidePassword' : 'auth.showPassword')" @click="showPassword = !showPassword">
              <EyeOff v-if="showPassword" :size="18" /><Eye v-else :size="18" />
            </button>
          </span>
          <small :class="{ 'is-visible': Boolean(confirmation) && !passwordsMatch }" aria-live="polite">
            {{ confirmation && !passwordsMatch ? t('auth.passwordMismatch') : '' }}
          </small>
        </label>

        <label class="pencil-field__shell auth-pencil-field">
          <span>{{ t(inviteCodeRequired ? 'auth.inviteCodeRequired' : 'auth.inviteCodeOptional') }}</span>
          <input v-model="inviteCode" :placeholder="t('auth.inviteCodePlaceholder')" />
        </label>
      </div>

      <label class="terms-row">
        <input v-model="acceptedTerms" type="checkbox" />
        <span class="terms-check"><Check :size="12" /></span>
        <span>{{ t('auth.termsAgreement') }}</span>
      </label>

      <div class="register-submit-wrap">
        <button class="pencil-primary pencil-primary--full auth-pencil-submit" type="submit" :disabled="submitting">
          {{ submitting ? t('auth.registering') : t('auth.createAccount') }}
        </button>
      </div>

      <p class="auth-switch">{{ t('auth.alreadyHaveAccount') }} <button type="button" @click="returnToLogin">{{ t('auth.goLogin') }}</button></p>
      <p v-if="countriesNotice" class="register-notice" role="status">{{ countriesNotice }}</p>
      <p v-if="error" class="auth-pencil-feedback" role="alert">{{ error }}</p>
    </form>
  </main>
</template>

<style scoped>
.auth-pencil-page {
  background: var(--surface);
  min-height: 100dvh;
  padding-top: env(safe-area-inset-top);
}

.auth-pencil-canvas {
  display: flex;
  flex-direction: column;
  gap: 12px;
  margin: 0 auto;
  max-width: 448px;
  min-height: calc(100dvh - env(safe-area-inset-top));
  padding: 12px 20px calc(24px + env(safe-area-inset-bottom));
  width: 100%;
}

.auth-brand-row {
  box-sizing: border-box;
  height: 62px;
  min-height: 62px;
  padding-top: 28px;
}

.auth-brand-row img {
  display: block;
  height: 34px;
  object-fit: contain;
  object-position: left center;
  width: 136px;
}

.auth-pencil-title {
  box-sizing: border-box;
  height: 88px;
  min-height: 88px;
  padding: 20px 0 8px;
}

.auth-pencil-title h1 {
  font-size: 24px;
  font-weight: 750;
  letter-spacing: 0;
  line-height: 35px;
  margin: 0;
}

.auth-pencil-title p {
  color: var(--muted);
  font-size: 12px;
  line-height: 17px;
  margin: 8px 0 0;
  max-width: 340px;
}

.register-fields {
  display: grid;
  gap: 12px;
}

.auth-pencil-field {
  align-content: center;
  box-sizing: border-box;
  display: grid;
  gap: 1px 8px;
  grid-template-columns: minmax(0, 1fr);
  grid-template-rows: 11px 22px;
  height: 48px;
  min-height: 48px;
  padding: 5px 14px;
}

.auth-pencil-field--action,
.auth-pencil-field--with-icon {
  grid-template-columns: minmax(0, 1fr) 44px;
}

.auth-pencil-field > span {
  color: var(--muted);
  font-size: 9px;
  font-weight: 500;
  grid-column: 1;
  grid-row: 1;
  line-height: 11px;
}

.auth-pencil-field > input,
.auth-pencil-field > select {
  font-size: 13px;
  grid-column: 1;
  grid-row: 2;
  line-height: 20px;
  min-height: 22px;
}

.auth-pencil-field > input {
  font-family: var(--font-geist-mono), var(--data-font);
}

.auth-pencil-field > button,
.auth-pencil-field__action {
  align-items: center;
  align-self: center;
  display: inline-flex;
  grid-column: 2;
  grid-row: 1 / 3;
  height: 44px;
  justify-content: center;
  margin: -5px -12px -5px 0;
  min-height: 44px;
  width: 44px;
}

.auth-pencil-field__action {
  color: var(--muted);
  pointer-events: none;
}

.register-confirm-field {
  display: grid;
  grid-template-rows: 48px 20px;
  height: 68px;
  min-height: 68px;
}

.register-confirm-field > small {
  color: transparent;
  font-size: 10px;
  line-height: 16px;
  padding-top: 2px;
}

.register-confirm-field > small.is-visible {
  color: var(--negative);
}

.terms-row {
  align-items: center;
  cursor: pointer;
  display: grid;
  gap: 8px;
  grid-template-columns: 16px minmax(0, 1fr);
  height: 16px;
  margin-top: 0;
  min-height: 16px;
  position: relative;
}

.terms-row input {
  height: 44px;
  left: -14px;
  margin: 0;
  opacity: 0;
  position: absolute;
  top: -14px;
  width: 44px;
}

.terms-check {
  align-items: center;
  background: var(--surface-2);
  border: 1px solid var(--line-strong);
  border-radius: 3px;
  box-sizing: border-box;
  color: transparent;
  display: inline-flex;
  height: 16px;
  justify-content: center;
  width: 16px;
}

.terms-row input:checked + .terms-check { background: var(--accent); border-color: var(--accent); color: var(--on-accent); }
.terms-row input:focus-visible + .terms-check { box-shadow: 0 0 0 3px var(--focus-ring); outline: 2px solid var(--focus); outline-offset: 2px; }
.terms-row > span:last-child {
  color: var(--muted);
  font-size: 10px;
  line-height: 16px;
}

.register-submit-wrap {
  box-sizing: border-box;
  height: 56px;
  min-height: 56px;
  padding-top: 8px;
}

.auth-pencil-submit {
  height: 48px;
  min-height: 48px;
  width: 100%;
}

.auth-switch {
  align-items: flex-end;
  color: var(--muted);
  display: flex;
  font-size: 13px;
  height: 33px;
  justify-content: center;
  margin: 0;
  min-height: 33px;
}

.auth-switch button {
  background: transparent;
  color: var(--positive);
  font-weight: 600;
  line-height: 19px;
  min-height: 19px;
  padding: 0 5px;
  position: relative;
}

.auth-switch button::before {
  content: '';
  inset: -10px;
  position: absolute;
}

.register-notice,
.auth-pencil-feedback {
  border-left: 3px solid currentColor;
  font-size: 10px;
  line-height: 1.4;
  margin: 0;
  padding: 5px 8px;
}

.register-notice {
  color: var(--muted);
}

.auth-pencil-feedback {
  background: var(--negative-soft);
  color: var(--negative);
}

@media (max-width: 340px) {
  .auth-pencil-canvas { padding-inline: 16px; }
}
</style>
