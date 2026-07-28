<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { BadgeCheck, Camera, ChevronRight, Languages, Link2, LogOut, Pencil, RefreshCw, ShieldCheck, UsersRound } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import LoginRequiredState from '@/components/LoginRequiredState.vue'
import PageHeader from '@/components/PageHeader.vue'
import { apiErrorMessage } from '@/api/client'
import { fetchKycStatus, fetchUserProfile, updateUsername, uploadUserAvatar, type KycStatus, type UserProfile } from '@/api/user'
import { formatDateTime } from '@/core/format'
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
const editOpen = ref(false)
const nameDraft = ref('')
const updatingName = ref(false)
const updatingAvatar = ref(false)
const avatarInput = ref<HTMLInputElement | null>(null)
const profileDialog = ref<HTMLElement | null>(null)
const { trapFocus: trapProfileFocus } = useModalDialog(editOpen, profileDialog, '[autofocus]')

const displayName = computed(() => profile.value?.username || profile.value?.email || profile.value?.phone || t('profile.defaultUser'))
const initials = computed(() => displayName.value.slice(0, 1).toUpperCase())
const currentLanguageLabel = computed(() => {
  const current = normalizeMobileLocale(locale.value) || 'zh-CN'
  const option = SUPPORTED_LOCALES.find((item) => item.code === current)
  return option ? t(option.labelKey) : current
})
const kycSummary = computed(() => {
  const status = kyc.value?.latestSubmission?.status
  if (status === 'approved') return t('profile.kycApproved')
  if (status === 'pending') return t('profile.kycPending')
  if (status === 'rejected') return t('profile.kycRejected')
  return t('profile.kycUnverified')
})
const kycTone = computed(() => kyc.value?.latestSubmission?.status === 'approved' ? 'up' : kyc.value?.latestSubmission?.status === 'rejected' ? 'down' : '')

async function load(): Promise<void> {
  if (!session.isAuthenticated) return
  loading.value = true
  error.value = ''
  try {
    const [nextProfile, nextKyc] = await Promise.all([fetchUserProfile(), fetchKycStatus()])
    profile.value = nextProfile
    kyc.value = nextKyc
    nameDraft.value = nextProfile.username || ''
  } catch (reason) {
    error.value = apiErrorMessage(reason, t('profile.loadFailed'))
  } finally {
    loading.value = false
  }
}

