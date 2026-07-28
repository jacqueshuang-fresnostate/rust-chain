<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import {
  BadgeCheck,
  Camera,
  CheckCircle2,
  ChevronRight,
  Copy,
  Headphones,
  History,
  Languages,
  Link2,
  LockKeyhole,
  LogOut,
  Mail,
  Moon,
  Settings2,
  Sun,
  UserRound,
  Users,
  X,
} from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { apiErrorMessage } from '@/api/client'
import { fetchKycStatus, fetchUserProfile, updateUsername, uploadUserAvatar, type KycStatus, type UserProfile } from '@/api/user'
import { useModalDialog } from '@/core/modalDialog'
import { useSessionStore } from '@/stores/session'
import { useThemeStore } from '@/stores/theme'
import { normalizeMobileLocale, SUPPORTED_LOCALES } from '@/i18n'

const router = useRouter()
const session = useSessionStore()
const theme = useThemeStore()
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
  : '--')
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
const memberLevel = computed(() => profileReady.value
  ? `LEVEL ${String(profile.value?.kycLevel || 0).padStart(2, '0')}`
  : 'LEVEL --')

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

onMounted(() => { void load() })
</script>

<template>
  <main
    class="view profile-view prototype-root-view"
    :class="{ 'guest-profile': !session.isAuthenticated }"
    data-profile-workspace="live"
  >
    <template v-if="!session.isAuthenticated">
      <section class="profile-identity">
        <div class="profile-noise" aria-hidden="true" />
        <span class="avatar"><UserRound :size="26" aria-hidden="true" /></span>
        <div class="identity-copy">
          <span class="eyebrow">{{ t('rootPrototype.guestModeEyebrow') }}</span>
          <h1>{{ t('rootPrototype.guestMode') }}</h1>
          <p>{{ t('rootPrototype.guestDescription') }}</p>
        </div>
      </section>
      <div class="dual-actions profile-auth-actions">
        <button type="button" @click="router.push({ name: 'login', query: { redirect: '/profile' } })">{{ t('auth.login') }}</button>
        <button type="button" @click="router.push({ name: 'register' })">{{ t('auth.register') }}</button>
      </div>
      <section class="content-section">
        <div class="settings-list">
          <button type="button" @click="router.push({ name: 'language' })">
            <span class="settings-icon"><Languages :size="19" /></span>
            <strong>{{ t('language.entry') }}</strong>
            <small>{{ currentLanguageLabel }}</small>
            <ChevronRight :size="16" />
          </button>
          <button type="button" @click="router.push({ name: 'message-center' })">
            <span class="settings-icon"><Headphones :size="19" /></span>
            <strong>{{ t('rootPrototype.support') }}</strong>
            <small>{{ t('rootPrototype.alwaysAvailable') }}</small>
            <ChevronRight :size="16" />
          </button>
        </div>
      </section>
    </template>

    <template v-else>
      <section class="profile-identity" :aria-busy="loading">
        <div class="profile-noise" aria-hidden="true" />
        <input ref="avatarInput" class="avatar-input" type="file" accept="image/*" @change="uploadAvatar" />
        <button class="avatar" type="button" :aria-label="t('profile.updateAvatar')" :disabled="updatingAvatar || !profileReady" @click="openAvatarPicker">
          <img v-if="profile?.avatarUrl" :src="profile.avatarUrl" :alt="t('profile.updateAvatar')" />
          <span v-else-if="profileReady">{{ initials }}</span>
          <UserRound v-else :size="24" aria-hidden="true" />
          <i><Camera :size="13" /></i>
        </button>
        <div class="identity-copy">
          <span class="eyebrow">{{ t('rootPrototype.verifiedMemberEyebrow') }}</span>
          <h1>{{ displayName }}</h1>
          <button type="button" :disabled="!profileReady || !profile" @click="copyUid">
            UID {{ profile?.id || '--' }} <CheckCircle2 v-if="copied" :size="13" /><Copy v-else :size="13" />
          </button>
        </div>
        <button class="icon-button" type="button" :aria-label="t('profile.editNickname')" :disabled="!profileReady" @click="openNameEditor">
          <Settings2 :size="18" />
        </button>
        <div v-if="error" class="profile-identity-state" role="alert">
          <span>{{ error }}</span>
          <button type="button" :disabled="loading" @click="load">{{ t('common.retry') }}</button>
        </div>
      </section>

      <section class="profile-level">
        <div><span>{{ t('rootPrototype.identityLevel') }}</span><strong>{{ memberLevel }}</strong></div>
        <div class="level-track"><i :style="{ width: `${profileReady ? Math.min(100, (profile?.kycLevel || 0) * 25) : 0}%` }" /></div>
        <span>{{ t('rootPrototype.nextLevel') }} --</span>
      </section>

      <section class="profile-metrics">
        <div><strong>--</strong><span>{{ t('rootPrototype.tradingDays') }}</span></div>
        <div><strong>--</strong><span>{{ t('rootPrototype.winRate') }}</span></div>
        <div><strong>--</strong><span>{{ t('rootPrototype.profitFactor') }}</span></div>
      </section>

      <section class="content-section">
        <div class="section-heading"><div><span class="eyebrow">{{ t('rootPrototype.accountMatrix') }}</span><h2>{{ t('rootPrototype.accountAndSecurity') }}</h2></div></div>
        <div class="settings-list">
          <button type="button" @click="router.push({ name: 'kyc' })">
            <span class="settings-icon"><BadgeCheck :size="19" /></span>
            <strong>{{ t('profile.kyc') }}</strong>
            <small :class="kycTone">{{ kycSummary }}</small>
            <ChevronRight :size="16" />
          </button>
          <button type="button" @click="router.push({ name: 'security' })">
            <span class="settings-icon"><LockKeyhole :size="19" /></span>
            <strong>{{ t('profile.security') }}</strong>
            <small>
              {{ profileReady
                ? profile?.fundPasswordSet
                  ? t('profile.fundPasswordSet')
                  : t('profile.improveSecurity')
                : profileStateLabel }}
            </small>
            <ChevronRight :size="16" />
          </button>
          <button type="button" @click="router.push({ name: 'account-bindings' })">
            <span class="settings-icon"><Mail :size="19" /></span>
            <strong>{{ t('profile.bindings') }}</strong>
            <small>
              {{ profileReady
                ? profile?.emailVerified
                  ? t('profile.emailVerified')
                  : t('profile.bindAccounts')
                : profileStateLabel }}
            </small>
            <ChevronRight :size="16" />
          </button>
          <button type="button" @click="router.push({ name: 'referrals' })">
            <span class="settings-icon"><Users :size="19" /></span>
            <strong>{{ t('profile.referrals') }}</strong>
            <small>{{ t('profile.referralDescription') }}</small>
            <ChevronRight :size="16" />
          </button>
          <button type="button" @click="router.push({ name: 'language' })">
            <span class="settings-icon"><Languages :size="19" /></span>
            <strong>{{ t('language.entry') }}</strong>
            <small>{{ currentLanguageLabel }}</small>
            <ChevronRight :size="16" />
          </button>
          <button type="button" @click="router.push({ name: 'orders' })">
            <span class="settings-icon"><History :size="19" /></span>
            <strong>{{ t('orders.title') }}</strong>
            <small>{{ t('common.viewAll') }}</small>
            <ChevronRight :size="16" />
          </button>
          <button type="button" @click="router.push({ name: 'message-center' })">
            <span class="settings-icon"><Headphones :size="19" /></span>
            <strong>{{ t('rootPrototype.support') }}</strong>
            <small>{{ t('rootPrototype.alwaysAvailable') }}</small>
            <ChevronRight :size="16" />
          </button>
          <button type="button" @click="theme.toggleTheme">
            <span class="settings-icon"><Moon v-if="theme.isDark" :size="19" /><Sun v-else :size="19" /></span>
            <strong>{{ t('rootPrototype.interfaceTheme') }}</strong>
            <small>{{ t(theme.isDark ? 'rootPrototype.darkTheme' : 'rootPrototype.lightTheme') }}</small>
            <span class="toggle" :class="{ on: !theme.isDark }"><i /></span>
          </button>
        </div>
      </section>

      <button class="logout-button" type="button" @click="logout"><LogOut :size="18" />{{ t('profile.logout') }}</button>
    </template>

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
