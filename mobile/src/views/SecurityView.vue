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
const canUpdateLoginPassword = computed(() => Boolean(loginOldPassword.value && loginNewPassword.value))
const canUpdateFundPassword = computed(() => Boolean(
  fundNewPassword.value
  && (profile.value?.fundPasswordSet ? fundOldPassword.value : fundLoginPassword.value),
))
const canResetFundPassword = computed(() => Boolean(fundPasswordResetCode.value.trim() && fundPasswordResetValue.value))
const protectionCount = computed(() => [
  profile.value?.emailVerified,
  twoFactor.value?.totpEnabled,
  profile.value?.fundPasswordSet,
].filter(Boolean).length)
const protectionPercent = computed(() => Math.round((protectionCount.value / 3) * 100))

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
    setupQr.value = await toDataURL(setup.value.otpAuthUri, { width: 196, margin: 1 })
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
  <main class="page page--plain page--prototype-grid security-page" data-security-workspace="live">
    <PageHeader
      :back="true"
      :eyebrow="t('security.authenticatorStatus')"
      :subtitle="t('security.twoFactorDescription')"
      :title="t('security.title')"
    />
    <div class="page-content security-content">
      <LoginRequiredState v-if="!session.isAuthenticated" :description="t('security.loginDescription')" />
      <section class="security-overview" :aria-label="t('security.title')">
        <header>
          <span>
            <ShieldCheck :size="22" />
            <small>{{ t('security.protectionScore') }}</small>
          </span>
          <strong class="numeric">{{ protectionPercent }}%</strong>
          <p>{{ t('security.protectionSummary', { completed: protectionCount, total: 3 }) }}</p>
          <i aria-hidden="true"><b :style="{ width: `${protectionPercent}%` }" /></i>
        </header>
        <div class="security-overview__checks">
          <div>
            <MailCheck :size="18" />
            <span>
              <small>{{ t('security.emailStatus') }}</small>
              <b :class="{ up: profile?.emailVerified }">{{ profile?.emailVerified ? t('security.enabled') : t('security.notSet') }}</b>
            </span>
          </div>
          <div>
            <ShieldCheck :size="18" />
            <span>
              <small>{{ t('security.authenticatorStatus') }}</small>
              <b :class="{ up: twoFactor?.totpEnabled }">{{ twoFactor?.totpEnabled ? t('security.enabled') : t('security.notSet') }}</b>
            </span>
          </div>
          <div>
            <LockKeyhole :size="18" />
            <span>
              <small>{{ fundPasswordLabel }}</small>
              <b :class="{ up: profile?.fundPasswordSet }">{{ profile?.fundPasswordSet ? t('security.enabled') : t('security.notSet') }}</b>
            </span>
          </div>
        </div>
      </section>
      <template v-if="session.isAuthenticated">
        <p v-if="error" class="error-message security-feedback" role="alert">{{ error }}</p>
        <p v-if="success" class="success-message security-feedback" role="status">{{ success }}</p>
        <p v-if="loading" class="empty-state">{{ t('security.loading') }}</p>
        <template v-else>
          <section class="security-block">
            <header>
              <ShieldCheck :size="21" />
              <div>
                <h2>{{ t('security.twoFactor') }}</h2>
                <p>{{ t('security.twoFactorDescription') }}</p>
              </div>
            </header>
            <div class="security-row">
              <span>
                <b>{{ t('security.authenticatorStatus') }}</b>
                <small>{{ twoFactor?.totpEnabled ? t('security.enabled') : t('security.notSet') }}</small>
              </span>
              <button v-if="!twoFactor?.totpEnabled" class="button button--secondary" type="button" :disabled="saving === 'two-factor-setup'" @click="beginTwoFactorSetup">{{ saving === 'two-factor-setup' ? t('security.preparing') : t('security.setup') }}</button>
              <span v-else class="status-text up">{{ t('security.enabled') }}</span>
            </div>
            <div class="security-row">
              <span>
                <b>{{ t('security.loginTwoFactor') }}</b>
                <small>{{ twoFactor?.canToggleLoginTwoFactor ? t('security.loginCodeRequired') : t('security.managedByPolicy') }}</small>
              </span>
              <label class="switch">
                <input type="checkbox" :aria-label="t('security.loginTwoFactor')" :checked="twoFactor?.loginTwoFactorEnabled" :disabled="!twoFactor?.canToggleLoginTwoFactor || saving === 'two-factor-toggle'" @change="toggleLoginTwoFactor" />
                <i />
              </label>
            </div>
            <button v-if="twoFactor?.totpEnabled" class="reset-toggle" type="button" :disabled="saving === 'two-factor-reset-code'" @click="sendTwoFactorReset">{{ saving === 'two-factor-reset-code' ? t('auth.sendingEllipsis') : t('security.resettingByEmail') }}</button>
            <section v-if="showTwoFactorReset" class="reset-panel">
              <MailCheck :size="19" />
              <div>
                <strong>{{ t('security.resetTwoFactor') }}</strong>
                <p>{{ t('security.resetCodeDescription') }}</p>
              </div>
              <label class="security-field">
                <span>{{ t('security.emailCode') }}</span>
                <input v-model="twoFactorResetCode" inputmode="numeric" autocomplete="one-time-code" :placeholder="t('security.emailCode')" />
              </label>
              <button class="button button--secondary button--full" type="button" :disabled="saving === 'two-factor-reset' || !twoFactorResetCode.trim()" @click="confirmTwoFactorReset">{{ saving === 'two-factor-reset' ? t('auth.resetting') : t('security.confirmReset') }}</button>
            </section>
          </section>

          <section v-if="setup" class="two-factor-setup">
            <h2>{{ t('security.bindAuthenticator') }}</h2>
            <img :src="setupQr" :alt="t('security.qrAlt')" />
            <p>{{ t('security.scanDescription') }}</p>
            <div class="secret-row">
              <code>{{ setup.secret }}</code>
              <button class="icon-button" type="button" :aria-label="t('security.copySecret')" @click="copySecret">
                <Check v-if="copied" :size="19" />
                <Copy v-else :size="19" />
              </button>
            </div>
            <label class="security-field">
              <span>{{ t('security.authenticatorCodePlaceholder') }}</span>
              <input v-model="setupCode" inputmode="numeric" autocomplete="one-time-code" maxlength="8" :placeholder="t('security.authenticatorCodePlaceholder')" />
            </label>
            <button class="button button--primary button--full" type="button" :disabled="saving === 'two-factor-confirm' || !setupCode.trim()" @click="confirmSetup">{{ saving === 'two-factor-confirm' ? t('auth.verifying') : t('security.confirmEnable') }}</button>
          </section>

          <section class="security-block">
            <header>
              <KeyRound :size="21" />
              <div>
                <h2>{{ t('security.loginPassword') }}</h2>
                <p>{{ t('security.loginPasswordDescription') }}</p>
              </div>
            </header>
            <label class="security-field">
              <span>{{ t('security.currentLoginPassword') }}</span>
              <input v-model="loginOldPassword" type="password" autocomplete="current-password" />
            </label>
            <label class="security-field">
              <span>{{ t('security.newLoginPassword') }}</span>
              <input v-model="loginNewPassword" type="password" autocomplete="new-password" />
            </label>
            <button class="button button--secondary button--full" type="button" :disabled="saving === 'login-password' || !canUpdateLoginPassword" @click="updateLoginPassword">{{ saving === 'login-password' ? t('security.updating') : t('security.updateLoginPassword') }}</button>
          </section>

          <section class="security-block">
            <header>
              <LockKeyhole :size="21" />
              <div>
                <h2>{{ fundPasswordLabel }}</h2>
                <p>{{ t('security.fundPasswordDescription') }}</p>
              </div>
            </header>
            <label v-if="!profile?.fundPasswordSet" class="security-field">
              <span>{{ t('security.loginPassword') }}</span>
              <input v-model="fundLoginPassword" type="password" autocomplete="current-password" />
            </label>
            <label v-else class="security-field">
              <span>{{ t('security.oldFundPassword') }}</span>
              <input v-model="fundOldPassword" type="password" autocomplete="off" />
            </label>
            <label class="security-field">
              <span>{{ t('security.newFundPassword') }}</span>
              <input v-model="fundNewPassword" type="password" autocomplete="new-password" />
            </label>
            <button class="button button--secondary button--full" type="button" :disabled="saving === 'fund-password' || !canUpdateFundPassword" @click="updateFundPassword">{{ saving === 'fund-password' ? t('common.saving') : fundPasswordLabel }}</button>
            <button v-if="profile?.fundPasswordSet" class="reset-toggle" type="button" :disabled="saving === 'fund-password-reset-code'" @click="sendFundPasswordReset">{{ saving === 'fund-password-reset-code' ? t('auth.sendingEllipsis') : t('security.forgotFundPassword') }}</button>
            <section v-if="showFundPasswordReset" class="reset-panel">
              <MailCheck :size="19" />
              <div>
                <strong>{{ t('security.resetFundPassword') }}</strong>
                <p>{{ t('security.resetFundDescription') }}</p>
              </div>
              <label class="security-field">
                <span>{{ t('security.emailCode') }}</span>
                <input v-model="fundPasswordResetCode" inputmode="numeric" autocomplete="one-time-code" :placeholder="t('security.emailCode')" />
              </label>
              <label class="security-field">
                <span>{{ t('security.newFundPassword') }}</span>
                <input v-model="fundPasswordResetValue" type="password" autocomplete="new-password" :placeholder="t('security.newFundPasswordPlaceholder')" />
              </label>
              <button class="button button--secondary button--full" type="button" :disabled="saving === 'fund-password-reset' || !canResetFundPassword" @click="confirmFundPasswordReset">{{ saving === 'fund-password-reset' ? t('auth.resetting') : t('security.confirmReset') }}</button>
            </section>
          </section>
        </template>
      </template>
    </div>
  </main>
