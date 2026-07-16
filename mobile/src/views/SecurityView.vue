<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { Check, Copy, KeyRound, LockKeyhole, MailCheck, ShieldCheck } from 'lucide-vue-next'
import { toDataURL } from 'qrcode'
import { useI18n } from 'vue-i18n'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import {
  changeFundPassword,
  changeLoginPassword,
  confirmTwoFactor,
  fetchTwoFactorStatus,
  fetchUserProfile,
  resetFundPassword,
  resetUserTwoFactor,
  sendFundPasswordResetCode,
  sendUserTwoFactorResetCode,
  setFundPassword,
  setupTwoFactor,
  updateLoginTwoFactor,
  type TwoFactorSetup,
  type TwoFactorStatus,
  type UserProfile,
} from '@/api/user'
import { useSessionStore } from '@/stores/session'

const session = useSessionStore()
const { t } = useI18n()
const profile = ref<UserProfile | null>(null)
const twoFactor = ref<TwoFactorStatus | null>(null)
const loading = ref(false)
const error = ref('')
const success = ref('')
const saving = ref('')
const loginOldPassword = ref('')
const loginNewPassword = ref('')
const fundLoginPassword = ref('')
const fundOldPassword = ref('')
const fundNewPassword = ref('')
const setup = ref<TwoFactorSetup | null>(null)
const setupQr = ref('')
const setupCode = ref('')
const copied = ref(false)
const showTwoFactorReset = ref(false)
const twoFactorResetCode = ref('')
const showFundPasswordReset = ref(false)
const fundPasswordResetCode = ref('')
const fundPasswordResetValue = ref('')

const fundPasswordLabel = computed(() => profile.value?.fundPasswordSet ? t('security.changeFundPassword') : t('security.setFundPassword'))

async function load(): Promise<void> {
  if (!session.isAuthenticated) return
  loading.value = true
  error.value = ''
  try {
    const [nextProfile, nextTwoFactor] = await Promise.all([fetchUserProfile(), fetchTwoFactorStatus()])
    profile.value = nextProfile
    twoFactor.value = nextTwoFactor
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('security.loadFailed'))
  } finally {
    loading.value = false
  }
}

async function updateLoginPassword(): Promise<void> {
  if (!loginOldPassword.value || !loginNewPassword.value) {
    error.value = t('security.currentAndNewRequired')
    return
  }
  saving.value = 'login-password'
  error.value = ''
  try {
    await changeLoginPassword(loginOldPassword.value, loginNewPassword.value)
    session.sync()
    loginOldPassword.value = ''
    loginNewPassword.value = ''
    success.value = t('security.loginPasswordUpdated')
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('security.loginPasswordFailed'))
  } finally {
    saving.value = ''
  }
}

async function updateFundPassword(): Promise<void> {
  if (!fundNewPassword.value || (!profile.value?.fundPasswordSet && !fundLoginPassword.value) || (profile.value?.fundPasswordSet && !fundOldPassword.value)) {
    error.value = profile.value?.fundPasswordSet ? t('security.oldAndNewFundRequired') : t('security.loginAndFundRequired')
    return
  }
  saving.value = 'fund-password'
  error.value = ''
  try {
    if (profile.value?.fundPasswordSet) await changeFundPassword(fundOldPassword.value, fundNewPassword.value)
    else await setFundPassword(fundLoginPassword.value, fundNewPassword.value)
    if (profile.value) profile.value = { ...profile.value, fundPasswordSet: true }
    fundLoginPassword.value = ''
    fundOldPassword.value = ''
    fundNewPassword.value = ''
    success.value = t('security.fundPasswordSaved')
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('security.fundPasswordFailed'))
  } finally {
    saving.value = ''
  }
}

async function beginTwoFactorSetup(): Promise<void> {
  saving.value = 'two-factor-setup'
  error.value = ''
  try {
    setup.value = await setupTwoFactor()
    setupQr.value = await toDataURL(setup.value.otpAuthUri, { width: 196, margin: 1, color: { dark: '#121416', light: '#ffffff' } })
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('security.twoFactorInitFailed'))
  } finally {
    saving.value = ''
  }
}

