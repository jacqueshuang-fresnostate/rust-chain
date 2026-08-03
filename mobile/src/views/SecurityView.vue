<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import {
  Check,
  ChevronRight,
  Copy,
  KeyRound,
  LockKeyhole,
  MailCheck,
  ShieldCheck,
} from 'lucide-vue-next'
import { toDataURL } from 'qrcode'
import { useI18n } from 'vue-i18n'
import { useRoute, useRouter } from 'vue-router'
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
const route = useRoute()
const router = useRouter()
const { t } = useI18n()
const profile = ref<UserProfile | null>(null)
const twoFactor = ref<TwoFactorStatus | null>(null)
const loading = ref(true)
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
type SecurityTask = 'login-password' | 'fund-password' | 'two-factor' | 'recovery'
const activeTask = ref<SecurityTask | null>(null)

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

function openLogin(): void {
  void router.push({ name: 'login', query: { redirect: route.fullPath } })
}

function toggleTask(task: SecurityTask): void {
  activeTask.value = activeTask.value === task ? null : task
}

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
  <main
    class="page page--plain pencil-page security-view"
    data-pencil-source="WZ42z sDl6T"
    data-security-workspace="live"
  >
    <PageHeader
      :back="true"
      :pencil="true"
      :title="t('security.title')"
    />

    <div class="pencil-content security-content" data-security-workflow="live">
      <section v-if="!session.isAuthenticated" class="account-login-state" aria-live="polite">
        <span class="state-icon" aria-hidden="true"><ShieldCheck :size="20" /></span>
        <div>
          <strong>{{ t('common.loginRequiredTitle') }}</strong>
          <p>{{ t('security.loginDescription') }}</p>
        </div>
        <button class="pencil-primary" type="button" @click="openLogin">{{ t('common.loginNow') }}</button>
      </section>

      <section v-else-if="loading" class="compact-state" role="status" aria-live="polite">
        <span class="state-icon" aria-hidden="true"><ShieldCheck :size="20" /></span>
        <div>
          <strong>{{ t('security.protectionScore') }}</strong>
          <p>{{ securityStateLabel }}</p>
        </div>
      </section>

      <section v-else-if="error && !securityReady" class="compact-state compact-state--error" role="alert">
        <span class="state-icon" aria-hidden="true"><ShieldCheck :size="20" /></span>
        <div>
          <strong>{{ t('common.serviceUnavailable') }}</strong>
          <p>{{ error }}</p>
        </div>
        <button class="pencil-secondary" type="button" @click="load">{{ t('common.retry') }}</button>
      </section>

      <template v-else-if="securityReady">
        <section
          class="security-hero"
          :data-protection-score="securityReady ? protectionPercent : '--'"
          aria-live="polite"
        >
          <span class="state-icon" aria-hidden="true"><ShieldCheck :size="20" /></span>
          <div>
            <strong>{{ t('security.protectionScore') }}</strong>
            <p>{{ t('security.protectionSummary', { completed: protectionCount, total: 3 }) }}</p>
          </div>
        </section>

        <div v-if="error || success" class="security-feedback" aria-live="polite">
          <p v-if="error" class="security-feedback--error" role="alert">{{ error }}</p>
          <p v-else role="status">{{ success }}</p>
        </div>

        <section class="security-methods" :aria-label="t('security.title')">
          <button
            class="security-method"
            type="button"
            :aria-expanded="activeTask === 'login-password'"
            @click="toggleTask('login-password')"
          >
            <span>
              <strong>{{ t('security.loginPassword') }}</strong>
              <small>{{ t('security.loginPasswordDescription') }}</small>
            </span>
            <span class="security-method__state is-positive">
              {{ t('security.enabled') }}
              <ChevronRight :size="15" />
            </span>
          </button>

          <section v-if="activeTask === 'login-password'" class="security-panel" data-security-task="password">
            <label class="security-field">
              <span>{{ t('security.currentLoginPassword') }}</span>
              <input v-model="loginOldPassword" type="password" autocomplete="current-password" />
            </label>
            <label class="security-field">
              <span>{{ t('security.newLoginPassword') }}</span>
              <input v-model="loginNewPassword" type="password" autocomplete="new-password" />
            </label>
            <label class="security-field">
              <span>{{ t('auth.confirmNewPassword') }}</span>
              <input v-model="loginNewPasswordConfirm" type="password" autocomplete="new-password" />
            </label>
            <button
              class="pencil-secondary panel-action"
              type="button"
              :disabled="!session.isAuthenticated || saving === 'login-password' || !canUpdateLoginPassword"
              @click="updateLoginPassword"
            >
              {{ saving === 'login-password' ? t('security.updating') : t('security.updateLoginPassword') }}
            </button>
          </section>

          <button
            class="security-method"
            type="button"
            :aria-expanded="activeTask === 'fund-password'"
            @click="toggleTask('fund-password')"
          >
            <span>
              <strong>{{ fundPasswordLabel }}</strong>
              <small>{{ t('security.fundPasswordDescription') }}</small>
            </span>
            <span class="security-method__state" :class="{ 'is-positive': profile?.fundPasswordSet }">
              {{ profile?.fundPasswordSet ? t('security.enabled') : t('security.notSet') }}
              <ChevronRight :size="15" />
            </span>
          </button>

          <section v-if="activeTask === 'fund-password'" class="security-panel" data-security-task="funds">
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
            <button
              class="pencil-secondary panel-action"
              type="button"
              :disabled="!session.isAuthenticated || !securityReady || loading || saving === 'fund-password' || !canUpdateFundPassword"
              @click="updateFundPassword"
            >
              {{ saving === 'fund-password' ? t('common.saving') : fundPasswordLabel }}
            </button>
          </section>

          <button
            class="security-method"
            type="button"
            :aria-expanded="activeTask === 'two-factor'"
            @click="toggleTask('two-factor')"
          >
            <span>
              <strong>{{ t('security.authenticatorStatus') }}</strong>
              <small>{{ t('security.twoFactorDescription') }}</small>
            </span>
            <span class="security-method__state" :class="{ 'is-positive': twoFactor?.totpEnabled }">
              {{ twoFactor?.totpEnabled ? t('security.enabled') : t('security.setup') }}
              <ChevronRight :size="15" />
            </span>
          </button>

          <section v-if="activeTask === 'two-factor'" class="security-panel" data-security-task="two-factor">
            <button
              v-if="!twoFactor?.totpEnabled"
              class="pencil-secondary panel-action"
              type="button"
              :disabled="!session.isAuthenticated || !securityReady || loading || saving === 'two-factor-setup'"
              @click="beginTwoFactorSetup"
            >
              {{ saving === 'two-factor-setup' ? t('security.preparing') : t('security.setup') }}
            </button>

            <label v-else class="policy-toggle">
              <span>
                <strong>{{ t('security.loginTwoFactor') }}</strong>
                <small>{{ twoFactor?.canToggleLoginTwoFactor ? t('security.loginCodeRequired') : t('security.managedByPolicy') }}</small>
              </span>
              <input
                type="checkbox"
                :checked="Boolean(twoFactor?.loginTwoFactorEnabled)"
                :disabled="!session.isAuthenticated || !securityReady || loading || !twoFactor?.canToggleLoginTwoFactor || saving === 'two-factor-toggle'"
                @change="toggleLoginTwoFactor"
              />
              <i :class="{ on: twoFactor?.loginTwoFactorEnabled }" aria-hidden="true" />
            </label>

            <section v-if="setup" class="authenticator-setup" data-security-task="two-factor-setup">
              <img v-if="setupQr" :src="setupQr" :alt="t('security.qrAlt')" />
              <div class="secret-box">
                <span>{{ t('auth.manualSecret') }}</span>
                <code>{{ setup.secret }}</code>
                <button type="button" :aria-label="t('security.copySecret')" @click="copySecret">
                  <Check v-if="copied" :size="18" />
                  <Copy v-else :size="18" />
                </button>
              </div>
              <label class="security-field">
                <span>{{ t('security.authenticatorCodePlaceholder') }}</span>
                <input v-model="setupCode" inputmode="numeric" autocomplete="one-time-code" maxlength="8" />
              </label>
              <button
                class="pencil-primary panel-action"
                type="button"
                :disabled="saving === 'two-factor-confirm' || !setupCode.trim()"
                @click="confirmSetup"
              >
                {{ saving === 'two-factor-confirm' ? t('auth.verifying') : t('security.confirmEnable') }}
              </button>
            </section>
          </section>

          <div class="security-method security-method--policy">
            <span>
              <strong>{{ t('security.emailStatus') }}</strong>
              <small>{{ profile?.emailVerified ? t('bindings.emailVerifiedDescription') : t('bindings.emailUnverifiedDescription') }}</small>
            </span>
            <span class="security-method__state" :class="{ 'is-positive': profile?.emailVerified }">
              {{ profile?.emailVerified ? t('security.enabled') : t('security.notSet') }}
            </span>
          </div>
        </section>

        <button
          class="security-recovery"
          type="button"
          :aria-expanded="activeTask === 'recovery'"
          @click="toggleTask('recovery')"
        >
          <KeyRound :size="16" />
          <span>{{ t('security.confirmReset') }}</span>
          <ChevronRight :size="16" />
        </button>

        <section v-if="activeTask === 'recovery'" class="security-panel recovery-panel" data-security-task="recovery">
          <template v-if="twoFactor?.totpEnabled">
            <div class="recovery-heading">
              <MailCheck :size="18" />
              <div>
                <strong>{{ t('security.resetTwoFactor') }}</strong>
                <small>{{ t('security.resetCodeDescription') }}</small>
              </div>
            </div>
            <button
              class="reset-link"
              type="button"
              :disabled="saving === 'two-factor-reset-code'"
              @click="sendTwoFactorReset"
            >
              {{ saving === 'two-factor-reset-code' ? t('auth.sendingEllipsis') : t('security.resettingByEmail') }}
            </button>
            <div v-if="showTwoFactorReset" class="reset-fields">
              <label class="security-field">
                <span>{{ t('security.emailCode') }}</span>
                <input v-model="twoFactorResetCode" inputmode="numeric" autocomplete="one-time-code" />
              </label>
              <button
                class="pencil-secondary panel-action"
                type="button"
                :disabled="saving === 'two-factor-reset' || !twoFactorResetCode.trim()"
                @click="confirmTwoFactorReset"
              >
                {{ saving === 'two-factor-reset' ? t('auth.resetting') : t('security.confirmReset') }}
              </button>
            </div>
          </template>

          <template v-if="profile?.fundPasswordSet">
            <div class="recovery-heading">
              <LockKeyhole :size="18" />
              <div>
                <strong>{{ t('security.resetFundPassword') }}</strong>
                <small>{{ t('security.resetFundDescription') }}</small>
              </div>
            </div>
            <button
              class="reset-link"
              type="button"
              :disabled="saving === 'fund-password-reset-code'"
              @click="sendFundPasswordReset"
            >
              {{ saving === 'fund-password-reset-code' ? t('auth.sendingEllipsis') : t('security.forgotFundPassword') }}
            </button>
            <div v-if="showFundPasswordReset" class="reset-fields">
              <label class="security-field">
                <span>{{ t('security.emailCode') }}</span>
                <input v-model="fundPasswordResetCode" inputmode="numeric" autocomplete="one-time-code" />
              </label>
              <label class="security-field">
                <span>{{ t('security.newFundPassword') }}</span>
                <input v-model="fundPasswordResetValue" type="password" autocomplete="new-password" />
              </label>
              <button
                class="pencil-secondary panel-action"
                type="button"
                :disabled="saving === 'fund-password-reset' || !canResetFundPassword"
                @click="confirmFundPasswordReset"
              >
                {{ saving === 'fund-password-reset' ? t('auth.resetting') : t('security.confirmReset') }}
              </button>
            </div>
          </template>
        </section>

        <p class="security-note">{{ t('security.fundPasswordDescription') }}</p>
      </template>
    </div>
  </main>
