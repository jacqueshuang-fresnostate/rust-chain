<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { AtSign, CheckCircle2, Link2, MailCheck, Send, WalletCards } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import {
  bindEmail,
  bindThirdPartyAccount,
  fetchThirdPartyBindings,
  fetchUserProfile,
  sendEmailBindCode,
  type ThirdPartyBindingStatus,
  type ThirdPartyProvider,
  type UserProfile,
} from '@/api/user'
import { useModalDialog } from '@/core/modalDialog'
import { useSessionStore } from '@/stores/session'

const session = useSessionStore()
const { t } = useI18n()
const profile = ref<UserProfile | null>(null)
const bindings = ref<ThirdPartyBindingStatus | null>(null)
const email = ref('')
const emailCode = ref('')
const provider = ref<ThirdPartyProvider | null>(null)
const accountIdentifier = ref('')
const displayName = ref('')
const loading = ref(false)
const saving = ref('')
const error = ref('')
const success = ref('')
const providerOpen = computed(() => provider.value !== null)
const providerDialog = ref<HTMLElement | null>(null)
const { trapFocus: trapProviderFocus } = useModalDialog(providerOpen, providerDialog, '[autofocus]')

const enabledProviders = computed(() => {
  const items: Array<{ provider: ThirdPartyProvider; label: string; description: string; icon: typeof WalletCards }> = []
  if (bindings.value?.coinbaseWalletEnabled) items.push({ provider: 'coinbase_wallet', label: 'Coinbase Wallet', description: t('bindings.walletDescription'), icon: WalletCards })
  if (bindings.value?.telegramAccountEnabled) items.push({ provider: 'telegram_account', label: 'Telegram', description: t('bindings.telegramDescription'), icon: AtSign })
  return items
})

async function load(): Promise<void> {
  if (!session.isAuthenticated) return
  loading.value = true
  error.value = ''
  try {
    const [nextProfile, nextBindings] = await Promise.all([fetchUserProfile(), fetchThirdPartyBindings()])
    profile.value = nextProfile
    bindings.value = nextBindings
    email.value = nextProfile.email || ''
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('bindings.loadFailed'))
  } finally {
    loading.value = false
  }
}

async function sendEmailCode(): Promise<void> {
  if (!email.value.includes('@')) {
    error.value = t('bindings.invalidEmail')
    return
  }
  saving.value = 'email-code'
  error.value = ''
  try {
    await sendEmailBindCode(email.value)
    success.value = t('bindings.codeSent')
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('bindings.codeFailed'))
  } finally {
    saving.value = ''
  }
}

async function saveEmail(): Promise<void> {
  if (!email.value.includes('@') || !emailCode.value.trim()) {
    error.value = t('bindings.emailFieldsRequired')
    return
  }
  saving.value = 'email-bind'
  error.value = ''
  try {
    const boundEmail = await bindEmail(email.value, emailCode.value)
    profile.value = profile.value ? { ...profile.value, email: boundEmail, emailVerified: true } : profile.value
    emailCode.value = ''
    success.value = t('bindings.emailBound')
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('bindings.emailBindFailed'))
  } finally {
    saving.value = ''
  }
}

function openProvider(providerName: ThirdPartyProvider): void {
  provider.value = providerName
  accountIdentifier.value = ''
  displayName.value = ''
  error.value = ''
}

function closeProvider(): void {
  if (saving.value.startsWith('provider-')) return
  provider.value = null
}

function handleProviderDialogKeydown(event: KeyboardEvent): void {
  trapProviderFocus(event, closeProvider)
}

async function saveProvider(): Promise<void> {
  if (!provider.value || !accountIdentifier.value.trim()) {
    error.value = t('bindings.identifierRequired')
    return
  }
  saving.value = `provider-${provider.value}`
  error.value = ''
  try {
    bindings.value = await bindThirdPartyAccount({
      provider: provider.value,
      accountIdentifier: accountIdentifier.value,
      displayName: displayName.value,
    })
    provider.value = null
    success.value = t('bindings.externalBound')
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('bindings.externalBindFailed'))
  } finally {
    saving.value = ''
  }
}

function boundIdentifier(providerName: ThirdPartyProvider): string | undefined {
  return bindings.value?.bindings.find((binding) => binding.provider === providerName && binding.status === 'bound')?.accountIdentifier
}

onMounted(() => { void load() })
</script>

