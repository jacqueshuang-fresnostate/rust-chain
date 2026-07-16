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
  <main class="page profile-page">
    <PageHeader :title="t('profile.title')" :back="false"><template #actions><button class="icon-button" type="button" :aria-label="t('language.title')" @click="router.push({ name: 'language' })"><Languages :size="20" /></button><button v-if="session.isAuthenticated" class="icon-button" type="button" :aria-label="t('profile.refresh')" :disabled="loading" @click="load"><RefreshCw :size="21" :class="{ spin: loading }" /></button></template></PageHeader>
    <div class="page-content">
      <LoginRequiredState v-if="!session.isAuthenticated" :description="t('profile.loginDescription')" />
      <template v-else>
        <p v-if="error" class="error-message">{{ error }}</p>
        <section v-if="profile" class="profile-summary"><input ref="avatarInput" class="avatar-input" type="file" accept="image/*" @change="uploadAvatar" /><button class="avatar-button" type="button" :aria-label="t('profile.updateAvatar')" :disabled="updatingAvatar" @click="openAvatarPicker"><img v-if="profile.avatarUrl" :src="profile.avatarUrl" :alt="t('profile.updateAvatar')" /><span v-else>{{ initials }}</span><i><Camera :size="13" /></i></button><div><strong>{{ displayName }}</strong><small>{{ profile.email || profile.phone || t('profile.userNumber', { id: profile.id }) }}</small><em>{{ t('profile.registeredAt', { time: formatDateTime(profile.createdAt) }) }}</em></div><button class="icon-button" type="button" :aria-label="t('profile.editNickname')" @click="openNameEditor"><Pencil :size="19" /></button></section>
        <p v-else-if="loading" class="empty-state">{{ t('profile.loading') }}</p>

        <section class="profile-menu">
          <button type="button" @click="router.push({ name: 'kyc' })"><span class="profile-menu__icon profile-menu__icon--green"><BadgeCheck :size="20" /></span><span><b>{{ t('profile.kyc') }}</b><small :class="kycTone">{{ kycSummary }}</small></span><ChevronRight :size="19" /></button>
          <button type="button" @click="router.push({ name: 'security' })"><span class="profile-menu__icon profile-menu__icon--blue"><ShieldCheck :size="20" /></span><span><b>{{ t('profile.security') }}</b><small>{{ profile?.fundPasswordSet ? t('profile.fundPasswordSet') : t('profile.improveSecurity') }}</small></span><ChevronRight :size="19" /></button>
          <button type="button" @click="router.push({ name: 'account-bindings' })"><span class="profile-menu__icon profile-menu__icon--purple"><Link2 :size="20" /></span><span><b>{{ t('profile.bindings') }}</b><small>{{ profile?.emailVerified ? t('profile.emailVerified') : t('profile.bindAccounts') }}</small></span><ChevronRight :size="19" /></button>
          <button type="button" @click="router.push({ name: 'referrals' })"><span class="profile-menu__icon profile-menu__icon--orange"><UsersRound :size="20" /></span><span><b>{{ t('profile.referrals') }}</b><small>{{ t('profile.referralDescription') }}</small></span><ChevronRight :size="19" /></button>
        </section>
        <button class="logout-button" type="button" @click="logout"><LogOut :size="18" />{{ t('profile.logout') }}</button>
      </template>
      <section class="profile-preferences">
        <button type="button" @click="router.push({ name: 'language' })"><span><Languages :size="20" /></span><span><b>{{ t('language.entry') }}</b><small>{{ currentLanguageLabel }}</small></span><ChevronRight :size="19" /></button>
      </section>
    </div>

    <div v-if="editOpen" class="profile-dialog-mask" @click.self="editOpen = false"><form class="profile-dialog" @submit.prevent="saveName"><h2>{{ t('profile.editNicknameTitle') }}</h2><input v-model="nameDraft" class="input" maxlength="48" :placeholder="t('profile.nicknamePlaceholder')" autofocus /><div><button class="button button--secondary" type="button" @click="editOpen = false">{{ t('common.cancel') }}</button><button class="button button--primary" type="submit" :disabled="updatingName">{{ updatingName ? t('common.saving') : t('common.save') }}</button></div></form></div>
  </main>
</template>

<style scoped>
.profile-page { background: var(--background); }.profile-page .page-content { background: var(--surface); min-height: calc(100dvh - 56px); padding-bottom: 112px; }.profile-summary { align-items: center; display: grid; gap: 13px; grid-template-columns: 52px minmax(0, 1fr) 44px; padding: 20px 0 24px; }.avatar-input { display: none; }.avatar-button { background: transparent; border-radius: 50%; height: 52px; overflow: visible; padding: 0; position: relative; width: 52px; }.avatar-button img,.avatar-button > span { background: var(--soft); border-radius: 50%; display: block; height: 52px; object-fit: cover; width: 52px; }.avatar-button > span { align-items: center; background: var(--ink); color: white; display: inline-flex; font-size: 21px; font-weight: 760; justify-content: center; }.avatar-button i { align-items: center; background: var(--accent); border: 2px solid white; border-radius: 50%; bottom: -2px; color: white; display: inline-flex; height: 21px; justify-content: center; position: absolute; right: -2px; width: 21px; }.profile-summary div { display: grid; gap: 4px; min-width: 0; }.profile-summary strong { font-size: 20px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }.profile-summary small,.profile-summary em { color: var(--muted); font-size: 12px; font-style: normal; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }.profile-menu { border-top: 1px solid var(--line); display: grid; }.profile-menu button,.profile-preferences button { align-items: center; background: transparent; border-bottom: 1px solid var(--line); display: grid; gap: 12px; grid-template-columns: 42px minmax(0, 1fr) auto; min-height: 78px; padding: 10px 0; text-align: left; width: 100%; }.profile-menu__icon,.profile-preferences button > span:first-child { align-items: center; border-radius: var(--radius); display: inline-flex; height: 40px; justify-content: center; width: 40px; }.profile-menu__icon--green { background: var(--positive-soft); color: var(--positive); }.profile-menu__icon--blue { background: #eaf1ff; color: #3975ca; }.profile-menu__icon--purple { background: #f1ebff; color: #7759c9; }.profile-menu__icon--orange { background: #fff0dc; color: #bb6b12; }.profile-menu button > span:nth-child(2),.profile-preferences button > span:nth-child(2) { display: grid; gap: 4px; }.profile-menu b,.profile-preferences b { font-size: 15px; }.profile-menu small,.profile-preferences small { color: var(--muted); font-size: 12px; }.profile-preferences { border-top: 1px solid var(--line); margin-top: 18px; }.profile-preferences button > span:first-child { background: #edf1f3; color: var(--muted-strong); }.logout-button { align-items: center; background: transparent; color: var(--negative); display: flex; font-size: 14px; gap: 8px; margin: 22px auto 0; padding: 9px; }.profile-dialog-mask { align-items: flex-end; background: rgb(15 23 42 / 42%); display: flex; inset: 0; justify-content: center; padding: 16px 16px calc(16px + env(safe-area-inset-bottom)); position: fixed; z-index: 60; }.profile-dialog { background: white; border-radius: var(--radius); display: grid; gap: 16px; max-height: calc(100dvh - 32px - env(safe-area-inset-top)); max-width: 520px; overflow-y: auto; padding: 18px; width: 100%; }.profile-dialog h2 { font-size: 19px; margin: 0; }.profile-dialog > div { display: flex; gap: 10px; justify-content: flex-end; }.profile-dialog .button { min-height: 40px; }.spin { animation: spin .8s linear infinite; }@keyframes spin { to { transform: rotate(360deg); } }
</style>
