<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import {
  BadgeCheck,
  Camera,
  CheckCircle2,
  ChevronRight,
  Copy,
  IdCard,
  Languages,
  LifeBuoy,
  Link2,
  LogOut,
  Settings,
  ShieldCheck,
  UserPlus,
  UserRound,
  X,
} from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchKycStatus, fetchUserProfile, updateUsername, uploadUserAvatar, type KycStatus, type UserProfile } from '@/api/user'
import { useModalDialog } from '@/core/modalDialog'
import { useSessionStore } from '@/stores/session'
import { normalizeMobileLocale, SUPPORTED_LOCALES } from '@/i18n'

const router = useRouter()
const session = useSessionStore()
const { locale, t } = useI18n()
const profile = ref<UserProfile | null>(null)
const kyc = ref<KycStatus | null>(null)
const loading = ref(false)
const error = ref('')
const profileReady = ref(false)
const copied = ref(false)
const editOpen = ref(false)
const nameDraft = ref('')
const updatingName = ref(false)
const updatingAvatar = ref(false)
const avatarInput = ref<HTMLInputElement | null>(null)
const profileDialog = ref<HTMLElement | null>(null)
const { trapFocus: trapProfileFocus } = useModalDialog(editOpen, profileDialog, '[autofocus]')

const displayName = computed(() => profileReady.value
  ? profile.value?.username || profile.value?.email || profile.value?.phone || t('profile.defaultUser')
  : t('profile.loading'))
const initials = computed(() => displayName.value.slice(0, 1).toUpperCase())
const profileStateLabel = computed(() => {
  if (loading.value) return t('profile.loading')
  if (error.value) return t('common.serviceUnavailable')
  return t('profile.loginDescription')
})
const currentLanguageLabel = computed(() => {
  const current = normalizeMobileLocale(locale.value) || 'zh-CN'
  const option = SUPPORTED_LOCALES.find((item) => item.code === current)
  return option ? t(option.labelKey) : current
})
const kycSummary = computed(() => {
  if (!profileReady.value) return profileStateLabel.value
  const status = kyc.value?.latestSubmission?.status
  if (status === 'approved') return t('profile.kycApproved')
  if (status === 'pending') return t('profile.kycPending')
  if (status === 'rejected') return t('profile.kycRejected')
  return t('profile.kycUnverified')
})
const kycTone = computed(() => kyc.value?.latestSubmission?.status === 'approved' ? 'positive' : kyc.value?.latestSubmission?.status === 'rejected' ? 'negative' : '')
const bindingCount = computed(() => {
  if (!profileReady.value || !profile.value) return 0
  return Number(profile.value.emailVerified) + Number(Boolean(profile.value.phone)) + Number(profile.value.fundPasswordSet)
})

async function load(): Promise<void> {
  if (!session.isAuthenticated) {
    profile.value = null
    kyc.value = null
    profileReady.value = false
    loading.value = false
    error.value = ''
    return
  }
  loading.value = true
  profileReady.value = false
  error.value = ''
  try {
    const [nextProfile, nextKyc] = await Promise.all([fetchUserProfile(), fetchKycStatus()])
    profile.value = nextProfile
    kyc.value = nextKyc
    profileReady.value = true
    nameDraft.value = nextProfile.username || ''
  } catch (reason) {
    profile.value = null
    kyc.value = null
    error.value = apiErrorMessage(reason, t('profile.loadFailed'))
  } finally {
    loading.value = false
  }
}

function openNameEditor(): void {
  if (!profileReady.value) return
  nameDraft.value = profile.value?.username || ''
  editOpen.value = true
}

function closeNameEditor(): void {
  if (updatingName.value) return
  editOpen.value = false
}

function handleProfileDialogKeydown(event: KeyboardEvent): void {
  trapProfileFocus(event, closeNameEditor)
}

async function saveName(): Promise<void> {
  if (!nameDraft.value.trim()) return
  updatingName.value = true
  try {
    const username = await updateUsername(nameDraft.value)
    if (profile.value) profile.value = { ...profile.value, username }
    editOpen.value = false
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('profile.nicknameFailed'))
  } finally {
    updatingName.value = false
  }
}

