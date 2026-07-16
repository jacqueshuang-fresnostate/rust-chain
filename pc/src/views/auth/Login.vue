<template>
  <div class="min-h-screen flex items-center justify-center bg-background relative overflow-hidden">
    <!-- Background Effects -->
    <div class="absolute inset-0 pointer-events-none">
      <div class="absolute top-[-10%] left-[-10%] w-[40%] h-[40%] bg-primary/10 rounded-full blur-[100px]"></div>
      <div class="absolute bottom-[-10%] right-[-10%] w-[40%] h-[40%] bg-neon-pink/10 rounded-full blur-[100px]"></div>
    </div>

    <div class="w-full max-w-md p-8 bg-card/60 backdrop-blur-xl border border-border rounded-xl shadow-2xl relative z-10">
      <div class="flex justify-center mb-6">
          <BrandLogo container-class="flex flex-col items-center" image-class="w-28 h-12 object-contain drop-shadow-neon" />
      </div>
      <div class="text-center mb-8">
        <h1 class="text-3xl font-black tracking-tighter text-glow mb-2">{{ t('auth.login_title') }}</h1>
        <p class="text-muted-foreground">{{ t('auth.login_desc') }}</p>
      </div>

      <form @submit.prevent="loginStep === 'password' ? handleLogin() : submitTwoFactor()" class="space-y-6">
        <template v-if="loginStep === 'password'">
          <div class="space-y-2">
            <label class="text-sm font-medium text-muted-foreground">{{ loginAccountLabel }}</label>
            <div class="relative">
              <span class="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground">
                 <span :class="[loginAccountIcon, 'w-4 h-4']"></span>
              </span>
              <input
                v-model="form.email"
                :type="usernameLoginEnabled ? 'text' : 'email'"
                class="w-full bg-background/50 border border-border rounded-lg pl-10 pr-4 py-3 text-foreground focus:border-primary focus:ring-1 focus:ring-primary transition-all outline-none"
                :placeholder="loginAccountPlaceholder"
                required
              />
            </div>
          </div>

          <div class="space-y-2">
            <div class="flex justify-between">
              <label class="text-sm font-medium text-muted-foreground">{{ t('auth.password') }}</label>
              <router-link to="/forgot-password" class="text-xs text-primary hover:underline">{{ t('auth.forgot_password') }}</router-link>
            </div>
            <div class="relative">
              <span class="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground">
                 <span class="i-lucide-lock w-4 h-4"></span>
              </span>
              <input
                v-model="form.password"
                type="password"
                class="w-full bg-background/50 border border-border rounded-lg pl-10 pr-4 py-3 text-foreground focus:border-primary focus:ring-1 focus:ring-primary transition-all outline-none"
                placeholder="••••••••"
                required
              />
            </div>
          </div>

          <div class="flex items-center space-x-2">
              <input type="checkbox" id="remember" v-model="rememberMe" class="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary bg-background/50">
              <label for="remember" class="text-sm text-muted-foreground select-none cursor-pointer">{{ t('auth.remember_me') }}</label>
          </div>
        </template>

        <template v-else-if="loginStep === '2fa'">
          <div class="p-4 rounded-lg border border-primary/30 bg-primary/5 text-sm text-muted-foreground">
            {{ t('auth.login_2fa_desc') }}
          </div>
          <div class="space-y-2">
            <label class="text-sm font-medium text-muted-foreground">{{ t('auth.authenticator_code') }}</label>
            <input
              v-model="twoFactorCode"
              inputmode="numeric"
              maxlength="6"
              class="w-full bg-background/50 border border-border rounded-lg px-4 py-3 text-foreground focus:border-primary focus:ring-1 focus:ring-primary transition-all outline-none"
              placeholder="123456"
              required
            />
          </div>
          <div class="space-y-2 border-t border-border pt-4">
            <button type="button" @click="showReset = !showReset" class="text-xs text-primary hover:underline">
              {{ t('auth.lost_authenticator') }}
            </button>
            <div v-if="showReset" class="space-y-3">
              <div class="flex gap-2">
                <input
                  v-model="resetCode"
                  class="flex-1 bg-background/50 border border-border rounded-lg px-4 py-2 text-sm focus:border-primary outline-none"
                  :placeholder="t('auth.email_code_placeholder')"
                />
                <button type="button" @click="sendResetCode" :disabled="resetLoading" class="px-3 py-2 bg-secondary text-secondary-foreground text-xs font-bold rounded disabled:opacity-50">
                  {{ t('auth.send_code') }}
                </button>
              </div>
              <button type="button" @click="handleResetTwoFactor" :disabled="resetLoading || !resetCode" class="w-full py-2 border border-border rounded text-sm font-bold hover:border-primary disabled:opacity-50">
                {{ t('auth.reset_2fa_login') }}
              </button>
            </div>
          </div>
        </template>

        <template v-else>
          <div class="p-4 rounded-lg border border-yellow-500/30 bg-yellow-500/5 text-sm text-muted-foreground space-y-2">
            <div class="font-bold text-foreground">{{ t('auth.setup_required_title') }}</div>
            <p>{{ t('auth.setup_required_desc') }}</p>
          </div>
          <button type="button" @click="loginStep = 'password'" class="w-full py-2 border border-border rounded text-sm font-bold hover:border-primary">
            {{ t('auth.back_to_password') }}
          </button>
        </template>

        <button
          v-if="loginStep !== 'setup-required'"
          type="submit"
          :disabled="loading"
          class="w-full py-3 bg-primary text-primary-foreground font-bold rounded-lg hover:bg-primary/90 transition-all disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
        >
          <span v-if="loading" class="w-4 h-4 border-2 border-current border-t-transparent rounded-full animate-spin"></span>
          {{ primaryButtonLabel }}
        </button>
      </form>

      <div class="mt-6 text-center text-sm text-muted-foreground">
        {{ t('auth.no_account') }}
        <router-link to="/register" class="text-primary hover:underline font-bold">{{ t('auth.create_account') }}</router-link>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { getLoginConfig, login, resetLoginTwoFactor, sendLoginTwoFactorResetCode, submitLoginTwoFactor } from '@/api/auth'
