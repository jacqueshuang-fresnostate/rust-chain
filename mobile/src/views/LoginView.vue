<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { Eye, EyeOff, ShieldCheck } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { fetchLoginConfig, loginWithPassword } from '@/api/auth'
import { apiErrorMessage } from '@/api/client'
import { useSessionStore } from '@/stores/session'
import { replaceAuthStep, sanitizeInternalRedirect } from '@/core/navigation'
import logo from '@/assets/logo.png'

type LoginMode = 'email' | 'username'

type TurnstileWindow = {
  turnstile?: {
    render: (element: string | HTMLElement, options: Record<string, unknown>) => string | number
    reset: (widgetId: string | number) => void
    remove: (widgetId: string | number) => void
  }
}

declare global {
  interface Window extends TurnstileWindow {}
}

const route = useRoute()
const router = useRouter()
const session = useSessionStore()
const { t } = useI18n()
const loginMode = ref<LoginMode>('email')
const account = ref('')
const password = ref('')
const error = ref('')
const submitting = ref(false)
const showPassword = ref(false)
const usernameLoginEnabled = ref(false)
const accountInput = ref<HTMLInputElement | null>(null)
const turnstileSiteKey = ref(String(import.meta.env.VITE_CF_TURNSTILE_SITE_KEY ?? '').trim())
const turnstileRequired = ref<boolean | null>(null)
const turnstileEnabled = computed(() => {
  if (!turnstileSiteKey.value) {
    return false
  }

  return turnstileRequired.value ?? true
})
const cfTurnstileToken = ref('')
const turnstileContainer = ref<HTMLDivElement | null>(null)
const turnstileWidgetId = ref<string | number | null>(null)
const safeRedirect = computed(() => sanitizeInternalRedirect(route.query.redirect))
let turnstileScriptPromise: Promise<void> | null = null

const turnstileEnabledText = computed(() => t('auth.turnstileRequired'))
const cfTurnstileTokenRequiredMessage = 'cf_turnstile_token is required'

function isTurnstileTokenMissingError(error: unknown): boolean {
  const responseData = (error as { response?: { data?: { code?: string; message?: string } } }).response?.data
  return responseData?.code === 'CF_TURNSTILE_TOKEN_MISSING' || responseData?.message === cfTurnstileTokenRequiredMessage
}

function openAuthRoute(name: 'register' | 'forgot-password'): void {
  void replaceAuthStep(router, { name, query: { redirect: safeRedirect.value } })
}

function selectMode(mode: LoginMode): void {
  loginMode.value = mode
  account.value = ''
  error.value = ''
  void nextTick(() => accountInput.value?.focus())
}

async function submit(): Promise<void> {
  error.value = ''
  const identifier = account.value.trim()
  if (!identifier || (loginMode.value === 'email' && !identifier.includes('@'))) {
    error.value = t(loginMode.value === 'email' ? 'auth.validEmailRequired' : 'auth.usernameRequired')
    return
  }
  if (!password.value) {
    error.value = t('auth.invalidCredentialsInput')
    return
  }
  if (turnstileEnabled.value && !cfTurnstileToken.value) {
    error.value = turnstileEnabledText.value
    return
  }
  submitting.value = true
  try {
    const result = await loginWithPassword(account.value, password.value, cfTurnstileToken.value || undefined)
    if (result.type === 'two-factor') {
      await replaceAuthStep(router, { name: 'login-two-factor', query: { challenge: result.challengeId, redirect: safeRedirect.value } })
      return
    }
    if (result.type === 'two-factor-setup') {
      await replaceAuthStep(router, { name: 'login-two-factor', query: { setup: result.setupChallengeId, redirect: safeRedirect.value } })
      return
    }
    session.sync()
    await replaceAuthStep(router, safeRedirect.value)
  } catch (reason) {
    if (isTurnstileTokenMissingError(reason)) {
      error.value = turnstileEnabledText.value
      try {
        await refreshLoginConfig()
      } catch {
        error.value = t('auth.turnstileLoadFailed')
      }
      return
    }
    error.value = apiErrorMessage(reason, t('auth.loginFailed'))
  } finally {
    submitting.value = false
    if (turnstileEnabled.value) {
      resetCfTurnstileWidget()
      void initializeTurnstile()
    }
  }
}

function getTurnstileWidgetWindow(): TurnstileWindow['turnstile'] | undefined {
  return typeof window === 'undefined' ? undefined : window.turnstile
}

function resetCfTurnstileWidget(): void {
  const turnstile = getTurnstileWidgetWindow()
  if (!turnstileWidgetId.value || !turnstile) return

  try {
    turnstile.reset(turnstileWidgetId.value)
  } catch {
    // fallback: if reset is unavailable under some browsers, attempt hard remove + re-render later.
    try {
      turnstile.remove(turnstileWidgetId.value)
    } catch {
      // ignore
    }
  }
  turnstileWidgetId.value = null
  cfTurnstileToken.value = ''
}