<template>
  <main class="page page--plain account-bindings-page">
    <PageHeader :title="t('bindings.title')" />
    <div class="page-content bindings-content">
      <LoginRequiredState v-if="!session.isAuthenticated" :description="t('bindings.loginDescription')" />
      <template v-else>
        <p v-if="error" class="error-message bindings-feedback" role="alert">{{ error }}</p>
        <p v-if="success" class="success-message bindings-feedback" role="status">{{ success }}</p>
        <p v-if="loading" class="empty-state bindings-loading" role="status">{{ t('bindings.loading') }}</p>
        <template v-else>
          <section class="binding-intro">
            <Link2 :size="24" />
            <div>
              <strong>{{ t('bindings.title') }}</strong>
              <p>{{ t('bindings.introDescription') }}</p>
            </div>
          </section>

          <section class="binding-section">
            <header>
              <MailCheck :size="20" />
              <div>
                <h2>{{ t('bindings.email') }}</h2>
                <p>{{ profile?.emailVerified ? t('bindings.emailVerifiedDescription') : t('bindings.emailUnverifiedDescription') }}</p>
              </div>
            </header>
            <div v-if="profile?.emailVerified" class="binding-status">
              <span><CheckCircle2 :size="18" /></span>
              <div><b>{{ profile.email }}</b><small>{{ t('bindings.verified') }}</small></div>
            </div>
            <template v-else>
              <label>
                <span>{{ t('bindings.emailAddress') }}</span>
                <input v-model="email" class="input" type="email" autocomplete="email" placeholder="name@example.com" />
              </label>
              <div class="code-row">
                <input v-model="emailCode" class="input" inputmode="numeric" autocomplete="one-time-code" :placeholder="t('bindings.emailCode')" />
                <button class="button button--secondary" type="button" :disabled="saving === 'email-code'" @click="sendEmailCode">
                  {{ saving === 'email-code' ? t('auth.sendingEllipsis') : t('bindings.sendCode') }}
                </button>
              </div>
              <button class="button button--primary button--full binding-primary" type="button" :disabled="saving === 'email-bind'" @click="saveEmail">
                {{ saving === 'email-bind' ? t('bindings.binding') : t('bindings.bindEmail') }}
              </button>
            </template>
          </section>

          <section class="binding-section">
            <header>
              <WalletCards :size="20" />
              <div>
                <h2>{{ t('bindings.externalAccounts') }}</h2>
                <p>{{ t('bindings.externalDescription') }}</p>
              </div>
            </header>
            <div v-for="item in enabledProviders" :key="item.provider" class="provider-row">
              <component :is="item.icon" :size="20" />
              <div><b>{{ item.label }}</b><small>{{ boundIdentifier(item.provider) || item.description }}</small></div>
              <button v-if="!boundIdentifier(item.provider)" class="button button--secondary" type="button" @click="openProvider(item.provider)">{{ t('bindings.bind') }}</button>
              <span v-else class="bound-label"><CheckCircle2 :size="15" />{{ t('bindings.bound') }}</span>
            </div>
            <p v-if="!enabledProviders.length" class="empty-state">{{ t('bindings.noneEnabled') }}</p>
          </section>
        </template>
      </template>
    </div>

    <div v-if="provider" class="provider-mask" @click.self="closeProvider">
      <form
        ref="providerDialog"
        class="provider-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="provider-dialog-title"
        :aria-busy="saving.startsWith('provider-')"
        @keydown="handleProviderDialogKeydown"
        @submit.prevent="saveProvider"
      >
        <header>
          <Send :size="20" />
          <h2 id="provider-dialog-title">{{ t('bindings.bindProvider', { provider: provider === 'telegram_account' ? 'Telegram' : 'Coinbase Wallet' }) }}</h2>
        </header>
        <label>
          <span>{{ t('bindings.accountIdentifier') }}</span>
          <input v-model="accountIdentifier" class="input" autofocus :placeholder="t('bindings.accountIdentifierPlaceholder')" />
        </label>
        <label>
          <span>{{ t('bindings.displayNameOptional') }}</span>
          <input v-model="displayName" class="input" :placeholder="t('bindings.displayNamePlaceholder')" />
        </label>
        <div class="provider-dialog__actions">
          <button class="button button--secondary" type="button" :disabled="saving.startsWith('provider-')" @click="closeProvider">{{ t('common.cancel') }}</button>
          <button class="button button--primary" type="submit" :disabled="saving.startsWith('provider-')">
            {{ saving.startsWith('provider-') ? t('common.saving') : t('bindings.confirmBinding') }}
          </button>
        </div>
      </form>
    </div>
  </main>
</template>

