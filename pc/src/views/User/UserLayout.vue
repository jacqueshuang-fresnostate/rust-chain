<template>
  <div class="max-w-7xl mx-auto p-4 md:p-8 flex flex-col md:flex-row gap-8">
    <!-- Sidebar -->
    <div class="w-full md:w-64 flex flex-col gap-2">
      <div class="mb-4 rounded-xl border border-border bg-card/70 p-4">
        <div class="flex items-center gap-3">
          <button
            type="button"
            class="group relative flex h-16 w-16 shrink-0 items-center justify-center overflow-hidden rounded-full border border-border bg-muted text-xl font-bold text-foreground transition-colors hover:border-primary/60 focus:outline-none focus-visible:ring-2 focus-visible:ring-primary/50 disabled:cursor-not-allowed disabled:opacity-70"
            :aria-label="isLoggedIn ? t('common.upload_avatar') : t('common.login_now')"
            :disabled="avatarUploading"
            @click="triggerAvatarUpload"
          >
            <img v-if="avatarUrl" :src="avatarUrl" :alt="userDisplayName" class="h-full w-full object-cover" />
            <span v-else>{{ userInitial }}</span>
            <span class="absolute inset-0 flex items-center justify-center bg-background/70 opacity-0 transition-opacity group-hover:opacity-100" :class="{ 'opacity-100': avatarUploading }">
              <Icon :icon="avatarUploading ? 'mdi:loading' : isLoggedIn ? 'mdi:camera-plus-outline' : 'mdi:login'" class="h-5 w-5 text-primary" :class="{ 'animate-spin': avatarUploading }" />
            </span>
          </button>
          <div class="min-w-0">
            <div class="truncate text-sm font-bold text-foreground">{{ userDisplayName }}</div>
            <button
              type="button"
              class="mt-1 text-xs font-semibold text-primary transition-colors hover:text-primary/80 disabled:text-muted-foreground"
              :disabled="avatarUploading"
              @click="triggerAvatarUpload"
            >
              {{ avatarUploading ? t('common.avatar_uploading') : isLoggedIn ? t('common.upload_avatar') : t('common.login_now') }}
            </button>
          </div>
        </div>
        <input ref="avatarInput" type="file" class="hidden" accept="image/png,image/jpeg,image/webp,image/gif" @change="handleAvatarChange" />
      </div>

      <router-link to="/user/assets" class="flex items-center gap-3 px-4 py-3 rounded-lg hover:bg-muted transition-colors" active-class="bg-primary/10 text-primary font-bold">
        <Icon icon="mdi:wallet-outline" class="w-5 h-5" />
        {{ $t('nav.assets') }}
      </router-link>

      <router-link to="/user/kyc" class="flex items-center gap-3 px-4 py-3 rounded-lg hover:bg-muted transition-colors" active-class="bg-primary/10 text-primary font-bold">
        <Icon icon="mdi:card-account-details-outline" class="w-5 h-5" />
        {{ $t('nav.kyc') }}
      </router-link>

      <router-link to="/user/security" class="flex items-center gap-3 px-4 py-3 rounded-lg hover:bg-muted transition-colors" active-class="bg-primary/10 text-primary font-bold">
        <Icon icon="mdi:shield-lock-outline" class="w-5 h-5" />
        {{ $t('nav.security') }}
      </router-link>


      <router-link to="/user/transaction" class="flex items-center gap-3 px-4 py-3 rounded-lg hover:bg-muted transition-colors" active-class="bg-primary/10 text-primary font-bold">
        <Icon icon="mdi:history" class="w-5 h-5" />
        {{ $t('nav.transaction') }}
      </router-link>


      <router-link to="/user/finance-orders" class="flex items-center gap-3 px-4 py-3 rounded-lg hover:bg-muted transition-colors" active-class="bg-primary/10 text-primary font-bold">
        <Icon icon="mdi:robot-outline" class="w-5 h-5" />
        {{ $t('nav.ai_finance') }}
      </router-link>

      <router-link to="/user/loan-orders" class="flex items-center gap-3 px-4 py-3 rounded-lg hover:bg-muted transition-colors" active-class="bg-primary/10 text-primary font-bold">
        <Icon icon="mdi:file-document-multiple-outline" class="w-5 h-5" />
        {{ $t('nav.loan_orders') }}
      </router-link>

      <router-link to="/user/prediction-orders" class="flex items-center gap-3 px-4 py-3 rounded-lg hover:bg-muted transition-colors" active-class="bg-primary/10 text-primary font-bold">
        <Icon icon="mdi:chart-timeline-variant-shimmer" class="w-5 h-5" />
        {{ $t('nav.prediction_orders') }}
      </router-link>

      <router-link to="/user/invite" class="flex items-center gap-3 px-4 py-3 rounded-lg hover:bg-muted transition-colors" active-class="bg-primary/10 text-primary font-bold">
        <Icon icon="mdi:account-multiple-plus-outline" class="w-5 h-5" />
        {{ $t('invite.title') }}
      </router-link>
    </div>

    <!-- Content Area -->
    <div class="flex-1 bg-card border border-border rounded-xl p-6 min-h-[500px]">
      <AuthRequiredState v-if="!isLoggedIn" />
      <router-view v-else></router-view>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useToast } from 'vue-toastification'
import { Icon } from '@iconify/vue'
import { uploadUserAvatar } from '@/api/user'
import AuthRequiredState from '@/components/common/AuthRequiredState.vue'
import { useAuthRequired } from '@/composables/useAuthRequired'

const { t } = useI18n()
const toast = useToast()
const { isLoggedIn, goToLogin, userStore } = useAuthRequired()

const avatarInput = ref<HTMLInputElement | null>(null)
const avatarUploading = ref(false)

const userDisplayName = computed(() => isLoggedIn.value ? (userStore.user?.username || userStore.user?.email || userStore.user?.phone || t('common.user')) : t('common.guest_user'))
const avatarUrl = computed(() => userStore.user?.avatar || userStore.user?.avatarString || '')
const userInitial = computed(() => userDisplayName.value.trim().charAt(0).toUpperCase() || 'U')

function triggerAvatarUpload() {
  if (avatarUploading.value) return
  if (!isLoggedIn.value) {
    goToLogin()
    return
  }
  avatarInput.value?.click()
}

async function handleAvatarChange(event: Event) {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  if (!file || !isLoggedIn.value) return

  avatarUploading.value = true
  try {
    const response = await uploadUserAvatar(file)
    userStore.setUser({ ...(userStore.user || {}), avatar: response.data.avatar_url })
    await userStore.loadProfile().catch((error) => console.error('Failed to refresh user profile', error))
    toast.success(t('common.avatar_upload_success'))
  } catch (error: any) {
    toast.error(error?.response?.data?.message || t('common.avatar_upload_failed'))
  } finally {
    avatarUploading.value = false
    input.value = ''
  }
}
</script>