function removeTurnstileWidget(): void {
  const turnstile = getTurnstileWidgetWindow()
  if (!turnstileWidgetId.value || !turnstile) return
  try {
    turnstile.remove(turnstileWidgetId.value)
  } catch {
    // ignore
  }
  turnstileWidgetId.value = null
  cfTurnstileToken.value = ''
}

async function loadTurnstileScript(): Promise<void> {
  if (turnstileScriptPromise) {
    return turnstileScriptPromise
  }

  if (typeof window === 'undefined' || getTurnstileWidgetWindow()) {
    return Promise.resolve()
  }

  turnstileScriptPromise = new Promise((resolve, reject) => {
    const script = document.createElement('script')
    script.src = 'https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit'
    script.async = true
    script.defer = true
    script.onload = () => {
      resolve()
    }
    script.onerror = () => {
      turnstileScriptPromise = null
      reject(new Error('Failed to load Cloudflare Turnstile script'))
    }
    document.head.appendChild(script)
  })

  await turnstileScriptPromise
}

async function initializeTurnstile(): Promise<void> {
  if (!turnstileEnabled.value) {
    return
  }

  await nextTick()
  if (!turnstileContainer.value) {
    return
  }

  try {
    await loadTurnstileScript()
    const turnstile = getTurnstileWidgetWindow()
    if (!turnstile || !turnstileContainer.value) {
      return
    }
    removeTurnstileWidget()
    turnstileWidgetId.value = turnstile.render(turnstileContainer.value, {
      sitekey: turnstileSiteKey.value,
      callback: (token: string) => {
        cfTurnstileToken.value = token || ''
      },
      'expired-callback': () => {
        cfTurnstileToken.value = ''
      },
      'error-callback': () => {
        cfTurnstileToken.value = ''
      },
      'timeout-callback': () => {
        cfTurnstileToken.value = ''
      },
    })
  } catch {
    error.value = t('auth.turnstileLoadFailed')
  }
}

async function refreshLoginConfig(): Promise<void> {
  const loginConfig = await fetchLoginConfig()
  usernameLoginEnabled.value = loginConfig.usernameLoginEnabled
  if (!turnstileSiteKey.value && loginConfig.cfTurnstileSiteKey) {
    turnstileSiteKey.value = loginConfig.cfTurnstileSiteKey
  }
  turnstileRequired.value = loginConfig.cfTurnstileEnabled

  if (loginConfig.cfTurnstileEnabled && (turnstileSiteKey.value || loginConfig.cfTurnstileSiteKey)) {
    await initializeTurnstile()
    return
  }

  resetCfTurnstileWidget()
}

onMounted(async () => {
  try {
    await refreshLoginConfig()
  } catch {
    usernameLoginEnabled.value = false
    if (loginMode.value === 'username') selectMode('email')
  }

  if (turnstileEnabled.value) {
    await initializeTurnstile()
  }
})

onBeforeUnmount(() => {
  removeTurnstileWidget()
})
</script>

<template>
  <main class="auth-pencil-page login-pencil" data-pencil-source="u99Fpg WNbsc">
    <section class="auth-pencil-canvas">
      <header class="auth-brand-row">
        <img :src="logo" alt="Hippo" />
      </header>

      <div class="auth-pencil-title">
        <h1>{{ t('auth.welcomeBack') }}</h1>
        <p>{{ t('auth.pencilLoginDescription') }}</p>
      </div>

      <form class="login-pencil__form" :aria-busy="submitting" @submit.prevent="submit">
        <div v-if="usernameLoginEnabled" class="auth-method-tabs pencil-segmented" role="tablist" :aria-label="t('auth.loginMethod')">
          <button type="button" role="tab" :aria-selected="loginMode === 'email'" @click="selectMode('email')">{{ t('auth.email') }}</button>
          <button type="button" role="tab" :aria-selected="loginMode === 'username'" @click="selectMode('username')">{{ t('auth.username') }}</button>
        </div>
        <span v-else class="auth-single-method">{{ t('auth.email') }}</span>

        <label class="pencil-field__shell auth-pencil-field">
          <span>{{ t(loginMode === 'email' ? 'auth.email' : 'auth.username') }}</span>
          <input
            ref="accountInput"
            v-model="account"
            :aria-invalid="Boolean(error)"
            :autocomplete="loginMode === 'email' ? 'email' : 'username'"
            :inputmode="loginMode === 'email' ? 'email' : 'text'"
            :placeholder="t(loginMode === 'email' ? 'auth.emailPlaceholder' : 'auth.usernamePlaceholder')"
          />
        </label>

        <label class="pencil-field__shell auth-pencil-field auth-pencil-field--action">
          <span>{{ t('auth.password') }}</span>
          <input
            v-model="password"
            :aria-invalid="Boolean(error)"
            :type="showPassword ? 'text' : 'password'"
            autocomplete="current-password"
            :placeholder="t('auth.passwordPlaceholder')"
          />
          <button type="button" :aria-label="t(showPassword ? 'auth.hidePassword' : 'auth.showPassword')" @click="showPassword = !showPassword">
            <EyeOff v-if="showPassword" :size="18" />
            <Eye v-else :size="18" />
          </button>
        </label>

        <div class="auth-form-meta">
          <span class="auth-remember"><i aria-hidden="true" />{{ t('auth.keepSignedIn') }}</span>
          <button type="button" @click="openAuthRoute('forgot-password')">{{ t('auth.forgotPassword') }}</button>
        </div>

        <div v-if="turnstileEnabled" class="auth-cf-turnstile-wrap">
          <div ref="turnstileContainer" class="cf-turnstile-widget" />
        </div>

        <div class="login-submit-wrap">
          <button class="pencil-primary pencil-primary--full auth-pencil-submit" type="submit" :disabled="submitting">
            {{ submitting ? t('auth.loggingIn') : t('auth.login') }}
          </button>
        </div>
        <p v-if="error" class="auth-pencil-feedback" role="alert">{{ error }}</p>
      </form>

      <p class="auth-switch">{{ t('auth.noAccount') }} <button type="button" @click="openAuthRoute('register')">{{ t('auth.registerNow') }}</button></p>
      <div class="auth-security-note">
        <ShieldCheck :size="17" />
        <span>{{ t('auth.newDeviceTwoFactor') }}</span>
      </div>
    </section>
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
  max-width: 330px;
}