async function confirmSetup(): Promise<void> {
  if (!setupCode.value.trim()) {
    error.value = t('security.authenticatorCodeRequired')
    return
  }
  saving.value = 'two-factor-confirm'
  error.value = ''
  try {
    await confirmTwoFactor(setupCode.value)
    setup.value = null
    setupQr.value = ''
    setupCode.value = ''
    success.value = t('security.twoFactorEnabled')
    await load()
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('security.codeFailed'))
  } finally {
    saving.value = ''
  }
}

async function toggleLoginTwoFactor(event: Event): Promise<void> {
  const target = event.target as HTMLInputElement
  const enabled = target.checked
  if (!twoFactor.value?.totpEnabled) {
    target.checked = false
    await beginTwoFactorSetup()
    return
  }
  saving.value = 'two-factor-toggle'
  error.value = ''
  try {
    await updateLoginTwoFactor(enabled)
    if (twoFactor.value) twoFactor.value = { ...twoFactor.value, loginTwoFactorEnabled: enabled }
    success.value = enabled ? t('security.loginTwoFactorEnabled') : t('security.loginTwoFactorDisabled')
  } catch (reason) {
    target.checked = !enabled
    error.value = apiErrorMessage(reason, t('security.loginTwoFactorFailed'))
  } finally {
    saving.value = ''
  }
}

async function sendTwoFactorReset(): Promise<void> {
  saving.value = 'two-factor-reset-code'
  error.value = ''
  try {
    await sendUserTwoFactorResetCode()
    showTwoFactorReset.value = true
    success.value = t('security.resetCodeSent')
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('security.resetCodeFailed'))
  } finally {
    saving.value = ''
  }
}

async function confirmTwoFactorReset(): Promise<void> {
  if (!twoFactorResetCode.value.trim()) {
    error.value = t('security.emailCodeRequired')
    return
  }
  saving.value = 'two-factor-reset'
  error.value = ''
  try {
    await resetUserTwoFactor(twoFactorResetCode.value)
    twoFactorResetCode.value = ''
    showTwoFactorReset.value = false
    success.value = t('security.twoFactorReset')
    await load()
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('security.twoFactorResetFailed'))
  } finally {
    saving.value = ''
  }
}

async function sendFundPasswordReset(): Promise<void> {
  saving.value = 'fund-password-reset-code'
  error.value = ''
  try {
    await sendFundPasswordResetCode()
    showFundPasswordReset.value = true
    success.value = t('security.resetCodeSent')
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('security.resetCodeFailed'))
  } finally {
    saving.value = ''
  }
}

async function confirmFundPasswordReset(): Promise<void> {
  if (!fundPasswordResetCode.value.trim() || !fundPasswordResetValue.value) {
    error.value = t('security.fundResetFieldsRequired')
    return
  }
  saving.value = 'fund-password-reset'
  error.value = ''
  try {
    await resetFundPassword(fundPasswordResetCode.value, fundPasswordResetValue.value)
    fundPasswordResetCode.value = ''
    fundPasswordResetValue.value = ''
    showFundPasswordReset.value = false
    if (profile.value) profile.value = { ...profile.value, fundPasswordSet: true }
    success.value = t('security.fundPasswordReset')
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('security.fundPasswordResetFailed'))
  } finally {
    saving.value = ''
  }
}

async function copySecret(): Promise<void> {
  if (!setup.value) return
  try {
    await navigator.clipboard.writeText(setup.value.secret)
  } catch {
    const field = document.createElement('textarea')
    field.value = setup.value.secret
    document.body.appendChild(field)
    field.select()
    document.execCommand('copy')
    field.remove()
  }
  copied.value = true
  window.setTimeout(() => { copied.value = false }, 1_600)
}

onMounted(() => { void load() })
</script>