function openAvatarPicker(): void {
  avatarInput.value?.click()
}

async function uploadAvatar(event: Event): Promise<void> {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  input.value = ''
  if (!file) return
  if (!file.type.startsWith('image/')) {
    error.value = t('profile.invalidImage')
    return
  }
  if (file.size > 5 * 1024 * 1024) {
    error.value = t('profile.imageTooLarge')
    return
  }
  updatingAvatar.value = true
  error.value = ''
  try {
    const avatarUrl = await uploadUserAvatar(file)
    if (profile.value && avatarUrl) profile.value = { ...profile.value, avatarUrl }
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('profile.avatarFailed'))
  } finally {
    updatingAvatar.value = false
  }
}

async function copyUid(): Promise<void> {
  if (!profile.value) return
  try {
    await navigator.clipboard.writeText(String(profile.value.id))
  } catch {
    const field = document.createElement('textarea')
    field.value = String(profile.value.id)
    document.body.appendChild(field)
    field.select()
    document.execCommand('copy')
    field.remove()
  }
  copied.value = true
  window.setTimeout(() => { copied.value = false }, 1_600)
}

function logout(): void {
  session.logout()
  void router.replace('/')
}

function openSettings(): void {
  void router.push({ name: session.isAuthenticated ? 'security' : 'language' })
}

onMounted(() => { void load() })
</script>