</template>

<style scoped>
.page.pencil-page.security-view {
  background: var(--page);
  background-image: none;
  min-height: 100dvh;
}

.security-content {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 6px 20px calc(20px + env(safe-area-inset-bottom));
}

.security-view button:focus-visible {
  outline: 2px solid var(--focus);
  outline-offset: 2px;
}

.security-view input:focus-visible {
  box-shadow: none;
  outline: 0;
}

.security-hero,
.compact-state,
.account-login-state {
  align-items: center;
  display: grid;
  gap: 12px;
  grid-template-columns: 44px minmax(0, 1fr);
  padding: 4px 0 8px;
}

.state-icon {
  align-items: center;
  background: var(--positive-soft);
  border-radius: 50%;
  color: var(--positive);
  display: inline-flex;
  height: 44px;
  justify-content: center;
  width: 44px;
}

.security-hero div,
.compact-state div,
.account-login-state div {
  display: grid;
  gap: 4px;
  min-width: 0;
}

.security-hero strong,
.compact-state strong,
.account-login-state strong {
  color: var(--ink);
  font-size: 15px;
  font-weight: 700;
  line-height: 1.25;
}

.security-hero p,
.compact-state p,
.account-login-state p {
  color: var(--muted);
  font-size: 11px;
  line-height: 1.4;
  margin: 0;
}