function openNameEditor(): void {
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

function logout(): void {
  session.logout()
  void router.replace('/')
}

onMounted(() => { void load() })
</script>

<template>
  <main class="page page--prototype-grid profile-page" data-profile-workspace="live">
    <PageHeader :title="t('profile.title')" :back="false">
      <template #actions>
        <button class="icon-button" type="button" :aria-label="t('language.title')" @click="router.push({ name: 'language' })">
          <Languages :size="20" />
        </button>
        <button v-if="session.isAuthenticated" class="icon-button" type="button" :aria-label="t('profile.refresh')" :disabled="loading" @click="load">
          <RefreshCw :size="21" :class="{ spin: loading }" />
        </button>
      </template>
    </PageHeader>
    <div class="page-content profile-content">
      <LoginRequiredState v-if="!session.isAuthenticated" :description="t('profile.loginDescription')" />
      <p v-if="error" class="error-message" role="alert">{{ error }}</p>
      <section v-if="profile" class="profile-summary">
          <input ref="avatarInput" class="avatar-input" type="file" accept="image/*" @change="uploadAvatar" />
          <button class="avatar-button" type="button" :aria-label="t('profile.updateAvatar')" :disabled="updatingAvatar" @click="openAvatarPicker">
            <img v-if="profile.avatarUrl" :src="profile.avatarUrl" :alt="t('profile.updateAvatar')" />
            <span v-else>{{ initials }}</span>
            <i><Camera :size="13" /></i>
          </button>
          <div class="profile-summary__identity">
            <strong>{{ displayName }}</strong>
            <small>{{ profile.email || profile.phone || t('profile.userNumber', { id: profile.id }) }}</small>
            <em>{{ t('profile.registeredAt', { time: formatDateTime(profile.createdAt) }) }}</em>
          </div>
          <button class="icon-button profile-summary__edit" type="button" :aria-label="t('profile.editNickname')" @click="openNameEditor">
            <Pencil :size="19" />
          </button>
          <div class="profile-status">
            <span :class="kycTone"><BadgeCheck :size="15" />{{ kycSummary }}</span>
            <span :class="{ up: profile.fundPasswordSet }"><ShieldCheck :size="15" />{{ profile.fundPasswordSet ? t('profile.fundPasswordSet') : t('profile.improveSecurity') }}</span>
          </div>
          <div class="profile-metrics">
            <span><small>{{ t('profile.kyc') }}</small><strong :class="kycTone">{{ kycSummary }}</strong></span>
            <span><small>{{ t('profile.security') }}</small><strong>{{ profile.fundPasswordSet ? t('profile.fundPasswordSet') : t('profile.improveSecurity') }}</strong></span>
            <span><small>{{ t('language.entry') }}</small><strong>{{ currentLanguageLabel }}</strong></span>
          </div>
      </section>
      <section v-else-if="!session.isAuthenticated" class="profile-summary profile-summary--guest">
        <span class="avatar-button" aria-hidden="true">
          <span>{{ initials }}</span>
        </span>
        <div class="profile-summary__identity">
          <strong>{{ displayName }}</strong>
          <small>{{ t('profile.loginDescription') }}</small>
        </div>
        <div class="profile-status">
          <span><BadgeCheck :size="15" />{{ t('profile.kycUnverified') }}</span>
          <span><ShieldCheck :size="15" />{{ t('profile.improveSecurity') }}</span>
        </div>
        <div class="profile-metrics">
          <span><small>{{ t('profile.kyc') }}</small><strong>{{ t('profile.kycUnverified') }}</strong></span>
          <span><small>{{ t('profile.security') }}</small><strong>{{ t('profile.improveSecurity') }}</strong></span>
          <span><small>{{ t('language.entry') }}</small><strong>{{ currentLanguageLabel }}</strong></span>
        </div>
      </section>
      <p v-else-if="loading" class="empty-state">{{ t('profile.loading') }}</p>

      <section class="profile-menu" :aria-label="t('profile.title')">
          <button type="button" @click="router.push({ name: 'kyc' })">
            <span class="profile-menu__icon profile-menu__icon--positive"><BadgeCheck :size="20" /></span>
            <span><b>{{ t('profile.kyc') }}</b><small :class="kycTone">{{ kycSummary }}</small></span>
            <ChevronRight :size="19" />
          </button>
          <button type="button" @click="router.push({ name: 'security' })">
            <span class="profile-menu__icon profile-menu__icon--focus"><ShieldCheck :size="20" /></span>
            <span><b>{{ t('profile.security') }}</b><small>{{ profile?.fundPasswordSet ? t('profile.fundPasswordSet') : t('profile.improveSecurity') }}</small></span>
            <ChevronRight :size="19" />
          </button>
          <button type="button" @click="router.push({ name: 'account-bindings' })">
            <span class="profile-menu__icon profile-menu__icon--accent"><Link2 :size="20" /></span>
            <span><b>{{ t('profile.bindings') }}</b><small>{{ profile?.emailVerified ? t('profile.emailVerified') : t('profile.bindAccounts') }}</small></span>
            <ChevronRight :size="19" />
          </button>
          <button type="button" @click="router.push({ name: 'referrals' })">
            <span class="profile-menu__icon"><UsersRound :size="20" /></span>
            <span><b>{{ t('profile.referrals') }}</b><small>{{ t('profile.referralDescription') }}</small></span>
            <ChevronRight :size="19" />
          </button>
      </section>
      <button v-if="session.isAuthenticated" class="logout-button" type="button" @click="logout">
        <LogOut :size="18" />{{ t('profile.logout') }}
      </button>

      <section class="profile-preferences">
        <button type="button" @click="router.push({ name: 'language' })">
          <span><Languages :size="20" /></span>
          <span><b>{{ t('language.entry') }}</b><small>{{ currentLanguageLabel }}</small></span>
          <ChevronRight :size="19" />
        </button>
      </section>
    </div>

    <div v-if="editOpen" class="profile-dialog-mask" @click.self="closeNameEditor">
      <form
        ref="profileDialog"
        class="profile-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="profile-dialog-title"
        @keydown="handleProfileDialogKeydown"
        @submit.prevent="saveName"
      >
        <h2 id="profile-dialog-title">{{ t('profile.editNicknameTitle') }}</h2>
        <input v-model="nameDraft" class="input" maxlength="48" :placeholder="t('profile.nicknamePlaceholder')" autofocus />
        <div>
          <button class="button button--secondary" type="button" :disabled="updatingName" @click="closeNameEditor">{{ t('common.cancel') }}</button>
          <button class="button button--primary" type="submit" :disabled="updatingName">{{ updatingName ? t('common.saving') : t('common.save') }}</button>
        </div>
      </form>
    </div>
  </main>
</template>

<style scoped>
.profile-page { background-color: var(--surface); }
.profile-content { min-height: calc(100dvh - 72px); padding-bottom: calc(112px + env(safe-area-inset-bottom)); }
.profile-content > :deep(.login-required) { margin: 12px -16px 0; }
.profile-summary {
  align-items: center;
  background:
    linear-gradient(var(--grid-line) 1px, transparent 1px),
    linear-gradient(90deg, var(--grid-line) 1px, transparent 1px),
    var(--surface-elevated);
  background-size: 36px 36px;
  border-bottom: 1px solid var(--line-strong);
  border-top: 3px solid var(--signal-green);
  display: grid;
  gap: 13px;
  grid-template-columns: 66px minmax(0, 1fr) 44px;
  margin: 0 -16px;
  min-height: 222px;
  padding: 26px 16px 0;
  position: relative;
}
.avatar-input { display: none; }
.avatar-button { background: transparent; border-radius: 50%; height: 64px; overflow: visible; padding: 0; position: relative; width: 64px; }
.avatar-button img,
.avatar-button > span { background: var(--soft); border: 1px solid var(--line-strong); border-radius: 50%; display: block; height: 64px; object-fit: cover; width: 64px; }
.avatar-button > span { align-items: center; background: var(--signal-green); color: var(--on-positive); display: inline-flex; font-size: 23px; font-weight: 780; justify-content: center; }
.avatar-button i { align-items: center; background: var(--surface); border: 1px solid var(--line-strong); border-radius: 50%; bottom: -2px; color: var(--ink); display: inline-flex; height: 24px; justify-content: center; position: absolute; right: -2px; width: 24px; }
.profile-summary__identity { display: grid; gap: 4px; min-width: 0; }
.profile-summary strong { font-size: 24px; letter-spacing: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.profile-summary small,
.profile-summary em { color: var(--muted); font-size: 12px; font-style: normal; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.profile-summary__edit { justify-self: end; }
.profile-summary--guest { grid-template-columns: 66px minmax(0, 1fr); }
.profile-status { display: flex; flex-wrap: wrap; gap: 7px; grid-column: 1 / -1; margin-top: 2px; }
.profile-status span { align-items: center; background: var(--soft); border: 1px solid var(--line); border-radius: 2px; color: var(--muted-strong); display: inline-flex; font-size: 11px; font-weight: 700; gap: 5px; min-height: 30px; padding: 4px 9px; }
.profile-status span.up { background: var(--positive-soft); border-color: color-mix(in srgb, var(--positive) 30%, var(--line)); color: var(--positive); }
.profile-status span.down { background: var(--negative-soft); border-color: color-mix(in srgb, var(--negative) 30%, var(--line)); color: var(--negative); }
.profile-metrics { align-self: end; border-top: 1px solid var(--line); display: grid; grid-column: 1 / -1; grid-template-columns: repeat(3, minmax(0, 1fr)); margin: 2px -16px 0; width: calc(100% + 32px); }
.profile-metrics > span { display: grid; gap: 4px; min-height: 66px; min-width: 0; padding: 10px 12px; }
.profile-metrics > span + span { border-left: 1px solid var(--line); }
.profile-metrics > span:nth-child(1) { border-top: 3px solid var(--signal-green); }
.profile-metrics > span:nth-child(2) { border-top: 3px solid var(--signal-coral); }
.profile-metrics > span:nth-child(3) { border-top: 3px solid var(--signal-blue); }
.profile-metrics small { color: var(--muted); font-size: 9px; }
.profile-metrics strong { font-family: var(--data-font); font-size: 11px; line-height: 1.3; white-space: normal; }
.profile-menu { background: var(--surface); border-left: 1px solid var(--line); border-top: 1px solid var(--line); display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); margin-top: 18px; }
.profile-menu button,
.profile-preferences button { align-items: center; background: transparent; border-bottom: 1px solid var(--line); display: grid; gap: 12px; grid-template-columns: 44px minmax(0, 1fr) auto; min-height: 76px; padding: 10px 0; text-align: left; width: 100%; }
.profile-menu button { border-right: 1px solid var(--line); grid-template-columns: 40px minmax(0, 1fr); min-height: 104px; padding: 12px 10px; }
.profile-menu button > svg { display: none; }
.profile-menu__icon,
.profile-preferences button > span:first-child { align-items: center; background: var(--soft); border: 1px solid var(--line); border-radius: var(--radius); color: var(--muted-strong); display: inline-flex; height: 42px; justify-content: center; width: 42px; }
.profile-menu__icon--positive { background: var(--positive-soft); border-color: color-mix(in srgb, var(--positive) 24%, var(--line)); color: var(--positive); }
.profile-menu__icon--focus { background: color-mix(in srgb, var(--focus) 12%, var(--surface)); border-color: color-mix(in srgb, var(--focus) 24%, var(--line)); color: var(--focus); }
.profile-menu__icon--accent { background: var(--accent-soft); border-color: color-mix(in srgb, var(--accent) 24%, var(--line)); color: var(--accent); }
.profile-menu button > span:nth-child(2),
.profile-preferences button > span:nth-child(2) { display: grid; gap: 4px; min-width: 0; }
.profile-menu b,
.profile-preferences b { font-size: 15px; }
.profile-menu small,
.profile-preferences small { color: var(--muted); font-size: 12px; line-height: 1.35; }
.profile-menu button > svg,
.profile-preferences button > svg { color: var(--muted); }
.profile-preferences { border-top: 1px solid var(--line); margin-top: 20px; }
.logout-button { align-items: center; background: var(--negative-soft); border: 1px solid color-mix(in srgb, var(--negative) 28%, var(--line)); border-radius: 0; color: var(--negative); display: flex; font-size: 14px; font-weight: 720; gap: 8px; justify-content: center; margin-top: 20px; min-height: 48px; width: 100%; }
.profile-dialog-mask { align-items: flex-end; background: var(--overlay); display: flex; inset: 0; justify-content: center; padding: 16px 16px calc(16px + env(safe-area-inset-bottom)); position: fixed; z-index: var(--layer-overlay); }
.profile-dialog { background: var(--surface-elevated); border: 1px solid var(--line); border-radius: 8px; box-shadow: var(--shadow-soft); display: grid; gap: 16px; max-height: calc(100dvh - 32px - env(safe-area-inset-top)); max-width: 448px; overflow-y: auto; padding: 20px; width: 100%; }
.profile-dialog h2 { font-size: 20px; margin: 0; }
.profile-dialog > div { display: grid; gap: 10px; grid-template-columns: 1fr 1fr; }
.profile-dialog .button { min-height: 46px; }
.spin { animation: spin .8s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }
@media (max-width: 360px) {
  .profile-content > :deep(.login-required),
  .profile-summary { margin-left: -12px; margin-right: -12px; }
  .profile-summary { padding-left: 12px; padding-right: 12px; }
  .profile-metrics { margin-left: -12px; margin-right: -12px; width: calc(100% + 24px); }
}
@media (max-width: 340px) {
  .profile-content { padding-left: 12px; padding-right: 12px; }
  .profile-content > :deep(.login-required),
  .profile-summary { margin-left: -12px; margin-right: -12px; }
  .profile-summary { gap: 10px; grid-template-columns: 58px minmax(0, 1fr) 44px; padding-left: 12px; padding-right: 12px; }
  .avatar-button,
  .avatar-button img,
  .avatar-button > span { height: 56px; width: 56px; }
  .profile-metrics { margin-left: -12px; margin-right: -12px; width: calc(100% + 24px); }
  .profile-metrics > span { padding-inline: 8px; }
  .profile-menu button,
  .profile-preferences button { gap: 9px; grid-template-columns: 40px minmax(0, 1fr) auto; }
  .profile-menu button { grid-template-columns: 36px minmax(0, 1fr); padding-inline: 8px; }
  .profile-menu__icon,
  .profile-preferences button > span:first-child { height: 38px; width: 38px; }
}
</style>