<template>
  <main class="page page--plain security-page">
    <PageHeader :title="t('security.title')" />
    <div class="page-content">
      <LoginRequiredState v-if="!session.isAuthenticated" :description="t('security.loginDescription')" />
      <template v-else>
        <p v-if="error" class="error-message">{{ error }}</p>
        <p v-if="success" class="success-message">{{ success }}</p>
        <p v-if="loading" class="empty-state">{{ t('security.loading') }}</p>
        <template v-else>
          <section class="security-block"><header><ShieldCheck :size="20" /><div><h2>{{ t('security.twoFactor') }}</h2><p>{{ t('security.twoFactorDescription') }}</p></div></header><div class="security-row"><span><b>{{ t('security.authenticatorStatus') }}</b><small>{{ twoFactor?.totpEnabled ? t('security.enabled') : t('security.notSet') }}</small></span><button v-if="!twoFactor?.totpEnabled" class="button button--secondary" type="button" :disabled="saving === 'two-factor-setup'" @click="beginTwoFactorSetup">{{ saving === 'two-factor-setup' ? t('security.preparing') : t('security.setup') }}</button><span v-else class="status-text up">{{ t('security.enabled') }}</span></div><div class="security-row"><span><b>{{ t('security.loginTwoFactor') }}</b><small>{{ twoFactor?.canToggleLoginTwoFactor ? t('security.loginCodeRequired') : t('security.managedByPolicy') }}</small></span><label class="switch"><input type="checkbox" :checked="twoFactor?.loginTwoFactorEnabled" :disabled="!twoFactor?.canToggleLoginTwoFactor || saving === 'two-factor-toggle'" @change="toggleLoginTwoFactor" /><i /></label></div><button v-if="twoFactor?.totpEnabled" class="reset-toggle" type="button" :disabled="saving === 'two-factor-reset-code'" @click="sendTwoFactorReset">{{ saving === 'two-factor-reset-code' ? t('auth.sendingEllipsis') : t('security.resettingByEmail') }}</button><section v-if="showTwoFactorReset" class="reset-panel"><MailCheck :size="18" /><div><strong>{{ t('security.resetTwoFactor') }}</strong><p>{{ t('security.resetCodeDescription') }}</p></div><input v-model="twoFactorResetCode" class="input" inputmode="numeric" autocomplete="one-time-code" :placeholder="t('security.emailCode')" /><button class="button button--secondary button--full" type="button" :disabled="saving === 'two-factor-reset'" @click="confirmTwoFactorReset">{{ saving === 'two-factor-reset' ? t('auth.resetting') : t('security.confirmReset') }}</button></section></section>
          <section v-if="setup" class="two-factor-setup surface"><h2>{{ t('security.bindAuthenticator') }}</h2><img :src="setupQr" :alt="t('security.qrAlt')" /><p>{{ t('security.scanDescription') }}</p><div class="secret-row"><code>{{ setup.secret }}</code><button class="icon-button" type="button" :aria-label="t('security.copySecret')" @click="copySecret"><Check v-if="copied" :size="19" /><Copy v-else :size="19" /></button></div><input v-model="setupCode" class="input" inputmode="numeric" autocomplete="one-time-code" maxlength="8" :placeholder="t('security.authenticatorCodePlaceholder')" /><button class="button button--primary button--full" type="button" :disabled="saving === 'two-factor-confirm'" @click="confirmSetup">{{ saving === 'two-factor-confirm' ? t('auth.verifying') : t('security.confirmEnable') }}</button></section>
          <section class="security-block"><header><KeyRound :size="20" /><div><h2>{{ t('security.loginPassword') }}</h2><p>{{ t('security.loginPasswordDescription') }}</p></div></header><label><span>{{ t('security.currentLoginPassword') }}</span><input v-model="loginOldPassword" class="input" type="password" autocomplete="current-password" /></label><label><span>{{ t('security.newLoginPassword') }}</span><input v-model="loginNewPassword" class="input" type="password" autocomplete="new-password" /></label><button class="button button--secondary button--full" type="button" :disabled="saving === 'login-password'" @click="updateLoginPassword">{{ saving === 'login-password' ? t('security.updating') : t('security.updateLoginPassword') }}</button></section>
          <section class="security-block"><header><LockKeyhole :size="20" /><div><h2>{{ fundPasswordLabel }}</h2><p>{{ t('security.fundPasswordDescription') }}</p></div></header><label v-if="!profile?.fundPasswordSet"><span>{{ t('security.loginPassword') }}</span><input v-model="fundLoginPassword" class="input" type="password" autocomplete="current-password" /></label><label v-else><span>{{ t('security.oldFundPassword') }}</span><input v-model="fundOldPassword" class="input" type="password" autocomplete="off" /></label><label><span>{{ t('security.newFundPassword') }}</span><input v-model="fundNewPassword" class="input" type="password" autocomplete="new-password" /></label><button class="button button--secondary button--full" type="button" :disabled="saving === 'fund-password'" @click="updateFundPassword">{{ saving === 'fund-password' ? t('common.saving') : fundPasswordLabel }}</button><button v-if="profile?.fundPasswordSet" class="reset-toggle" type="button" :disabled="saving === 'fund-password-reset-code'" @click="sendFundPasswordReset">{{ saving === 'fund-password-reset-code' ? t('auth.sendingEllipsis') : t('security.forgotFundPassword') }}</button><section v-if="showFundPasswordReset" class="reset-panel"><MailCheck :size="18" /><div><strong>{{ t('security.resetFundPassword') }}</strong><p>{{ t('security.resetFundDescription') }}</p></div><input v-model="fundPasswordResetCode" class="input" inputmode="numeric" autocomplete="one-time-code" :placeholder="t('security.emailCode')" /><input v-model="fundPasswordResetValue" class="input" type="password" autocomplete="new-password" :placeholder="t('security.newFundPasswordPlaceholder')" /><button class="button button--secondary button--full" type="button" :disabled="saving === 'fund-password-reset'" @click="confirmFundPasswordReset">{{ saving === 'fund-password-reset' ? t('auth.resetting') : t('security.confirmReset') }}</button></section></section>
        </template>
      </template>
    </div>
  </main>
