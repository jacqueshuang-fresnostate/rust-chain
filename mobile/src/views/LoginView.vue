<script setup lang="ts">
import { computed, nextTick, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { ArrowLeft, Eye, EyeOff, Languages, UserRound, X } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { fetchLoginConfig, loginWithPassword } from '@/api/auth'
import { apiErrorMessage } from '@/api/client'
import { useSessionStore } from '@/stores/session'
import { goBackOr, sanitizeInternalRedirect } from '@/core/navigation'
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
  void goBackOr(router, '/')
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
      await router.push({ name: 'login-two-factor', query: { challenge: result.challengeId, redirect: route.query.redirect } })
      return
    }
    if (result.type === 'two-factor-setup') {
      await router.push({ name: 'login-two-factor', query: { setup: result.setupChallengeId } })
      return
    }
    session.sync()
    const redirect = sanitizeInternalRedirect(route.query.redirect)
    await router.replace(redirect)
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
      <button class="icon-button" type="button" :aria-label="t('language.title')" @click="router.push({ name: 'language' })"><Languages :size="21" /></button>
    </header>

    <section class="login-panel">
      <div class="login-panel__main">
        <img :src="logo" alt="Hippo" class="login-panel__logo" />
        <div class="auth-progress" aria-hidden="true"><i :class="{ active: step >= 1 }"></i><i :class="{ active: step >= 2 }"></i></div>
        <h1>{{ step === 1 ? t('auth.login') : t('auth.welcomeBack') }}</h1>
        <p>{{ step === 1 ? identityDescription : t('auth.passwordStepDescription') }}</p>

        <form @submit.prevent="submit">
          <template v-if="step === 1">
            <div v-if="usernameLoginEnabled" class="login-modes" role="tablist" :aria-label="t('auth.loginMethod')">
              <button type="button" role="tab" :aria-selected="loginMode === 'email'" :class="{ active: loginMode === 'email' }" @click="selectMode('email')">{{ t('auth.email') }}</button>
              <button type="button" role="tab" :aria-selected="loginMode === 'username'" :class="{ active: loginMode === 'username' }" @click="selectMode('username')">{{ t('auth.username') }}</button>
            </div>
            <label class="auth-label"><span>{{ t(loginMode === 'email' ? 'auth.email' : 'auth.username') }}</span><div class="auth-field"><UserRound :size="19" /><input ref="accountInput" v-model="account" :autocomplete="loginMode === 'email' ? 'email' : 'username'" :inputmode="loginMode === 'email' ? 'email' : 'text'" :placeholder="t(loginMode === 'email' ? 'auth.emailPlaceholder' : 'auth.usernamePlaceholder')" /></div></label>
          </template>

          <template v-else>
            <button class="account-summary" type="button" @click="handleBack"><span><UserRound :size="18" />{{ account }}</span><b>{{ t('auth.change') }}</b></button>
            <label class="auth-label"><span>{{ t('auth.password') }}</span><div class="auth-field"><input ref="passwordInput" v-model="password" :type="showPassword ? 'text' : 'password'" autocomplete="current-password" :placeholder="t('auth.passwordPlaceholder')" /><button class="password-toggle" type="button" :aria-label="t(showPassword ? 'auth.hidePassword' : 'auth.showPassword')" @click="showPassword = !showPassword"><EyeOff v-if="showPassword" :size="19" /><Eye v-else :size="19" /></button></div></label>
            <button class="forgot-link" type="button" @click="router.push({ name: 'forgot-password' })">{{ t('auth.forgotPassword') }}</button>
          </template>

          <p v-if="error" class="error-message" aria-live="polite">{{ error }}</p>
          <button class="button button--primary button--full login-submit" type="submit" :disabled="submitting">{{ submitting ? t('auth.loggingIn') : step === 1 ? t('auth.next') : t('auth.login') }}</button>
        </form>
      </div>

      <p class="login-panel__footer">{{ t('auth.noAccount') }} <button type="button" @click="router.push({ name: 'register' })">{{ t('auth.registerNow') }}</button></p>
    </section>
  </main>