import { useUserStore } from '@/stores/user'
import { useToast } from 'vue-toastification'
import BrandLogo from '@/components/common/BrandLogo.vue'
import { useI18n } from 'vue-i18n'

const router = useRouter()
const userStore = useUserStore()
const toast = useToast()
const { t } = useI18n()
const loading = ref(false)
const resetLoading = ref(false)
const rememberMe = ref(false)
const usernameLoginEnabled = ref(false)
const loginStep = ref<'password' | '2fa' | 'setup-required'>('password')
const challengeId = ref('')
const twoFactorCode = ref('')
const resetCode = ref('')
const showReset = ref(false)
const form = ref({
  email: '',
  password: ''
})

const primaryButtonLabel = computed(() => {
  if (loading.value) return loginStep.value === '2fa' ? t('auth.verifying') : t('auth.signing_in')
  return loginStep.value === '2fa' ? t('auth.verify_and_sign_in') : t('auth.sign_in')
})

const loginAccountLabel = computed(() => usernameLoginEnabled.value ? t('auth.email_or_username') : t('auth.email'))
const loginAccountPlaceholder = computed(() => usernameLoginEnabled.value ? t('auth.email_or_username_placeholder') : t('auth.email_placeholder'))
const loginAccountIcon = computed(() => usernameLoginEnabled.value ? 'i-lucide-user' : 'i-lucide-mail')

onMounted(async () => {
  const savedEmail = localStorage.getItem('remember_email')
  localStorage.removeItem('remember_password')
  if (savedEmail) {
    form.value.email = savedEmail
    rememberMe.value = true
  }
  try {
    const config = await getLoginConfig()
    usernameLoginEnabled.value = Boolean(config.data.usernameLoginEnabled)
  } catch (error) {
    console.error('Failed to load login config', error)
  }
})

const handleLogin = async () => {
  loading.value = true
  try {
    const account = form.value.email.trim()
    const isUsernameLogin = usernameLoginEnabled.value && !account.includes('@')
    const res: any = await login({
        email: isUsernameLogin ? undefined : account,
        username: isUsernameLogin ? account : undefined,
        password: form.value.password,
        type: 'password'
    })

    if (res.code === 0 || res.code === 200) {
        await handleAuthResponse(res)
    } else {
        toast.error(res.message || t('auth.login_failed'))
    }
  } catch (error: any) {
    console.error(error)
     // Toast error is handled by request interceptor for 500s, but we might want to show generic error if not handled
    if (!error.response) { // Network errors etc
         toast.error(error.message || t('auth.login_error'))
    }
  } finally {
    loading.value = false
  }
}

const submitTwoFactor = async () => {
  if (!challengeId.value) return
  loading.value = true
  try {
    const res: any = await submitLoginTwoFactor(challengeId.value, twoFactorCode.value)
    await handleAuthResponse(res)
  } catch (error: any) {
    console.error(error)
    if (!error.response) {
      toast.error(error.message || t('auth.two_factor_failed'))
    }
  } finally {
    loading.value = false
  }
}

async function handleAuthResponse(res: any) {
  if (res.data?.requires2fa) {
    challengeId.value = res.data.challengeId
    twoFactorCode.value = ''
    loginStep.value = '2fa'
    toast.info(t('auth.two_factor_required'))
    return
  }

  if (res.data?.requires2faSetup) {
    challengeId.value = res.data.setupChallengeId
    loginStep.value = 'setup-required'
    return
  }

  if (!res.data?.token) {
    toast.error(res.message || t('auth.login_failed'))
    return
  }

  if (rememberMe.value) {
      localStorage.setItem('remember_email', form.value.email)
  } else {
      localStorage.removeItem('remember_email')
  }
  localStorage.removeItem('remember_password')

  userStore.setAuthSession({
      token: res.data.token,
      refreshToken: res.data.refreshToken,
      user: res.data,
  })
  await userStore.loadProfile().catch((error) => console.error('Failed to load profile:', error))
  await userStore.loadWalletAccounts().catch((error) => console.error('Failed to load wallet accounts:', error))

  toast.success(t('auth.login_success'))
  router.push('/')
}

async function sendResetCode() {
  if (!challengeId.value) return
  resetLoading.value = true
  try {
    await sendLoginTwoFactorResetCode(challengeId.value)
    toast.success(t('auth.reset_code_sent'))
  } catch (error: any) {
    console.error(error)
    if (!error.response) toast.error(error.message || t('auth.reset_code_failed'))
  } finally {
    resetLoading.value = false
  }
}

async function handleResetTwoFactor() {
  if (!challengeId.value || !resetCode.value) return
  resetLoading.value = true
  try {
    await resetLoginTwoFactor(challengeId.value, resetCode.value)
    toast.success(t('auth.two_factor_reset_login'))
    loginStep.value = 'password'
    challengeId.value = ''
    twoFactorCode.value = ''
    resetCode.value = ''
    showReset.value = false
  } catch (error: any) {
    console.error(error)
    if (!error.response) toast.error(error.message || t('auth.two_factor_reset_failed'))
  } finally {
    resetLoading.value = false
  }
}
</script>
