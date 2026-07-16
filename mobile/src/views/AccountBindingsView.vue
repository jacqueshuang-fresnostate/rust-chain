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
    <div class="page-content">
      <LoginRequiredState v-if="!session.isAuthenticated" :description="t('bindings.loginDescription')" />
      <template v-else>
        <p v-if="error" class="error-message">{{ error }}</p>
        <p v-if="success" class="success-message">{{ success }}</p>
        <p v-if="loading" class="empty-state">{{ t('bindings.loading') }}</p>
        <template v-else>
          <section class="binding-intro"><Link2 :size="24" /><div><strong>{{ t('bindings.title') }}</strong><p>{{ t('bindings.introDescription') }}</p></div></section>
          <section class="binding-section"><header><MailCheck :size="20" /><div><h2>{{ t('bindings.email') }}</h2><p>{{ profile?.emailVerified ? t('bindings.emailVerifiedDescription') : t('bindings.emailUnverifiedDescription') }}</p></div></header><div v-if="profile?.emailVerified" class="binding-status"><span><CheckCircle2 :size="18" /></span><div><b>{{ profile.email }}</b><small>{{ t('bindings.verified') }}</small></div></div><template v-else><label><span>{{ t('bindings.emailAddress') }}</span><input v-model="email" class="input" type="email" autocomplete="email" placeholder="name@example.com" /></label><div class="code-row"><input v-model="emailCode" class="input" inputmode="numeric" autocomplete="one-time-code" :placeholder="t('bindings.emailCode')" /><button class="button button--secondary" type="button" :disabled="saving === 'email-code'" @click="sendEmailCode">{{ saving === 'email-code' ? t('auth.sendingEllipsis') : t('bindings.sendCode') }}</button></div><button class="button button--primary button--full" type="button" :disabled="saving === 'email-bind'" @click="saveEmail">{{ saving === 'email-bind' ? t('bindings.binding') : t('bindings.bindEmail') }}</button></template></section>
          <section class="binding-section"><header><WalletCards :size="20" /><div><h2>{{ t('bindings.externalAccounts') }}</h2><p>{{ t('bindings.externalDescription') }}</p></div></header><div v-for="item in enabledProviders" :key="item.provider" class="provider-row"><component :is="item.icon" :size="20" /><div><b>{{ item.label }}</b><small>{{ boundIdentifier(item.provider) || item.description }}</small></div><button v-if="!boundIdentifier(item.provider)" class="button button--secondary" type="button" @click="openProvider(item.provider)">{{ t('bindings.bind') }}</button><span v-else class="bound-label">{{ t('bindings.bound') }}</span></div><p v-if="!enabledProviders.length" class="empty-state">{{ t('bindings.noneEnabled') }}</p></section>
        </template>
      </template>
    </div>

    <div v-if="provider" class="provider-mask" @click.self="provider = null"><form class="provider-dialog" @submit.prevent="saveProvider"><h2>{{ t('bindings.bindProvider', { provider: provider === 'telegram_account' ? 'Telegram' : 'Coinbase Wallet' }) }}</h2><label><span>{{ t('bindings.accountIdentifier') }}</span><input v-model="accountIdentifier" class="input" autofocus :placeholder="t('bindings.accountIdentifierPlaceholder')" /></label><label><span>{{ t('bindings.displayNameOptional') }}</span><input v-model="displayName" class="input" :placeholder="t('bindings.displayNamePlaceholder')" /></label><div><button class="button button--secondary" type="button" @click="provider = null">{{ t('common.cancel') }}</button><button class="button button--primary" type="submit" :disabled="saving.startsWith('provider-')">{{ saving.startsWith('provider-') ? t('common.saving') : t('bindings.confirmBinding') }}</button></div></form></div>
  </main>
</template>

<style scoped>
.account-bindings-page .page-content { display: grid; gap: 22px; padding-bottom: 42px; padding-top: 16px; }
.binding-intro { align-items: center; background: #eef5ff; border: 1px solid #d5e5fd; border-radius: var(--radius); color: #3975ca; display: flex; gap: 11px; padding: 15px; }.binding-intro div { display: grid; gap: 4px; }.binding-intro strong { color: var(--ink); font-size: 17px; }.binding-intro p { color: var(--muted-strong); font-size: 12px; line-height: 1.4; margin: 0; }
.binding-section { border-top: 1px solid var(--line); display: grid; gap: 13px; padding-top: 18px; }.binding-section > header { align-items: flex-start; display: flex; gap: 10px; }.binding-section > header > svg { color: var(--accent); margin-top: 2px; }.binding-section h2 { font-size: 18px; margin: 0; }.binding-section header p { color: var(--muted); font-size: 12px; line-height: 1.45; margin: 4px 0 0; }.binding-section label { display: grid; gap: 7px; }.binding-section label > span { color: var(--muted); font-size: 13px; }.code-row { display: grid; gap: 9px; grid-template-columns: minmax(0, 1fr) 112px; }.code-row .button { font-size: 12px; min-height: 46px; padding: 0 6px; }
.binding-status,.provider-row { align-items: center; background: var(--soft); border-radius: var(--radius); display: flex; gap: 10px; min-height: 58px; padding: 10px 12px; }.binding-status > span { color: var(--positive); }.binding-status div,.provider-row > div { display: grid; gap: 4px; min-width: 0; }.binding-status b,.provider-row b { font-size: 13px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }.binding-status small,.provider-row small { color: var(--muted); font-size: 11px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }.provider-row > svg { color: var(--accent); flex: 0 0 auto; }.provider-row .button { font-size: 12px; margin-left: auto; min-height: 32px; padding: 0 10px; }.bound-label { color: var(--positive); font-size: 12px; font-weight: 700; margin-left: auto; }
.provider-mask { align-items: flex-end; background: rgb(15 23 42 / 42%); display: flex; inset: 0; justify-content: center; padding: 16px 16px calc(16px + env(safe-area-inset-bottom)); position: fixed; z-index: 60; }.provider-dialog { background: white; border-radius: var(--radius); display: grid; gap: 15px; max-height: calc(100dvh - 32px - env(safe-area-inset-top)); max-width: 520px; overflow-y: auto; padding: 18px; width: 100%; }.provider-dialog h2 { font-size: 19px; margin: 0; }.provider-dialog label { display: grid; gap: 7px; }.provider-dialog label > span { color: var(--muted); font-size: 13px; }.provider-dialog > div { display: flex; gap: 10px; justify-content: flex-end; }.provider-dialog .button { min-height: 40px; }.success-message { color: var(--positive); font-size: 13px; font-weight: 650; margin: 0; }
</style>