</template>

<style scoped>
.security-page .page-content { display: grid; gap: 22px; padding-bottom: 42px; padding-top: 16px; }.security-block { border-top: 1px solid var(--line); display: grid; gap: 13px; padding-top: 18px; }.security-block header { align-items: flex-start; display: flex; gap: 10px; }.security-block header > svg { color: var(--accent); margin-top: 2px; }.security-block h2 { font-size: 18px; margin: 0; }.security-block header p { color: var(--muted); font-size: 12px; line-height: 1.45; margin: 4px 0 0; }.security-block label:not(.switch) { display: grid; gap: 7px; }.security-block label:not(.switch) > span { color: var(--muted); font-size: 13px; }.security-row { align-items: center; background: var(--soft); border-radius: var(--radius); display: flex; justify-content: space-between; min-height: 61px; padding: 10px 12px; }.security-row > span:first-child { display: grid; gap: 4px; }.security-row b { font-size: 14px; }.security-row small { color: var(--muted); font-size: 12px; }.security-row .button { font-size: 12px; min-height: 33px; padding: 0 12px; }.status-text { font-size: 13px; font-weight: 700; }.switch { display: inline-flex; position: relative; }.switch input { height: 1px; opacity: 0; position: absolute; width: 1px; }.switch i { background: #c2c8ce; border-radius: 15px; display: block; height: 30px; position: relative; transition: background-color .18s ease; width: 50px; }.switch i::after { background: white; border-radius: 50%; box-shadow: 0 1px 3px rgb(15 23 42 / 24%); content: ''; height: 24px; left: 3px; position: absolute; top: 3px; transition: transform .18s ease; width: 24px; }.switch input:checked + i { background: var(--positive); }.switch input:checked + i::after { transform: translateX(20px); }.switch input:disabled + i { opacity: .55; }.two-factor-setup { display: grid; gap: 12px; padding: 16px; }.two-factor-setup h2 { font-size: 18px; margin: 0; }.two-factor-setup img { border: 1px solid var(--line); border-radius: var(--radius); justify-self: center; padding: 6px; width: 196px; }.two-factor-setup p { color: var(--muted); font-size: 12px; line-height: 1.45; margin: 0; text-align: center; }.secret-row { align-items: center; background: var(--soft); border-radius: var(--radius); display: grid; grid-template-columns: 1fr 44px; min-height: 44px; }.secret-row code { font-size: 12px; overflow: auto; padding-left: 12px; white-space: nowrap; }.reset-toggle { background: transparent; color: var(--accent); font-size: 12px; font-weight: 700; justify-self: start; padding: 2px 0; }.reset-panel { background: #f6fbf9; border: 1px solid #d7ebe3; border-radius: var(--radius); display: grid; gap: 10px; grid-template-columns: auto minmax(0, 1fr); padding: 13px; }.reset-panel > svg { color: var(--accent); margin-top: 2px; }.reset-panel > div { display: grid; gap: 3px; }.reset-panel strong { font-size: 14px; }.reset-panel p { color: var(--muted); font-size: 11px; line-height: 1.4; margin: 0; }.reset-panel input,.reset-panel button { grid-column: 1 / -1; }.success-message { color: var(--positive); font-size: 13px; font-weight: 650; margin: 0; }
</style>
