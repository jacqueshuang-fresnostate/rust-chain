<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import {
  Check,
  CheckCircle2,
  Copy,
  Fingerprint,
  LockKeyhole,
  MailCheck,
  MonitorSmartphone,
  ShieldCheck,
} from 'lucide-vue-next'
import { toDataURL } from 'qrcode'
import { useI18n } from 'vue-i18n'
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
const securityReady = ref(false)
const saving = ref('')
const loginOldPassword = ref('')
const loginNewPassword = ref('')
const loginNewPasswordConfirm = ref('')
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

const fundPasswordLabel = computed(() => {
  if (!securityReady.value) return t('security.fundProtection')
  return profile.value?.fundPasswordSet ? t('security.changeFundPassword') : t('security.setFundPassword')
})
const securityStateLabel = computed(() => {
  if (loading.value) return t('security.loading')
  if (error.value) return t('common.serviceUnavailable')
  if (!session.isAuthenticated) return t('security.loginDescription')
  return '--'
})
const canUpdateLoginPassword = computed(() => Boolean(
  loginOldPassword.value
  && loginNewPassword.value.length >= 8
  && loginNewPassword.value === loginNewPasswordConfirm.value,
))
const canUpdateFundPassword = computed(() => Boolean(
  securityReady.value
  &&
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
  if (!session.isAuthenticated) {
    profile.value = null
    twoFactor.value = null
    securityReady.value = false
    loading.value = false
    error.value = ''
    return
  }
  loading.value = true
  securityReady.value = false
  error.value = ''
  try {
    const [nextProfile, nextTwoFactor] = await Promise.all([fetchUserProfile(), fetchTwoFactorStatus()])
    profile.value = nextProfile
    twoFactor.value = nextTwoFactor
    securityReady.value = true
  } catch (reason) {
    profile.value = null
    twoFactor.value = null
    error.value = apiErrorMessage(reason, t('security.loadFailed'))
  } finally {
    loading.value = false
  }
}

async function updateLoginPassword(): Promise<void> {
  if (!canUpdateLoginPassword.value) {
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
    loginNewPasswordConfirm.value = ''
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
  const target = event.currentTarget instanceof HTMLInputElement ? event.currentTarget : null
  const enabled = target ? target.checked : !twoFactor.value?.loginTwoFactorEnabled
  if (!twoFactor.value?.totpEnabled) {
    if (target) target.checked = false
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
    if (target) target.checked = !enabled
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
  <main class="secondary-view page page--plain page--prototype-grid security-view" data-security-workspace="live">
    <PageHeader
      :back="true"
      :eyebrow="t('security.scene')"
      :subtitle="t('security.context')"
      :title="t('security.title')"
    />
    <div class="secondary-content page-content security-content">
      <section class="security-page" data-security-workflow="live">
        <section class="protection-overview" :aria-label="t('security.title')">
          <div class="protection-score">
            <span>{{ t('security.protectionScore') }}</span>
            <strong class="numeric">{{ securityReady ? protectionPercent : '--' }}<small>/100</small></strong>
            <div aria-hidden="true"><i :style="{ width: `${securityReady ? protectionPercent : 0}%` }" /></div>
            <p>
              {{ securityReady
                ? t('security.protectionSummary', { completed: protectionCount, total: 3 })
                : securityStateLabel }}
            </p>
          </div>
          <div class="security-checklist">
            <article :class="{ complete: profile?.emailVerified }">
              <span class="security-check-icon">
                <CheckCircle2 v-if="profile?.emailVerified" :size="18" />
                <MailCheck v-else :size="18" />
              </span>
              <div>
                <strong>{{ t('security.emailStatus') }}</strong>
                <small>{{ securityReady ? (profile?.emailVerified ? t('security.enabled') : t('security.notSet')) : securityStateLabel }}</small>
              </div>
              <b>{{ securityReady ? (profile?.emailVerified ? t('security.enabled') : t('security.notSet')) : '--' }}</b>
            </article>
            <article :class="{ complete: twoFactor?.totpEnabled }">
              <span class="security-check-icon">
                <CheckCircle2 v-if="twoFactor?.totpEnabled" :size="18" />
                <Fingerprint v-else :size="18" />
              </span>
              <div>
                <strong>{{ t('security.authenticatorStatus') }}</strong>
                <small>{{ securityReady ? (twoFactor?.totpEnabled ? t('security.enabled') : t('security.notSet')) : securityStateLabel }}</small>
              </div>
              <b>{{ securityReady ? (twoFactor?.totpEnabled ? t('security.enabled') : t('security.notSet')) : '--' }}</b>
            </article>
            <article :class="{ complete: profile?.fundPasswordSet }">
              <span class="security-check-icon">
                <CheckCircle2 v-if="profile?.fundPasswordSet" :size="18" />
                <ShieldCheck v-else :size="18" />
              </span>
              <div>
                <strong>{{ fundPasswordLabel }}</strong>
                <small>{{ securityReady ? (profile?.fundPasswordSet ? t('security.enabled') : t('security.notSet')) : securityStateLabel }}</small>
              </div>
              <b>{{ securityReady ? (profile?.fundPasswordSet ? t('security.enabled') : t('security.notSet')) : '--' }}</b>
            </article>
          </div>
        </section>

        <div class="security-feedback-slot" aria-live="polite">
          <p v-if="error" class="error-message security-feedback" role="alert">{{ error }}</p>
          <p v-else-if="success" class="success-message security-feedback" role="status">{{ success }}</p>
          <p v-else-if="!session.isAuthenticated" class="security-feedback">{{ t('security.loginDescription') }}</p>
          <p v-else-if="loading" class="security-feedback">{{ t('security.loading') }}</p>
          <p v-else class="security-feedback">{{ t('security.twoFactorDescription') }}</p>
        </div>

        <h3 class="group-title">{{ t('security.twoFactor') }}</h3>
        <section class="security-task security-block" data-security-task="two-factor">
          <header>
            <span class="security-task-icon"><Fingerprint :size="20" /></span>
            <div>
              <strong>{{ t('security.twoFactor') }}</strong>
              <small>{{ t('security.twoFactorDescription') }}</small>
            </div>
            <b class="status-badge" :class="twoFactor?.totpEnabled ? 'is-positive' : 'is-pending'">
              {{ securityReady ? (twoFactor?.totpEnabled ? t('security.enabled') : t('security.notSet')) : '--' }}
            </b>
          </header>

          <div class="security-row">
            <span>
              <b>{{ t('security.authenticatorStatus') }}</b>
              <small>{{ securityReady ? (twoFactor?.totpEnabled ? t('security.enabled') : t('security.notSet')) : securityStateLabel }}</small>
            </span>
            <button
              v-if="!twoFactor?.totpEnabled"
              class="button button--secondary"
              type="button"
              :disabled="!session.isAuthenticated || !securityReady || loading || saving === 'two-factor-setup'"
              @click="beginTwoFactorSetup"
            >
              {{ saving === 'two-factor-setup' ? t('security.preparing') : t('security.setup') }}
            </button>
            <span v-else class="status-text up">{{ t('security.enabled') }}</span>
          </div>

          <button
            class="policy-toggle"
            type="button"
            :aria-pressed="Boolean(twoFactor?.loginTwoFactorEnabled)"
            :disabled="!session.isAuthenticated || !securityReady || loading || !twoFactor?.canToggleLoginTwoFactor || saving === 'two-factor-toggle'"
            @click="toggleLoginTwoFactor"
          >
            <span>
              <strong>{{ t('security.loginTwoFactor') }}</strong>
              <small>{{ twoFactor?.canToggleLoginTwoFactor ? t('security.loginCodeRequired') : t('security.managedByPolicy') }}</small>
            </span>
            <i :class="{ on: twoFactor?.loginTwoFactorEnabled }" />
          </button>

          <button
            v-if="twoFactor?.totpEnabled"
            class="reset-toggle"
            type="button"
            :disabled="saving === 'two-factor-reset-code'"
            @click="sendTwoFactorReset"
          >
            {{ saving === 'two-factor-reset-code' ? t('auth.sendingEllipsis') : t('security.resettingByEmail') }}
          </button>

          <section v-if="showTwoFactorReset" class="reset-panel">
            <MailCheck :size="19" />
            <div>
              <strong>{{ t('security.resetTwoFactor') }}</strong>
              <p>{{ t('security.resetCodeDescription') }}</p>
            </div>
            <label class="field security-field" :data-field-state="twoFactorResetCode.trim() ? 'complete' : 'idle'">
              <span>{{ t('security.emailCode') }}</span>
              <div>
                <input v-model="twoFactorResetCode" inputmode="numeric" autocomplete="one-time-code" :placeholder="t('security.emailCode')" />
              </div>
            </label>
            <button class="button button--secondary button--full" type="button" :disabled="saving === 'two-factor-reset' || !twoFactorResetCode.trim()" @click="confirmTwoFactorReset">
              {{ saving === 'two-factor-reset' ? t('auth.resetting') : t('security.confirmReset') }}
            </button>
          </section>
        </section>

        <section v-if="setup" class="security-task two-factor-setup" data-security-task="two-factor-setup">
          <header>
            <span class="security-task-icon"><Fingerprint :size="20" /></span>
            <div>
              <strong>{{ t('security.bindAuthenticator') }}</strong>
              <small>{{ t('security.scanDescription') }}</small>
            </div>
            <b class="status-badge is-pending">{{ t('security.notSet') }}</b>
          </header>
          <div class="secret-box">
            <img :src="setupQr" :alt="t('security.qrAlt')" />
            <code>{{ setup.secret }}</code>
            <button class="icon-button" type="button" :aria-label="t('security.copySecret')" @click="copySecret">
              <Check v-if="copied" :size="19" />
              <Copy v-else :size="19" />
            </button>
          </div>
          <label class="field security-field" :data-field-state="setupCode.trim() ? 'complete' : 'idle'">
            <span>{{ t('security.authenticatorCodePlaceholder') }}</span>
            <div>
              <input v-model="setupCode" inputmode="numeric" autocomplete="one-time-code" maxlength="8" :placeholder="t('security.authenticatorCodePlaceholder')" />
            </div>
          </label>
          <button class="button button--primary button--full" type="button" :disabled="saving === 'two-factor-confirm' || !setupCode.trim()" @click="confirmSetup">
            {{ saving === 'two-factor-confirm' ? t('auth.verifying') : t('security.confirmEnable') }}
          </button>
        </section>

        <section class="security-task security-block" data-security-task="password">
          <header>
            <span class="security-task-icon"><LockKeyhole :size="20" /></span>
            <div>
              <strong>{{ t('security.loginPassword') }}</strong>
              <small>{{ t('security.loginPasswordDescription') }}</small>
            </div>
            <b class="status-badge is-pending">{{ securityReady ? t('security.enabled') : '--' }}</b>
          </header>

          <label class="field security-field" :data-field-state="loginOldPassword ? 'complete' : 'idle'">
            <span>{{ t('security.currentLoginPassword') }}</span>
            <div><input v-model="loginOldPassword" type="password" autocomplete="current-password" :disabled="!session.isAuthenticated || loading" /></div>
          </label>
          <label class="field security-field" :data-field-state="loginNewPassword ? 'complete' : 'idle'">
            <span>{{ t('security.newLoginPassword') }}</span>
            <div>
              <input
                v-model="loginNewPassword"
                type="password"
                autocomplete="new-password"
                :disabled="!session.isAuthenticated || loading"
                :aria-invalid="Boolean(loginNewPassword) && loginNewPassword.length < 8"
              />
            </div>
          </label>
          <label
            class="field security-field"
            :data-field-state="loginNewPasswordConfirm && loginNewPassword.length >= 8 && loginNewPasswordConfirm === loginNewPassword ? 'complete' : loginNewPasswordConfirm ? 'invalid' : 'idle'"
          >
            <span>{{ t('auth.confirmNewPassword') }}</span>
            <div>
              <input
                v-model="loginNewPasswordConfirm"
                type="password"
                autocomplete="new-password"
                :disabled="!session.isAuthenticated || loading"
                :aria-invalid="Boolean(loginNewPasswordConfirm) && loginNewPasswordConfirm !== loginNewPassword"
              />
            </div>
          </label>
          <p class="field-hint">
            {{ t('auth.passwordLengthRule') }} · {{ t('auth.passwordMatchRule') }}
          </p>
          <button class="button button--secondary button--full" type="button" :disabled="!session.isAuthenticated || saving === 'login-password' || !canUpdateLoginPassword" @click="updateLoginPassword">
            {{ saving === 'login-password' ? t('security.updating') : t('security.updateLoginPassword') }}
          </button>
        </section>

        <section class="security-task security-block" data-security-task="funds">
          <header>
            <span class="security-task-icon"><ShieldCheck :size="20" /></span>
            <div>
              <strong>{{ fundPasswordLabel }}</strong>
              <small>{{ t('security.fundPasswordDescription') }}</small>
            </div>
            <b class="status-badge" :class="profile?.fundPasswordSet ? 'is-positive' : 'is-pending'">
              {{ profile?.fundPasswordSet ? t('security.enabled') : t('security.notSet') }}
            </b>
          </header>

          <label v-if="!profile?.fundPasswordSet" class="field security-field" :data-field-state="fundLoginPassword ? 'complete' : 'idle'">
            <span>{{ t('security.loginPassword') }}</span>
            <div><input v-model="fundLoginPassword" type="password" autocomplete="current-password" :disabled="!session.isAuthenticated || !securityReady || loading" /></div>
          </label>
          <label v-else class="field security-field" :data-field-state="fundOldPassword ? 'complete' : 'idle'">
            <span>{{ t('security.oldFundPassword') }}</span>
            <div><input v-model="fundOldPassword" type="password" autocomplete="off" :disabled="!session.isAuthenticated || !securityReady || loading" /></div>
          </label>
          <label class="field security-field" :data-field-state="fundNewPassword ? 'complete' : 'idle'">
            <span>{{ t('security.newFundPassword') }}</span>
            <div><input v-model="fundNewPassword" type="password" autocomplete="new-password" :disabled="!session.isAuthenticated || !securityReady || loading" /></div>
          </label>
          <button class="button button--secondary button--full" type="button" :disabled="!session.isAuthenticated || !securityReady || saving === 'fund-password' || !canUpdateFundPassword" @click="updateFundPassword">
            {{ saving === 'fund-password' ? t('common.saving') : fundPasswordLabel }}
          </button>
          <button
            v-if="profile?.fundPasswordSet"
            class="reset-toggle"
            type="button"
            :disabled="saving === 'fund-password-reset-code'"
            @click="sendFundPasswordReset"
          >
            {{ saving === 'fund-password-reset-code' ? t('auth.sendingEllipsis') : t('security.forgotFundPassword') }}
          </button>
          <section v-if="showFundPasswordReset" class="reset-panel">
            <MailCheck :size="19" />
            <div>
              <strong>{{ t('security.resetFundPassword') }}</strong>
              <p>{{ t('security.resetFundDescription') }}</p>
            </div>
            <label class="field security-field" :data-field-state="fundPasswordResetCode.trim() ? 'complete' : 'idle'">
              <span>{{ t('security.emailCode') }}</span>
              <div><input v-model="fundPasswordResetCode" inputmode="numeric" autocomplete="one-time-code" :placeholder="t('security.emailCode')" /></div>
            </label>
            <label class="field security-field" :data-field-state="fundPasswordResetValue ? 'complete' : 'idle'">
              <span>{{ t('security.newFundPassword') }}</span>
              <div><input v-model="fundPasswordResetValue" type="password" autocomplete="new-password" :placeholder="t('security.newFundPasswordPlaceholder')" /></div>
            </label>
            <button class="button button--secondary button--full" type="button" :disabled="saving === 'fund-password-reset' || !canResetFundPassword" @click="confirmFundPasswordReset">
              {{ saving === 'fund-password-reset' ? t('auth.resetting') : t('security.confirmReset') }}
            </button>
          </section>
        </section>

        <section class="device-section">
          <div class="section-heading-row">
            <h3 class="group-title">{{ t('security.title') }}</h3>
            <span>{{ t('security.managedByPolicy') }}</span>
          </div>
          <div class="device-list">
            <article data-device-state="unavailable">
              <span class="device-icon"><MonitorSmartphone :size="19" /></span>
              <div>
                <strong>{{ t('security.managedByPolicy') }}</strong>
                <small>{{ t('security.twoFactorDescription') }}</small>
              </div>
              <button type="button" disabled>{{ t('security.notSet') }}</button>
            </article>
          </div>
        </section>
      </section>
    </div>
  </main>
</template>

<style scoped>
.security-page {
  display: grid;
  gap: 16px;
  min-width: 0;
}

.security-feedback-slot {
  display: grid;
  min-height: 58px;
}

.security-feedback {
  align-content: center;
  background: var(--soft);
  border-left: 3px solid var(--line-strong);
  color: var(--muted);
  display: grid;
  font-size: 11px;
  line-height: 1.45;
  margin: 0;
  min-height: 52px;
  padding: 9px 12px;
}

.error-message.security-feedback {
  background: var(--negative-soft);
  border-left-color: var(--negative);
  color: var(--negative);
}

.success-message.security-feedback {
  background: var(--positive-soft);
  border-left-color: var(--positive);
  color: var(--positive);
}

.protection-overview {
  border-block: 1px solid var(--line-strong);
  display: grid;
  min-width: 0;
}

.protection-score {
  border-bottom: 1px solid var(--line);
  display: grid;
  gap: 7px;
  padding: 16px 0;
}

.protection-score > span,
.protection-score p {
  color: var(--muted);
  font-size: 10px;
}

.protection-score strong {
  font-size: 31px;
}

.protection-score strong small {
  color: var(--muted);
  font-size: 11px;
}

.protection-score > div {
  background: var(--soft);
  height: 4px;
  overflow: hidden;
}

.protection-score i {
  background: linear-gradient(90deg, var(--signal-coral), var(--signal-blue), var(--signal-green));
  display: block;
  height: 100%;
  transition: width var(--motion-fast) var(--motion-ease);
}

.protection-score p { margin: 0; }

.security-checklist {
  display: grid;
}

.security-checklist article {
  align-items: center;
  border-bottom: 1px solid var(--line);
  display: grid;
  gap: 9px;
  grid-template-columns: 36px minmax(0, 1fr) auto;
  min-height: 66px;
  min-width: 0;
}

.security-checklist article:last-child { border-bottom: 0; }

.security-check-icon,
.security-task-icon,
.device-icon,
.confirmation-icon {
  border: 1px solid var(--line);
  color: var(--accent);
  display: grid;
  height: 36px;
  place-items: center;
  width: 36px;
}

.security-checklist article.complete .security-check-icon {
  color: var(--positive);
}

.security-checklist article > div {
  display: grid;
  gap: 3px;
  min-width: 0;
}

.security-checklist strong,
.security-checklist small {
  font-size: 10px;
  overflow-wrap: anywhere;
}

.security-checklist small { color: var(--muted); }

.security-checklist article > b {
  color: var(--negative);
  font-size: 9px;
}

.security-checklist article.complete > b { color: var(--positive); }

.security-task {
  border-block: 1px solid var(--line-strong);
  display: grid;
  gap: 13px;
  min-width: 0;
  padding-block: 14px;
}

.security-task + .security-task {
  border-top: 0;
  margin-top: -17px;
}

.security-task > header {
  align-items: center;
  display: grid;
  gap: 10px;
  grid-template-columns: 38px minmax(0, 1fr) auto;
  min-width: 0;
}

.security-task > header > div {
  display: grid;
  gap: 3px;
  min-width: 0;
}

.security-task > header strong { font-size: 12px; }

.security-task > header small,
.security-task > p {
  color: var(--muted);
  font-size: 10px;
  line-height: 1.5;
}

.status-badge {
  color: var(--accent);
  font-size: 9px;
  max-width: 72px;
  overflow-wrap: anywhere;
  text-align: right;
}

.status-badge.is-positive { color: var(--positive); }
.status-badge.is-pending { color: var(--accent); }

.security-row {
  align-items: center;
  background: var(--soft);
  border: 1px solid var(--line);
  display: flex;
  gap: 12px;
  justify-content: space-between;
  min-height: 68px;
  padding: 10px 12px;
}

.security-row > span:first-child {
  display: grid;
  gap: 4px;
  min-width: 0;
}

.security-row b { font-size: 12px; }

.security-row small {
  color: var(--muted);
  font-size: 10px;
  line-height: 1.35;
}

.security-row .button {
  flex: 0 0 auto;
  font-size: 11px;
  min-height: 44px;
  padding-inline: 13px;
}

.status-text {
  flex: 0 0 auto;
  font-size: 11px;
  font-weight: 750;
}

.policy-toggle {
  align-items: center;
  background: transparent;
  border: 1px solid var(--line);
  color: var(--ink);
  display: grid;
  gap: 12px;
  grid-template-columns: minmax(0, 1fr) 50px;
  min-height: 58px;
  padding: 8px 10px;
  text-align: left;
}

.policy-toggle > span {
  display: grid;
  gap: 3px;
  min-width: 0;
}

.policy-toggle strong { font-size: 11px; }

.policy-toggle small {
  color: var(--muted);
  font-size: 9px;
  line-height: 1.4;
}

.policy-toggle i {
  background: var(--line-strong);
  border: 1px solid var(--line-strong);
  display: block;
  height: 28px;
  position: relative;
  width: 48px;
}

.policy-toggle i::after {
  background: var(--surface);
  border: 1px solid var(--line);
  content: '';
  height: 22px;
  left: 2px;
  position: absolute;
  top: 2px;
  transition: transform var(--motion-fast) var(--motion-ease);
  width: 22px;
}

.policy-toggle i.on {
  background: var(--positive);
  border-color: var(--positive);
}

.policy-toggle i.on::after { transform: translateX(20px); }

.two-factor-setup img {
  border: 1px solid var(--line);
  max-height: 148px;
  max-width: 148px;
  padding: 4px;
}

.secret-box {
  align-items: center;
  background: var(--soft);
  border: 1px solid var(--line);
  display: grid;
  gap: 8px;
  grid-template-columns: auto minmax(0, 1fr) 44px;
  min-height: 64px;
  padding: 7px;
}

.secret-box code {
  font-size: 10px;
  min-width: 0;
  overflow: auto;
  white-space: nowrap;
}

.secret-box button {
  min-height: 44px;
  min-width: 44px;
}

.security-field {
  background: var(--field-surface);
  border: 1px solid var(--line);
  display: grid;
  gap: 2px;
  min-width: 0;
  padding: 7px 12px;
}

.security-field:focus-within {
  background: var(--surface-elevated);
  border-color: var(--focus);
  box-shadow: 0 0 0 3px var(--focus-ring);
}

.security-field[data-field-state='invalid'] {
  border-color: var(--negative);
}

.security-field > span {
  color: var(--muted);
  font-size: 11px;
  font-weight: 650;
}

.security-field > div {
  align-items: center;
  display: grid;
  min-height: 36px;
}

.security-field input {
  background: transparent;
  border: 0;
  color: var(--ink);
  min-height: 36px;
  min-width: 0;
  outline: 0;
  padding: 0;
  width: 100%;
}

.reset-toggle {
  align-items: center;
  background: transparent;
  color: var(--accent);
  display: inline-flex;
  font-size: 11px;
  font-weight: 750;
  justify-self: start;
  min-height: 44px;
  padding: 0;
}

.reset-panel {
  background: var(--surface-elevated);
  border-block: 1px solid var(--line);
  display: grid;
  gap: 11px;
  grid-template-columns: auto minmax(0, 1fr);
  padding: 15px 0;
}

.reset-panel > svg {
  color: var(--accent);
  margin-top: 2px;
}

.reset-panel > div {
  display: grid;
  gap: 3px;
}

.reset-panel strong { font-size: 14px; }

.reset-panel p {
  color: var(--muted);
  font-size: 11px;
  line-height: 1.45;
  margin: 0;
}

.reset-panel .security-field,
.reset-panel button { grid-column: 1 / -1; }

.device-section {
  display: grid;
  gap: 8px;
}

.device-section .section-heading-row > span {
  color: var(--muted);
  font-size: 9px;
}

.device-list {
  border-top: 1px solid var(--line);
  display: grid;
}

.device-list article {
  align-items: center;
  border-bottom: 1px solid var(--line);
  display: grid;
  gap: 10px;
  grid-template-columns: 38px minmax(0, 1fr) auto;
  min-height: 70px;
  min-width: 0;
}

.device-list article > div {
  display: grid;
  gap: 3px;
  min-width: 0;
}

.device-list strong,
.device-list small {
  font-size: 10px;
  overflow-wrap: anywhere;
}

.device-list small { color: var(--muted); }

.device-list button {
  background: transparent;
  border: 1px solid var(--line);
  color: var(--muted);
  font-size: 9px;
  min-height: 44px;
  min-width: 78px;
  padding-inline: 9px;
}

@media (max-width: 340px) {
  .security-task > header,
  .device-list article {
    grid-template-columns: 36px minmax(0, 1fr);
  }

  .security-task > header > .status-badge,
  .device-list button {
    grid-column: 2;
    justify-self: start;
  }

  .security-row {
    align-items: stretch;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
  }

  .security-row .button { padding-inline: 10px; }

  .secret-box {
    grid-template-columns: minmax(0, 1fr) 44px;
  }

  .secret-box img {
    grid-column: 1 / -1;
    justify-self: center;
  }
}
</style>
