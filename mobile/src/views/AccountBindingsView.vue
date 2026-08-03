<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { LockKeyhole, Plus, Send } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { useRoute, useRouter } from 'vue-router'
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
const route = useRoute()
const router = useRouter()
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
const showEmailEditor = ref(false)
const providerOpen = computed(() => provider.value !== null)
const providerDialog = ref<HTMLElement | null>(null)
const { trapFocus: trapProviderFocus } = useModalDialog(providerOpen, providerDialog, '[autofocus]')

function openLogin(): void {
  void router.push({ name: 'login', query: { redirect: route.fullPath } })
}

const enabledProviders = computed(() => {
  const items: Array<{ provider: ThirdPartyProvider; label: string; description: string }> = []
  if (bindings.value?.coinbaseWalletEnabled) items.push({ provider: 'coinbase_wallet', label: 'Coinbase Wallet', description: t('bindings.walletDescription') })
  if (bindings.value?.telegramAccountEnabled) items.push({ provider: 'telegram_account', label: 'Telegram', description: t('bindings.telegramDescription') })
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
    showEmailEditor.value = false
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
  <main
    class="page page--plain pencil-page account-bindings-page"
    data-pencil-source="x84Cbv Z0ging"
  >
    <PageHeader
      :back="true"
      :pencil="true"
      :title="t('bindings.title')"
    />
    <div class="pencil-content bindings-content">
      <section v-if="!session.isAuthenticated" class="account-login-state">
        <span class="account-login-state__icon"><LockKeyhole :size="20" /></span>
        <div><strong>{{ t('common.loginRequiredTitle') }}</strong><p>{{ t('bindings.loginDescription') }}</p></div>
        <button class="pencil-primary" type="button" @click="openLogin">{{ t('common.loginNow') }}</button>
      </section>
      <template v-else>
        <p v-if="error" class="pencil-message pencil-message--error bindings-feedback" role="alert">{{ error }}</p>
        <p v-else-if="success" class="pencil-message pencil-message--success bindings-feedback" role="status">{{ success }}</p>
        <p v-if="loading" class="bindings-loading" role="status">{{ t('bindings.loading') }}</p>
        <template v-else>
          <section class="binding-list" :aria-label="t('bindings.title')">
            <button class="binding-row" type="button" :disabled="profile?.emailVerified" @click="showEmailEditor = !showEmailEditor">
              <span><strong>{{ t('bindings.email') }}</strong><small>{{ profile?.emailVerified ? t('bindings.emailVerifiedDescription') : t('bindings.emailUnverifiedDescription') }}</small></span>
              <b :class="{ 'is-positive': profile?.emailVerified }">{{ profile?.emailVerified ? t('bindings.verified') : t('bindings.bind') }}</b>
            </button>

            <section v-if="showEmailEditor && !profile?.emailVerified" class="binding-editor" :aria-label="t('bindings.bindEmail')">
              <label class="binding-field">
                <span>{{ t('bindings.emailAddress') }}</span>
                <input v-model="email" type="email" autocomplete="email" :placeholder="t('auth.emailPlaceholder')" />
              </label>
              <div class="code-row">
                <label class="binding-field binding-field--code">
                  <span>{{ t('bindings.emailCode') }}</span>
                  <input v-model="emailCode" inputmode="numeric" autocomplete="one-time-code" :placeholder="t('bindings.emailCode')" />
                </label>
                <button class="pencil-secondary" type="button" :disabled="saving === 'email-code'" @click="sendEmailCode">
                  {{ saving === 'email-code' ? t('auth.sendingEllipsis') : t('bindings.sendCode') }}
                </button>
              </div>
              <button class="pencil-primary binding-primary" type="button" :disabled="saving === 'email-bind'" @click="saveEmail">
                {{ saving === 'email-bind' ? t('bindings.binding') : t('bindings.bindEmail') }}
              </button>
            </section>

            <div class="binding-row binding-row--static">
              <span><strong>{{ t('auth.account') }}</strong><small>{{ profile?.phone || t('security.notSet') }}</small></span>
              <b :class="{ 'is-positive': Boolean(profile?.phone) }">{{ profile?.phone ? t('bindings.bound') : t('security.notSet') }}</b>
            </div>

            <button
              v-for="item in enabledProviders"
              :key="item.provider"
              class="binding-row"
              type="button"
              :disabled="Boolean(boundIdentifier(item.provider))"
              @click="openProvider(item.provider)"
            >
              <span><strong>{{ item.label }}</strong><small>{{ boundIdentifier(item.provider) || item.description }}</small></span>
              <b :class="{ 'is-positive': Boolean(boundIdentifier(item.provider)) }">{{ boundIdentifier(item.provider) ? t('bindings.bound') : t('bindings.bind') }}</b>
            </button>
          </section>

          <button v-if="!profile?.emailVerified" class="binding-add" type="button" @click="showEmailEditor = true">
            <Plus :size="16" />{{ t('bindings.bindEmail') }}
          </button>
          <p class="bindings-note">{{ enabledProviders.length ? t('bindings.externalDescription') : t('bindings.noneEnabled') }}</p>
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
        <label class="binding-field">
          <span>{{ t('bindings.accountIdentifier') }}</span>
          <input v-model="accountIdentifier" class="input" autofocus :placeholder="t('bindings.accountIdentifierPlaceholder')" />
        </label>
        <label class="binding-field">
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
.page.pencil-page.account-bindings-page { background: var(--page); background-image: none; min-height: 100dvh; }
.bindings-content { display: flex; flex-direction: column; gap: 10px; padding-bottom: calc(20px + env(safe-area-inset-bottom)); padding-top: 6px; }
.account-login-state { align-items: center; display: grid; gap: 12px; grid-template-columns: 44px minmax(0, 1fr) auto; min-height: 76px; }
.account-login-state__icon { align-items: center; background: var(--accent-soft); border-radius: 50%; color: var(--positive); display: inline-flex; height: 44px; justify-content: center; width: 44px; }
.account-login-state div { display: grid; gap: 3px; min-width: 0; }
.account-login-state strong { color: var(--ink); font-size: 14px; }
.account-login-state p { color: var(--muted); font-size: 11px; line-height: 16px; margin: 0; }
.account-login-state .pencil-primary { min-height: 44px; padding-inline: 16px; }
.bindings-feedback { margin: 0; }
.bindings-loading { color: var(--muted); font-size: 11px; margin: 0; min-height: 44px; padding-block: 14px; }
.binding-list { display: grid; gap: 10px; }
.binding-row { align-items: center; background: transparent; color: var(--ink); display: flex; gap: 12px; height: 52px; justify-content: space-between; min-height: 52px; padding: 0; text-align: left; width: 100%; }
.binding-row > span { display: grid; gap: 3px; min-width: 0; }
.binding-row strong { color: var(--ink); font-size: 13px; font-weight: 700; line-height: 18px; }
.binding-row small { color: var(--muted); font-size: 11px; line-height: 15px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.binding-row > b { color: var(--muted); flex: 0 0 auto; font-size: 11px; font-weight: 400; max-width: 92px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.binding-row > b.is-positive { color: var(--positive); }
.binding-row:disabled { cursor: default; opacity: 1; }
.binding-row--static { cursor: default; }
.binding-editor { background: var(--surface-2); border-radius: 10px; display: grid; gap: 10px; margin-block: 4px 10px; padding: 12px; }
.binding-field { background: var(--surface); border: 1px solid transparent; border-radius: 8px; display: grid; gap: 3px; min-width: 0; padding: 6px 10px; }
.binding-field:focus-within { border-color: var(--positive); box-shadow: 0 0 0 2px var(--focus-ring); }
.binding-field > span { color: var(--muted); font-size: 10px; font-weight: 500; }
.binding-field input { background: transparent; border: 0; color: var(--ink); min-height: 32px; min-width: 0; outline: 0; padding: 0; width: 100%; }
.code-row { display: grid; gap: 8px; grid-template-columns: minmax(0, 1fr) 108px; }
.code-row .pencil-secondary { min-height: 48px; padding-inline: 8px; }
.binding-primary { min-height: 48px; width: 100%; }
.binding-add { align-items: center; background: transparent; color: var(--positive); display: inline-flex; font-size: 13px; gap: 8px; height: 44px; justify-content: flex-start; min-height: 44px; padding: 0; width: 100%; }
.bindings-note { color: var(--muted); font-size: 11px; line-height: 16px; margin: 0; }
.account-bindings-page button:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
.provider-mask { align-items: flex-end; background: var(--overlay); display: flex; inset: 0; justify-content: center; padding: 16px 16px calc(16px + env(safe-area-inset-bottom)); position: fixed; z-index: var(--layer-overlay); }
.provider-dialog { background: var(--surface-elevated); border-radius: 20px; display: grid; gap: 16px; max-height: calc(100dvh - 32px - env(safe-area-inset-top)); max-width: 520px; overflow-y: auto; padding: 20px; width: 100%; }
.provider-dialog > header { align-items: center; border-bottom: 1px solid var(--line); display: flex; gap: 9px; padding-bottom: 14px; }
.provider-dialog > header svg { color: var(--accent); flex: 0 0 auto; }
.provider-dialog h2 { font-size: 19px; letter-spacing: 0; margin: 0; }
.provider-dialog label { display: grid; gap: 7px; }
.provider-dialog label > span { color: var(--muted); font-size: 12px; font-weight: 650; }
.provider-dialog__actions { display: grid; gap: 10px; grid-template-columns: repeat(2, minmax(0, 1fr)); }
.provider-dialog .button { min-height: 48px; padding-inline: 10px; }
@media (max-width: 340px) {
  .bindings-content { padding-inline: 16px; }
  .account-login-state { align-items: start; grid-template-columns: 44px minmax(0, 1fr); }
  .account-login-state .pencil-primary { grid-column: 2; justify-self: start; }
  .code-row { grid-template-columns: minmax(0, 1fr) 96px; }
  .provider-dialog { padding: 16px; }
}
</style>