.account-login-state .pencil-primary,
.compact-state .pencil-secondary {
  grid-column: 2;
  justify-self: start;
  min-height: 44px;
  min-width: 132px;
  padding-inline: 18px;
}

.compact-state--error .state-icon {
  background: var(--negative-soft);
  color: var(--negative);
}

.security-feedback {
  background: var(--surface-2);
  border-left: 2px solid var(--positive);
  padding: 8px 10px;
}

.security-feedback p {
  color: var(--positive);
  font-size: 11px;
  line-height: 1.4;
  margin: 0;
}

.security-feedback .security-feedback--error {
  color: var(--negative);
}

.security-feedback:has(.security-feedback--error) {
  border-left-color: var(--negative);
}

.security-methods {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.security-method {
  align-items: center;
  background: transparent;
  color: var(--ink);
  display: flex;
  gap: 12px;
  height: 52px;
  justify-content: space-between;
  min-height: 52px;
  padding: 0;
  text-align: left;
  width: 100%;
}

.security-method > span:first-child {
  display: grid;
  gap: 3px;
  min-width: 0;
}

.security-method strong {
  color: var(--ink);
  font-size: 13px;
  font-weight: 700;
  line-height: 1.2;
}

.security-method small {
  color: var(--muted);
  font-size: 11px;
  line-height: 1.2;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.security-method__state {
  align-items: center;
  color: var(--muted-strong);
  display: inline-flex;
  flex: 0 0 auto;
  font-size: 11px;
  gap: 2px;
  line-height: 1;
}

.security-method__state.is-positive {
  color: var(--positive);
}

.security-method--policy {
  cursor: default;
}

.security-panel {
  background: var(--surface-2);
  display: grid;
  gap: 10px;
  padding: 12px;
}

.security-field {
  background: var(--surface);
  border: 1px solid transparent;
  border-radius: 8px;
  display: grid;
  gap: 3px;
  min-width: 0;
  padding: 6px 10px;
}

.security-field:focus-within {
  border-color: var(--positive);
  box-shadow: 0 0 0 2px var(--focus-ring);
}

.security-field > span {
  color: var(--muted-strong);
  font-size: 11px;
  font-weight: 500;
}

.security-field input {
  background: transparent;
  border: 0;
  color: var(--ink);
  font-size: 14px;
  font-weight: 600;
  min-height: 32px;
  outline: 0;
  padding: 0;
  width: 100%;
}

.panel-action {
  min-height: 48px;
  width: 100%;
}

.policy-toggle {
  align-items: center;
  display: grid;
  gap: 10px;
  grid-template-columns: minmax(0, 1fr) 42px;
  min-height: 52px;
  position: relative;
}

.policy-toggle > span {
  display: grid;
  gap: 3px;
}

.policy-toggle strong {
  color: var(--ink);
  font-size: 13px;
}

.policy-toggle small {
  color: var(--muted);
  font-size: 11px;
}

.policy-toggle input {
  height: 44px;
  opacity: 0;
  position: absolute;
  right: 0;
  width: 44px;
  z-index: 1;
}

.policy-toggle i {
  background: var(--line);
  border-radius: 10px;
  display: block;
  height: 20px;
  position: relative;
  width: 36px;
}

.policy-toggle i::after {
  background: var(--surface);
  border-radius: 50%;
  content: '';
  height: 16px;
  left: 2px;
  position: absolute;
  top: 2px;
  transition: transform 160ms ease;
  width: 16px;
}

.policy-toggle i.on {
  background: var(--positive);
}

.policy-toggle i.on::after {
  transform: translateX(16px);
}

.authenticator-setup,
.reset-fields {
  display: grid;
  gap: 10px;
}

.authenticator-setup > img {
  align-self: center;
  background: var(--surface-elevated);
  border: 1px solid var(--line);
  height: 196px;
  padding: 8px;
  width: 196px;
}

.secret-box {
  align-items: center;
  background: var(--surface);
  display: grid;
  gap: 3px 8px;
  grid-template-columns: minmax(0, 1fr) 44px;
  padding: 8px 8px 8px 12px;
}

.secret-box span {
  color: var(--muted);
  font-size: 10px;
}

.secret-box code {
  color: var(--ink);
  font-size: 12px;
  overflow-wrap: anywhere;
}

.secret-box button {
  background: transparent;
  color: var(--positive);
  grid-column: 2;
  grid-row: 1 / 3;
  min-height: 44px;
  min-width: 44px;
}

.security-recovery {
  align-items: center;
  background: transparent;
  color: var(--ink);
  display: grid;
  gap: 10px;
  grid-template-columns: 16px minmax(0, 1fr) 16px;
  height: 48px;
  min-height: 48px;
  padding: 4px 0 0;
  text-align: left;
  width: 100%;
}

.security-recovery span {
  font-size: 13px;
}

.security-recovery svg:last-child {
  color: var(--muted);
}

.recovery-panel {
  gap: 12px;
}

.recovery-heading {
  align-items: center;
  color: var(--positive);
  display: flex;
  gap: 9px;
}

.recovery-heading > div {
  display: grid;
  gap: 2px;
}

.recovery-heading strong {
  color: var(--ink);
  font-size: 13px;
}

.recovery-heading small {
  color: var(--muted);
  font-size: 11px;
}

.reset-link {
  background: transparent;
  color: var(--positive);
  font-size: 12px;
  font-weight: 650;
  justify-self: start;
  min-height: 44px;
  padding: 0;
  text-align: left;
}

.security-note {
  color: var(--muted);
  font-size: 11px;
  line-height: 1.45;
  margin: 0;
}

@media (max-width: 340px) {
  .security-content {
    padding-inline: 16px;
  }

  .security-method small {
    max-width: 180px;
  }
}

@media (prefers-reduced-motion: reduce) {
  .policy-toggle i::after {
    transition: none;
  }
}
</style>