</template>

<style scoped>
.login-page { background: var(--surface); display: grid; grid-template-rows: auto minmax(0, 1fr); min-height: 100dvh; padding-top: env(safe-area-inset-top); }
.auth-topbar { align-items: center; display: flex; justify-content: space-between; min-height: 68px; padding: 8px 16px; }
.login-panel { display: flex; flex-direction: column; margin: 0 auto; max-width: 430px; padding: 22px 24px calc(26px + env(safe-area-inset-bottom)); width: 100%; }
.login-panel__main { width: 100%; }
.login-panel__logo { display: block; height: 28px; margin-bottom: 36px; max-width: 112px; object-fit: contain; object-position: left center; }
.auth-progress { display: grid; gap: 7px; grid-template-columns: repeat(2, 36px); margin-bottom: 20px; }
.auth-progress i { background: var(--line); border-radius: 2px; height: 3px; transition: background-color 160ms ease; }
.auth-progress i.active { background: var(--ink); }
.login-panel h1 { font-size: 36px; line-height: 1.15; margin: 0; overflow-wrap: anywhere; }
.login-panel__main > p { color: var(--muted-strong); font-size: 15px; line-height: 1.65; margin: 12px 0 34px; }
.login-panel form { display: grid; gap: 18px; }
.login-modes { border-bottom: 1px solid var(--line); display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); }
.login-modes button { background: transparent; color: var(--muted); font-size: 16px; font-weight: 700; min-height: 49px; padding: 0 4px; position: relative; }
.login-modes button.active { color: var(--ink); }
.login-modes button.active::after { background: var(--ink); bottom: -1px; content: ''; height: 2px; inset-inline: 16px; position: absolute; }
.auth-label { display: grid; gap: 9px; }
.auth-label > span { font-size: 14px; font-weight: 720; }
.auth-field { align-items: center; background: var(--soft); border: 1px solid transparent; border-radius: var(--radius); color: var(--muted-strong); display: flex; gap: 11px; min-height: 58px; padding: 0 15px; transition: background-color 160ms ease, border-color 160ms ease, box-shadow 160ms ease; }
.auth-field:focus-within { background: white; border-color: var(--accent); box-shadow: 0 0 0 3px rgb(22 124 103 / 9%); }
.auth-field input { background: transparent; border: 0; color: var(--ink); font-size: 16px; min-width: 0; outline: 0; width: 100%; }
.auth-field input::placeholder { color: #a3a8ad; }
.password-toggle { align-items: center; background: transparent; color: var(--muted-strong); display: inline-flex; flex: 0 0 40px; height: 40px; justify-content: center; margin-right: -8px; padding: 0; }
.account-summary { align-items: center; background: var(--soft); border-radius: var(--radius); color: var(--ink); display: flex; gap: 12px; justify-content: space-between; min-height: 54px; padding: 0 14px; text-align: left; width: 100%; }
.account-summary span { align-items: center; display: inline-flex; gap: 9px; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.account-summary b { color: var(--accent); flex: 0 0 auto; font-size: 12px; }
.forgot-link { background: transparent; color: var(--accent); font-size: 13px; font-weight: 700; justify-self: end; margin-top: -7px; padding: 4px 0; }
.login-submit { margin-top: 4px; min-height: 54px; }
.login-panel__footer { color: var(--muted-strong); font-size: 14px; margin: auto 0 0; padding-top: 44px; text-align: center; }
.login-panel__footer button { background: transparent; color: var(--ink); font-weight: 750; padding: 4px; text-decoration: underline; text-underline-offset: 3px; }
@media (max-height: 690px) {
  .login-panel { padding-top: 8px; }
  .login-panel__logo { margin-bottom: 22px; }
  .login-panel__main > p { margin-bottom: 22px; }
  .login-panel__footer { padding-top: 28px; }
}
@media (max-width: 360px) {
  .login-panel { padding-left: 18px; padding-right: 18px; }
  .login-panel h1 { font-size: 32px; }
}
</style>