<template>
  <main
    class="page pencil-page pencil-root-page profile-pencil"
    :class="session.isAuthenticated ? 'profile-pencil--member' : 'profile-pencil--guest'"
    data-profile-workspace="live"
    data-pencil-source="dUqOS duJTW S23rM S0Bj8"
  >
    <PageHeader :back="false" :pencil="true" :title="t('profile.title')">
      <template #actions>
        <button
          class="icon-button"
          type="button"
          :aria-label="session.isAuthenticated ? t('profile.securityCenter') : t('language.entry')"
          @click="openSettings"
        >
          <Settings :size="20" aria-hidden="true" />
        </button>
      </template>
    </PageHeader>

    <div class="pencil-content profile-pencil__content">
      <section class="profile-identity-pencil" :aria-busy="loading">
        <input ref="avatarInput" class="avatar-input" type="file" accept="image/*" @change="uploadAvatar" />
        <button
          v-if="session.isAuthenticated"
          class="profile-avatar profile-avatar--button"
          type="button"
          :aria-label="t('profile.updateAvatar')"
          :disabled="updatingAvatar || !profileReady"
          @click="openAvatarPicker"
        >
          <img v-if="profile?.avatarUrl" :src="profile.avatarUrl" :alt="t('profile.updateAvatar')" />
          <span v-else-if="profileReady">{{ initials }}</span>
          <UserRound v-else :size="23" aria-hidden="true" />
          <i><Camera :size="12" /></i>
        </button>
        <span v-else class="profile-avatar"><UserRound :size="23" aria-hidden="true" /></span>

        <div class="profile-identity-pencil__copy">
          <template v-if="session.isAuthenticated">
            <strong>{{ displayName }}</strong>
            <button v-if="profileReady && profile" type="button" @click="copyUid">
              UID {{ profile.id }}
              <CheckCircle2 v-if="copied" :size="13" />
              <Copy v-else :size="13" />
            </button>
            <small v-else>{{ profileStateLabel }}</small>
          </template>
          <template v-else>
            <strong>{{ t('profile.loginAccount') }}</strong>
            <small>{{ t('profile.guestSubtitle') }}</small>
          </template>
        </div>
      </section>

      <div v-if="!session.isAuthenticated" class="profile-auth-actions">
        <button class="pencil-primary" type="button" @click="router.push({ name: 'login', query: { redirect: '/profile' } })">
          {{ t('auth.login') }}
        </button>
        <button class="profile-register-action" type="button" @click="router.push({ name: 'register', query: { redirect: '/profile' } })">
          {{ t('auth.register') }}
        </button>
      </div>
      <div v-else class="profile-status-row" aria-live="polite">
        <span class="pencil-pill" :class="{ 'pencil-pill--negative': kycTone === 'negative' }">
          <BadgeCheck :size="14" />{{ kycSummary }}
        </span>
        <span class="pencil-pill"><Link2 :size="13" />{{ t('profile.bindingProgress', { current: bindingCount, total: 3 }) }}</span>
      </div>

      <div v-if="error" class="pencil-message pencil-message--error" role="alert">
        <span>{{ error }}</span>
        <button class="pencil-secondary" type="button" :disabled="loading" @click="load">{{ t('common.retry') }}</button>
      </div>

      <section class="profile-group">
        <h2 class="profile-group__heading">{{ t('profile.identitySecurityGroup') }}</h2>
        <div class="pencil-list">
          <button class="pencil-row" type="button" @click="router.push({ name: 'kyc' })">
            <span class="pencil-row__icon"><IdCard :size="18" /></span>
            <span class="pencil-row__copy"><strong>{{ t('profile.kyc') }}</strong></span>
            <span class="pencil-row__value"><small v-if="session.isAuthenticated" :class="kycTone">{{ kycSummary }}</small><ChevronRight :size="16" /></span>
          </button>
          <button class="pencil-row" type="button" @click="router.push({ name: 'security' })">
            <span class="pencil-row__icon"><ShieldCheck :size="18" /></span>
            <span class="pencil-row__copy"><strong>{{ t('profile.securityCenter') }}</strong></span>
            <span class="pencil-row__value"><ChevronRight :size="16" /></span>
          </button>
          <button class="pencil-row" type="button" @click="router.push({ name: 'account-bindings' })">
            <span class="pencil-row__icon"><Link2 :size="18" /></span>
            <span class="pencil-row__copy"><strong>{{ t('profile.accountBindings') }}</strong></span>
            <span class="pencil-row__value"><small v-if="session.isAuthenticated">{{ bindingCount }}/3</small><ChevronRight :size="16" /></span>
          </button>
        </div>
      </section>

      <section class="profile-group profile-group--support">
        <h2 class="profile-group__heading">{{ t('profile.preferencesSupportGroup') }}</h2>
        <div class="pencil-list">
          <button class="pencil-row" type="button" @click="router.push({ name: 'referrals' })">
            <span class="pencil-row__icon"><UserPlus :size="18" /></span>
            <span class="pencil-row__copy"><strong>{{ t('profile.referrals') }}</strong><small>{{ t('profile.referralDescription') }}</small></span>
            <span class="pencil-row__value"><ChevronRight :size="16" /></span>
          </button>
          <button class="pencil-row" type="button" @click="router.push({ name: 'language' })">
            <span class="pencil-row__icon"><Languages :size="18" /></span>
            <span class="pencil-row__copy"><strong>{{ t('language.entry') }}</strong></span>
            <span class="pencil-row__value"><small v-if="session.isAuthenticated">{{ currentLanguageLabel }}</small><ChevronRight :size="16" /></span>
          </button>
          <button class="pencil-row" type="button" @click="router.push({ name: 'help-support' })">
            <span class="pencil-row__icon"><LifeBuoy :size="18" /></span>
            <span class="pencil-row__copy"><strong>{{ t('profile.helpSupport') }}</strong></span>
            <span class="pencil-row__value"><ChevronRight :size="16" /></span>
          </button>
        </div>
      </section>

      <button v-if="session.isAuthenticated" class="profile-logout" type="button" @click="logout">
        <LogOut :size="17" />{{ t('profile.logout') }}
      </button>
    </div>

    <div v-if="editOpen" class="confirmation-layer">
      <button class="confirmation-overlay-dismiss" type="button" :aria-label="t('common.close')" :disabled="updatingName" tabindex="-1" @click="closeNameEditor" />
      <section
        ref="profileDialog"
        class="confirmation-sheet"
        role="dialog"
        aria-modal="true"
        :aria-busy="updatingName"
        :aria-label="t('profile.editNicknameTitle')"
        tabindex="-1"
        @keydown="handleProfileDialogKeydown"
      >
        <header>
          <span class="confirmation-icon"><Link2 :size="20" /></span>
          <div><span>{{ t('profile.editNickname') }}</span><h2>{{ t('profile.editNicknameTitle') }}</h2></div>
        </header>
        <label class="field">
          <span>{{ t('profile.nicknamePlaceholder') }}</span>
          <div><input v-model="nameDraft" autofocus :placeholder="t('profile.nicknamePlaceholder')" /></div>
        </label>
        <div class="confirmation-actions">
          <button data-dialog-cancel type="button" :disabled="updatingName" @click="closeNameEditor"><X :size="16" />{{ t('common.cancel') }}</button>
          <button class="confirmation-primary" type="button" :disabled="updatingName || !nameDraft.trim()" @click="saveName">
            {{ updatingName ? t('common.saving') : t('common.save') }}
          </button>
        </div>
      </section>
    </div>
  </main>