.login-pencil__form {
  display: block;
}

.auth-method-tabs,
.auth-single-method {
  box-sizing: border-box;
  height: 26px;
  min-height: 26px;
}

.auth-method-tabs {
  gap: 24px;
  padding-top: 0;
}

.auth-method-tabs button {
  font-size: 13px;
  height: 26px;
  min-height: 26px;
  padding-bottom: 7px;
}

.auth-method-tabs button[aria-selected='true'] {
  color: var(--ink);
  font-weight: 500;
}

.auth-single-method {
  color: var(--ink);
  display: block;
  font-size: 13px;
  font-weight: 500;
  line-height: 19px;
  padding: 0 0 7px;
  position: relative;
  width: max-content;
}

.auth-single-method::after {
  background: var(--accent);
  bottom: 0;
  content: '';
  height: 2px;
  left: 0;
  position: absolute;
  width: 22px;
}

.auth-pencil-field {
  align-content: center;
  box-sizing: border-box;
  display: grid;
  gap: 1px 8px;
  grid-template-columns: minmax(0, 1fr);
  grid-template-rows: 11px 22px;
  height: 48px;
  margin-top: 12px;
  min-height: 48px;
  padding: 5px 14px;
}

.auth-pencil-field--action {
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

.auth-pencil-field > input {
  font-family: var(--font-geist-mono), var(--data-font);
  font-size: 13px;
  grid-column: 1;
  grid-row: 2;
  line-height: 20px;
  min-height: 22px;
}

.auth-pencil-field > button {
  grid-column: 2;
  grid-row: 1 / 3;
  height: 44px;
  margin: -5px -12px -5px 0;
  min-height: 44px;
  width: 44px;
}

.auth-form-meta {
  align-items: center;
  display: flex;
  height: 16px;
  justify-content: space-between;
  margin-top: 12px;
  min-height: 16px;
}

.auth-remember {
  align-items: center;
  color: var(--muted);
  display: inline-flex;
  font-size: 11px;
  gap: 7px;
}

.auth-remember i {
  border: 1px solid var(--line);
  border-radius: 3px;
  box-sizing: border-box;
  height: 14px;
  width: 14px;
}

.auth-form-meta button {
  background: transparent;
  color: var(--positive);
  font-size: 11px;
  font-weight: 600;
  height: 16px;
  line-height: 16px;
  min-height: 16px;
  padding: 0;
  position: relative;
}

.auth-form-meta button::before {
  content: '';
  inset: -14px -10px;
  position: absolute;
}

.login-submit-wrap {
  box-sizing: border-box;
  height: 56px;
  margin-top: 12px;
  min-height: 56px;
  padding-top: 8px;
}

.auth-pencil-submit {
  height: 48px;
  min-height: 48px;
  width: 100%;
}

.auth-pencil-feedback {
  background: var(--negative-soft);
  border-left: 3px solid var(--negative);
  color: var(--negative);
  font-size: 11px;
  line-height: 1.45;
  margin: 8px 0 0;
  padding: 9px 10px;
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
  inset: -13px -10px;
  position: absolute;
}

.auth-cf-turnstile-wrap {
  align-items: stretch;
  display: flex;
  justify-content: flex-start;
  margin-top: 12px;
  min-height: 82px;
}

.cf-turnstile-widget {
  transform: translateZ(0);
  width: 100%;
}

.auth-security-note {
  align-items: flex-end;
  color: var(--muted);
  display: flex;
  font-size: 10px;
  gap: 8px;
  height: 24px;
  justify-content: center;
  min-height: 24px;
  text-align: center;
}

.auth-security-note svg {
  color: var(--muted);
  flex: 0 0 auto;
}

@media (max-width: 340px) {
  .auth-pencil-canvas { padding-inline: 16px; }
}
</style>