</template>

<style scoped>
.security-content { display: grid; gap: 22px; padding-bottom: calc(44px + env(safe-area-inset-bottom)); padding-top: 18px; }
.security-feedback { background: var(--surface-elevated); border: 1px solid currentColor; border-radius: var(--radius); line-height: 1.45; margin: 0; padding: 12px 14px; }
.success-message { color: var(--positive); font-size: 13px; font-weight: 680; }
.security-overview {
  background:
    linear-gradient(var(--grid-line) 1px, transparent 1px),
    linear-gradient(90deg, var(--grid-line) 1px, transparent 1px),
    var(--surface);
  background-size: 34px 34px;
  border-bottom: 1px solid var(--line);
  border-top: 3px solid var(--signal-coral);
  display: grid;
  margin: 0 -16px;
}
.security-overview > header { display: grid; gap: 8px; min-height: 132px; padding: 18px 16px 14px; }
.security-overview > header > span { align-items: center; display: flex; gap: 8px; }
.security-overview > header small { color: var(--muted); font-size: 10px; }
.security-overview > header strong { font-size: 32px; line-height: 1; }
.security-overview > header p { color: var(--muted); font-size: 11px; margin: 0; }
.security-overview > header > i { background: var(--soft); display: block; height: 4px; overflow: hidden; }
.security-overview > header > i > b { background: var(--signal-green); display: block; height: 100%; max-width: 100%; }
.security-overview__checks { border-top: 1px solid var(--line); display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); }
.security-overview__checks > div { align-content: center; display: grid; gap: 7px; min-height: 86px; min-width: 0; padding: 10px; }
.security-overview__checks > div + div { border-left: 1px solid var(--line); }
.security-overview__checks > div:nth-child(1) { border-top: 3px solid var(--signal-blue); }
.security-overview__checks > div:nth-child(2) { border-top: 3px solid var(--signal-green); }
.security-overview__checks > div:nth-child(3) { border-top: 3px solid var(--signal-coral); }
.security-overview svg { color: var(--accent); }
.security-overview span { display: grid; gap: 4px; min-width: 0; }
.security-overview small { color: var(--muted); font-size: 11px; line-height: 1.25; }
.security-overview b { font-size: 13px; line-height: 1.25; }
.security-block { background: var(--surface); border-top: 3px solid var(--signal-blue); display: grid; gap: 13px; padding: 20px 12px 14px; }
.security-block:nth-of-type(3) { border-top-color: var(--signal-coral); }
.security-block:nth-of-type(4) { border-top-color: var(--signal-green); }
.security-block header { align-items: flex-start; display: flex; gap: 11px; }
.security-block header > svg { color: var(--accent); flex: 0 0 auto; margin-top: 2px; }
.security-block h2,
.two-factor-setup h2 { font-size: 19px; letter-spacing: 0; margin: 0; }
.security-block header p { color: var(--muted); font-size: 12px; line-height: 1.5; margin: 4px 0 0; }
.security-row { align-items: center; background: var(--soft); border: 1px solid var(--line); border-radius: var(--radius); display: flex; gap: 12px; justify-content: space-between; min-height: 68px; padding: 10px 12px; }
.security-row > span:first-child { display: grid; gap: 4px; min-width: 0; }
.security-row b { font-size: 14px; }
.security-row small { color: var(--muted); font-size: 12px; line-height: 1.35; }
.security-row .button { flex: 0 0 auto; font-size: 12px; min-height: 44px; padding: 0 13px; }
.status-text { flex: 0 0 auto; font-size: 13px; font-weight: 750; }
.switch { align-items: center; display: inline-flex; flex: 0 0 56px; height: 44px; justify-content: flex-end; position: relative; }
.switch input { height: 1px; opacity: 0; position: absolute; width: 1px; }
.switch i { background: var(--line-strong); border: 1px solid var(--line-strong); border-radius: var(--radius); display: block; height: 30px; position: relative; transition: background-color var(--motion-fast) var(--motion-ease), border-color var(--motion-fast) var(--motion-ease); width: 50px; }
.switch i::after { background: var(--surface); border: 1px solid var(--line); border-radius: var(--radius); box-shadow: var(--shadow-soft); content: ''; height: 24px; left: 2px; position: absolute; top: 2px; transition: transform var(--motion-fast) var(--motion-ease); width: 24px; }
.switch input:focus-visible + i { box-shadow: 0 0 0 3px var(--focus-ring); outline: 2px solid var(--focus); outline-offset: 2px; }
.switch input:checked + i { background: var(--positive); border-color: var(--positive); }
.switch input:checked + i::after { transform: translateX(20px); }
.switch input:disabled + i { opacity: .55; }
.two-factor-setup { border-bottom: 1px solid var(--line); border-top: 1px solid var(--accent); display: grid; gap: 13px; padding: 20px 0; }
.two-factor-setup img { border: 1px solid var(--line); border-radius: var(--radius); justify-self: center; max-width: 100%; padding: 6px; width: 196px; }
.two-factor-setup p { color: var(--muted); font-size: 12px; line-height: 1.5; margin: 0; text-align: center; }
.secret-row { align-items: center; background: var(--soft); border: 1px solid var(--line); border-radius: var(--radius); display: grid; grid-template-columns: minmax(0, 1fr) 44px; min-height: 48px; }
.secret-row code { font-size: 12px; overflow: auto; padding-left: 12px; white-space: nowrap; }
.security-field { background: var(--field-surface); border: 1px solid var(--line); border-radius: var(--radius); display: grid; gap: 2px; padding: 7px 12px; }
.security-field:focus-within { background: var(--surface-elevated); border-color: var(--focus); box-shadow: 0 0 0 3px var(--focus-ring); }
.security-field > span { color: var(--muted); font-size: 11px; font-weight: 650; }
.security-field input { background: transparent; border: 0; color: var(--ink); min-height: 30px; outline: 0; padding: 0; width: 100%; }
.reset-toggle { align-items: center; background: transparent; color: var(--accent); display: inline-flex; font-size: 12px; font-weight: 750; justify-self: start; min-height: 44px; padding: 0; }
.reset-panel { background: var(--surface-elevated); border-bottom: 1px solid var(--line); border-top: 1px solid var(--line); display: grid; gap: 11px; grid-template-columns: auto minmax(0, 1fr); padding: 15px 0; }
.reset-panel > svg { color: var(--accent); margin-top: 2px; }
.reset-panel > div { display: grid; gap: 3px; }
.reset-panel strong { font-size: 14px; }
.reset-panel p { color: var(--muted); font-size: 11px; line-height: 1.45; margin: 0; }
.reset-panel .security-field,
.reset-panel button { grid-column: 1 / -1; }
@media (max-width: 360px) {
  .security-overview { margin-left: -12px; margin-right: -12px; }
}
@media (max-width: 340px) {
  .security-content { padding-left: 16px; padding-right: 16px; }
  .security-overview { margin-left: -16px; margin-right: -16px; }
  .security-overview__checks { grid-template-columns: 1fr; }
  .security-overview__checks > div { grid-template-columns: 28px minmax(0, 1fr); min-height: 64px; }
  .security-overview__checks > div + div { border-left: 0; }
  .security-row { align-items: stretch; display: grid; grid-template-columns: minmax(0, 1fr) auto; }
  .security-row .button { padding-inline: 10px; }
}
</style>
