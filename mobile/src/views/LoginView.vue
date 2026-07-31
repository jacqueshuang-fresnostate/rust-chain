<script setup lang="ts">
import { computed, nextTick, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { ArrowLeft, Eye, EyeOff, Languages, UserRound, X } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { fetchLoginConfig, loginWithPassword } from '@/api/auth'
import { apiErrorMessage } from '@/api/client'
import { useSessionStore } from '@/stores/session'
import { goBackOr, replaceAuthStep, sanitizeInternalRedirect } from '@/core/navigation'
import logo from '@/assets/logo.png'

type LoginMode = 'email' | 'username'

const route = useRoute()
const router = useRouter()
const session = useSessionStore()
const { t } = useI18n()
const step = ref<1 | 2>(1)
const loginMode = ref<LoginMode>('email')
const account = ref('')
const password = ref('')
const error = ref('')
const submitting = ref(false)
const showPassword = ref(false)
const usernameLoginEnabled = ref(false)
const accountInput = ref<HTMLInputElement | null>(null)
const passwordInput = ref<HTMLInputElement | null>(null)
const identityDescription = computed(() => t(usernameLoginEnabled.value ? 'auth.loginIdentityDescription' : 'auth.loginEmailDescription'))
const safeRedirect = computed(() => sanitizeInternalRedirect(route.query.redirect))

function openAuthRoute(name: 'register' | 'forgot-password'): void {
  void replaceAuthStep(router, { name, query: { redirect: safeRedirect.value } })
}

function openLanguage(): void {
  const back = sanitizeInternalRedirect(router.resolve({
    name: 'login',
    query: { redirect: safeRedirect.value },
  }).fullPath)
  void router.push({ name: 'language', query: { back } })
}

function selectMode(mode: LoginMode): void {
  loginMode.value = mode
  account.value = ''
  error.value = ''
  void nextTick(() => accountInput.value?.focus())
}

function continueToPassword(): void {
  const identifier = account.value.trim()
  if (!identifier || (loginMode.value === 'email' && !identifier.includes('@'))) {
    error.value = t(loginMode.value === 'email' ? 'auth.validEmailRequired' : 'auth.usernameRequired')
    return
  }
  error.value = ''
  step.value = 2
  void nextTick(() => passwordInput.value?.focus())
}

function handleBack(): void {
  if (step.value === 2) {
    step.value = 1
    password.value = ''
    error.value = ''
    void nextTick(() => accountInput.value?.focus())
    return
  }
  void goBackOr(router, safeRedirect.value)
}

async function submit(): Promise<void> {
  if (step.value === 1) {
    continueToPassword()
    return
  }
  error.value = ''
  if (!account.value.trim() || !password.value) {
    error.value = t('auth.invalidCredentialsInput')
    return
  }
  submitting.value = true
  try {
    const result = await loginWithPassword(account.value, password.value)
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
    error.value = apiErrorMessage(reason, t('auth.loginFailed'))
  } finally {
    submitting.value = false
  }
}

onMounted(async () => {
  try {
    usernameLoginEnabled.value = (await fetchLoginConfig()).usernameLoginEnabled
  } catch {
    usernameLoginEnabled.value = false
    if (loginMode.value === 'username') selectMode('email')
  }
})
</script>

<template>
  <main class="login-page">
    <header class="auth-topbar">
      <button class="icon-button" type="button" :aria-label="t('common.back')" @click="handleBack"><ArrowLeft v-if="step === 2" :size="24" /><X v-else :size="25" /></button>
      <div class="auth-topbar__copy">
        <span>{{ t('auth.loginMethod') }}</span>
        <strong>{{ step === 1 ? t('auth.login') : t('auth.welcomeBack') }}</strong>
        <small>{{ t('auth.stepProgress', { current: step, total: 2 }) }}</small>
      </div>
      <button class="icon-button" type="button" :aria-label="t('language.title')" @click="openLanguage"><Languages :size="21" /></button>
    </header>

    <section class="login-panel">
      <div class="login-panel__main">
        <img :src="logo" alt="Hippo" class="login-panel__logo" />
        <div class="auth-progress" aria-hidden="true"><i :class="{ active: step >= 1 }"></i><i :class="{ active: step >= 2 }"></i></div>
        <h1>{{ step === 1 ? t('auth.login') : t('auth.welcomeBack') }}</h1>
        <p>{{ step === 1 ? identityDescription : t('auth.passwordStepDescription') }}</p>

        <form :aria-busy="submitting" @submit.prevent="submit">
          <template v-if="step === 1">
            <div v-if="usernameLoginEnabled" class="login-modes" role="tablist" :aria-label="t('auth.loginMethod')">
              <button type="button" role="tab" :aria-selected="loginMode === 'email'" :class="{ active: loginMode === 'email' }" @click="selectMode('email')">{{ t('auth.email') }}</button>
              <button type="button" role="tab" :aria-selected="loginMode === 'username'" :class="{ active: loginMode === 'username' }" @click="selectMode('username')">{{ t('auth.username') }}</button>
            </div>
            <label class="auth-label"><span>{{ t(loginMode === 'email' ? 'auth.email' : 'auth.username') }}</span><div class="auth-field"><UserRound :size="19" /><input ref="accountInput" v-model="account" :aria-invalid="Boolean(error)" :autocomplete="loginMode === 'email' ? 'email' : 'username'" :inputmode="loginMode === 'email' ? 'email' : 'text'" :placeholder="t(loginMode === 'email' ? 'auth.emailPlaceholder' : 'auth.usernamePlaceholder')" /></div></label>
          </template>

          <template v-else>
            <button class="account-summary" type="button" @click="handleBack"><span><UserRound :size="18" />{{ account }}</span><b>{{ t('auth.change') }}</b></button>
            <label class="auth-label"><span>{{ t('auth.password') }}</span><div class="auth-field"><input ref="passwordInput" v-model="password" :aria-invalid="Boolean(error)" :type="showPassword ? 'text' : 'password'" autocomplete="current-password" :placeholder="t('auth.passwordPlaceholder')" /><button class="password-toggle" type="button" :aria-label="t(showPassword ? 'auth.hidePassword' : 'auth.showPassword')" @click="showPassword = !showPassword"><EyeOff v-if="showPassword" :size="19" /><Eye v-else :size="19" /></button></div></label>
            <button class="forgot-link" type="button" @click="openAuthRoute('forgot-password')">{{ t('auth.forgotPassword') }}</button>
          </template>

          <p v-if="error" class="error-message auth-feedback" role="alert">{{ error }}</p>
          <button class="button button--primary button--full login-submit" type="submit" :disabled="submitting">{{ submitting ? t('auth.loggingIn') : step === 1 ? t('auth.next') : t('auth.login') }}</button>
        </form>
      </div>

      <p class="login-panel__footer">{{ t('auth.noAccount') }} <button type="button" @click="openAuthRoute('register')">{{ t('auth.registerNow') }}</button></p>
    </section>
  </main>
</template>

<style scoped>
.login-page { background: var(--background); display: grid; grid-template-rows: auto minmax(0, 1fr); min-height: 100dvh; padding-top: env(safe-area-inset-top); }
.auth-topbar { align-items: center; background: var(--surface); border-bottom: 1px solid var(--line); display: grid; gap: 8px; grid-template-columns: 44px minmax(0, 1fr) 44px; isolation: isolate; min-height: 72px; padding: 8px 12px; position: sticky; top: 0; z-index: var(--layer-sticky-header); }
.auth-topbar .icon-button { border: 1px solid var(--line); }
.auth-topbar__copy { display: grid; gap: 2px; min-width: 0; text-align: left; }
.auth-topbar__copy span { color: var(--positive); font-family: var(--data-font); font-size: 9px; font-weight: 750; line-height: 1.2; overflow: hidden; text-overflow: ellipsis; text-transform: uppercase; white-space: nowrap; }
.auth-topbar__copy strong { font-size: 17px; font-weight: 780; line-height: 1.2; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.auth-topbar__copy small { color: var(--muted); font-size: 10px; line-height: 1.3; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.login-panel { display: flex; flex-direction: column; margin: 0 auto; max-width: 448px; padding: 24px 24px calc(28px + env(safe-area-inset-bottom)); width: 100%; }
.login-panel__main { width: 100%; }
.login-panel__logo { display: block; height: 30px; margin-bottom: 34px; max-width: 120px; object-fit: contain; object-position: left center; }
.auth-progress { display: grid; gap: 7px; grid-template-columns: repeat(2, 36px); margin-bottom: 20px; }
.auth-progress i { background: var(--line-strong); border-radius: 2px; height: 3px; transition: background-color var(--motion-fast) var(--motion-ease); }
.auth-progress i.active { background: var(--accent); }
.login-panel h1 { font-size: 36px; letter-spacing: 0; line-height: 1.08; margin: 0; overflow-wrap: anywhere; }
.login-panel__main > p { color: var(--muted-strong); font-size: 15px; line-height: 1.6; margin: 13px 0 32px; }
.login-panel form { display: grid; gap: 18px; }
.login-modes { background: var(--soft); border: 1px solid var(--line); border-radius: var(--radius); display: grid; gap: 4px; grid-template-columns: repeat(2, minmax(0, 1fr)); padding: 4px; }
.login-modes button { background: transparent; border: 1px solid transparent; border-radius: calc(var(--radius) - 3px); color: var(--muted); font-size: 14px; font-weight: 720; min-height: 44px; padding: 0 8px; }
.login-modes button.active { background: var(--surface-elevated); border-color: var(--line-strong); box-shadow: var(--shadow-soft); color: var(--ink); }
.auth-label { display: grid; gap: 9px; }
.auth-label > span { font-size: 14px; font-weight: 720; }
.auth-field { align-items: center; background: var(--field-surface); border: 1px solid var(--line); border-radius: var(--radius); color: var(--muted-strong); display: flex; gap: 11px; min-height: 56px; padding: 0 14px; transition: background-color var(--motion-fast) var(--motion-ease), border-color var(--motion-fast) var(--motion-ease), box-shadow var(--motion-fast) var(--motion-ease); }
.auth-field:focus-within { background: var(--surface-elevated); border-color: var(--focus); box-shadow: 0 0 0 3px var(--focus-ring); }
.auth-field:has(input[aria-invalid='true']) { border-color: var(--negative); }
.auth-field input { background: transparent; border: 0; color: var(--ink); font-size: 16px; min-height: 44px; min-width: 0; outline: 0; width: 100%; }
.password-toggle { align-items: center; background: transparent; border-radius: var(--radius); color: var(--muted-strong); display: inline-flex; flex: 0 0 44px; height: 44px; justify-content: center; margin-right: -8px; padding: 0; }
.account-summary { align-items: center; background: var(--soft); border: 1px solid var(--line); border-radius: var(--radius); color: var(--ink); display: flex; gap: 12px; justify-content: space-between; min-height: 54px; padding: 0 14px; text-align: left; width: 100%; }
.account-summary span { align-items: center; display: inline-flex; gap: 9px; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.account-summary b { color: var(--accent); flex: 0 0 auto; font-size: 12px; }
.forgot-link { background: transparent; color: var(--accent); font-size: 13px; font-weight: 720; justify-self: end; margin-top: -9px; min-height: 44px; padding: 0; }
.auth-feedback { background: var(--negative-soft); border: 1px solid currentColor; border-radius: var(--radius); margin: 0; padding: 11px 13px; }
.login-submit { margin-top: 2px; min-height: 52px; }
.login-panel__footer { color: var(--muted-strong); font-size: 14px; margin: auto 0 0; padding-top: 44px; text-align: center; }
.login-panel__footer button { background: transparent; color: var(--accent); font-weight: 750; min-height: 44px; padding: 0 6px; text-decoration: underline; text-underline-offset: 3px; }
@media (max-height: 690px) {
  .login-panel { padding-top: 8px; }
  .login-panel__logo { margin-bottom: 22px; }
  .login-panel__main > p { margin-bottom: 22px; }
  .login-panel__footer { padding-top: 28px; }
}
@media (max-width: 340px) {
  .login-panel { padding-left: 18px; padding-right: 18px; }
  .login-panel__logo { margin-bottom: 26px; }
  .login-panel__main > p { margin-bottom: 26px; }
}
</style>