</template>

<style scoped>
.profile-pencil__content {
  display: grid;
  gap: 10px;
  padding-bottom: 0;
  padding-top: 10px;
}

.profile-identity-pencil {
  align-items: center;
  display: grid;
  gap: 14px;
  grid-template-columns: 56px minmax(0, 1fr);
  height: 72px;
  min-height: 72px;
  padding: 12px 0 4px;
}

.avatar-input { display: none; }
.profile-avatar { align-items: center; background: var(--ink); border: 0; border-radius: 50%; color: var(--surface); display: inline-flex; height: 56px; justify-content: center; overflow: hidden; padding: 0; position: relative; width: 56px; }
.profile-avatar--button { background: var(--accent); color: var(--on-accent); }
.profile-avatar img { height: 100%; object-fit: cover; width: 100%; }
.profile-avatar i { display: none; }
.profile-identity-pencil__copy { display: grid; gap: 5px; min-width: 0; }
.profile-identity-pencil__copy strong { font-size: 17px; font-weight: 750; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.profile-identity-pencil__copy small { color: var(--muted); font-size: 11px; line-height: 1.4; }
.profile-identity-pencil__copy button { align-items: center; background: transparent; color: var(--muted); display: inline-flex; font-family: var(--font-geist-mono), var(--data-font); font-size: 10px; gap: 5px; justify-self: start; min-height: 24px; padding: 0; }
.profile-auth-actions { display: grid; gap: 10px; grid-template-columns: 1fr 1fr; height: 58px; min-height: 58px; padding: 8px 0 4px; }
.profile-auth-actions > button { height: 46px; min-height: 46px; }
.profile-register-action { background: var(--ink); border-radius: 999px; color: var(--surface); font-size: 14px; font-weight: 700; }
.profile-status-row { align-items: center; display: flex; gap: 8px; height: 44px; min-height: 44px; overflow-x: auto; padding: 8px 0 4px; }
.profile-status-row .pencil-pill { height: 32px; min-height: 32px; padding-inline: 12px; }
.profile-group {
  box-sizing: border-box;
  display: grid;
  gap: 6px;
  grid-template-rows: 23px 156px;
  height: 201px;
  margin: 0;
  min-height: 201px;
  padding: 12px 0 4px;
}

.profile-group--support {
  grid-template-rows: 23px 156px;
  height: 195px;
  min-height: 195px;
  padding: 10px 0 0;
}

.profile-group__heading {
  color: var(--muted);
  font-size: 11px;
  font-weight: 600;
  line-height: 23px;
  margin: 0;
}

.profile-group .pencil-list { grid-template-rows: repeat(3, 52px); }
.profile-group--support .pencil-list { grid-template-rows: repeat(3, 52px); }
.profile-group .pencil-row { height: 52px; min-height: 52px; }
.profile-group .pencil-row__value { align-items: center; display: flex; gap: 4px; max-width: 112px; }
.profile-group .pencil-row__value small { max-width: 84px; }
.profile-logout { align-items: center; background: transparent; border: 1px solid var(--negative); border-radius: 999px; color: var(--negative); display: flex; font-size: 13px; font-weight: 650; gap: 6px; height: 44px; justify-content: center; margin: 0; min-height: 44px; width: 100%; }
@media (max-width: 340px) {
  .profile-group .pencil-row__value { max-width: 88px; }
  .profile-group .pencil-row__value small { max-width: 62px; }
}
</style>