<style scoped>
.account-bindings-page { background: var(--background); }
.bindings-content { display: grid; gap: 22px; margin: 0 auto; max-width: 520px; padding-bottom: calc(44px + env(safe-area-inset-bottom)); padding-top: 18px; width: 100%; }
.bindings-feedback { border: 1px solid currentColor; border-radius: var(--radius); line-height: 1.45; margin: 0; padding: 11px 13px; }
.error-message.bindings-feedback { background: var(--negative-soft); }
.success-message { background: var(--positive-soft); color: var(--positive); font-size: 13px; font-weight: 680; }
.bindings-loading { background: var(--soft); border: 1px solid var(--line); border-radius: var(--radius); margin: 0; padding: 24px 12px; }
.binding-intro { align-items: flex-start; border-bottom: 1px solid var(--line); display: flex; gap: 11px; padding: 2px 0 20px; }
.binding-intro > svg { color: var(--accent); flex: 0 0 auto; margin-top: 1px; }
.binding-intro div { display: grid; gap: 4px; min-width: 0; }
.binding-intro strong { font-size: 18px; }
.binding-intro p { color: var(--muted-strong); font-size: 12px; line-height: 1.45; margin: 0; }
.binding-section { border-top: 1px solid var(--line); display: grid; gap: 14px; padding-top: 20px; }
.binding-section > header { align-items: flex-start; display: flex; gap: 10px; }
.binding-section > header > svg { color: var(--accent); flex: 0 0 auto; margin-top: 2px; }
.binding-section h2 { font-size: 19px; letter-spacing: 0; margin: 0; }
.binding-section header p { color: var(--muted); font-size: 12px; line-height: 1.45; margin: 4px 0 0; }
.binding-section label { display: grid; gap: 7px; }
.binding-section label > span { color: var(--muted); font-size: 12px; font-weight: 650; }
.binding-section .input { min-height: 52px; }
.code-row { display: grid; gap: 9px; grid-template-columns: minmax(0, 1fr) 112px; }
.code-row .button { font-size: 12px; min-height: 52px; padding: 0 7px; }
.binding-primary { min-height: 52px; }
.binding-status,.provider-row { align-items: center; background: var(--field-surface); border: 1px solid var(--line); border-radius: var(--radius); display: flex; gap: 10px; min-height: 64px; padding: 10px 12px; }
.binding-status > span { color: var(--positive); }
.binding-status div,.provider-row > div { display: grid; gap: 4px; min-width: 0; }
.binding-status b,.provider-row b { font-size: 13px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.binding-status small,.provider-row small { color: var(--muted); font-size: 11px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.provider-row > svg { color: var(--accent); flex: 0 0 auto; }
.provider-row .button { flex: 0 0 auto; font-size: 12px; margin-left: auto; min-height: 44px; padding: 0 12px; }
.bound-label { align-items: center; color: var(--positive); display: inline-flex; flex: 0 0 auto; font-size: 12px; font-weight: 720; gap: 4px; margin-left: auto; }
.provider-mask { align-items: flex-end; background: var(--overlay); display: flex; inset: 0; justify-content: center; padding: 16px 16px calc(16px + env(safe-area-inset-bottom)); position: fixed; z-index: var(--layer-overlay); }
.provider-dialog { background: var(--surface-elevated); border: 1px solid var(--line); border-radius: var(--radius); box-shadow: var(--shadow-soft); display: grid; gap: 16px; max-height: calc(100dvh - 32px - env(safe-area-inset-top)); max-width: 520px; overflow-y: auto; padding: 20px; width: 100%; }
.provider-dialog > header { align-items: center; border-bottom: 1px solid var(--line); display: flex; gap: 9px; padding-bottom: 14px; }
.provider-dialog > header svg { color: var(--accent); flex: 0 0 auto; }
.provider-dialog h2 { font-size: 19px; letter-spacing: 0; margin: 0; }
.provider-dialog label { display: grid; gap: 7px; }
.provider-dialog label > span { color: var(--muted); font-size: 12px; font-weight: 650; }
.provider-dialog .input { min-height: 52px; }
.provider-dialog__actions { display: grid; gap: 10px; grid-template-columns: repeat(2, minmax(0, 1fr)); }
.provider-dialog .button { min-height: 48px; padding-inline: 10px; }
@media (max-width: 340px) {
  .bindings-content { padding-left: 16px; padding-right: 16px; }
  .code-row { grid-template-columns: minmax(0, 1fr) 96px; }
  .provider-dialog { padding: 16px; }
}
</style>
