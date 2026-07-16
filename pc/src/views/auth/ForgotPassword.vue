<template>
  <div class="min-h-screen flex items-center justify-center bg-background relative overflow-hidden">
    <!-- Background Effects -->
    <div class="absolute inset-0 pointer-events-none">
      <div class="absolute top-[-10%] right-[-10%] w-[40%] h-[40%] bg-primary/10 rounded-full blur-[100px]"></div>
      <div class="absolute bottom-[-10%] left-[-10%] w-[40%] h-[40%] bg-neon-blue/10 rounded-full blur-[100px]"></div>
    </div>

    <div class="w-full max-w-md p-8 bg-card/60 backdrop-blur-xl border border-border rounded-xl shadow-2xl relative z-10">
      <div class="flex justify-center mb-6">
          <BrandLogo container-class="flex flex-col items-center" image-class="w-28 h-12 object-contain drop-shadow-neon" />
      </div>
      <div class="text-center mb-8">
        <h1 class="text-3xl font-black tracking-tighter text-glow mb-2">{{ t('auth.password_reset') }}</h1>
        <p class="text-muted-foreground">{{ t('auth.password_reset_desc') }}</p>
      </div>

      <form @submit.prevent="handleReset" class="space-y-5">
        <div class="space-y-2">
          <label class="text-sm font-medium text-muted-foreground">{{ t('auth.email') }}</label>
          <div class="relative">
            <span class="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground">
               <span class="i-lucide-mail w-4 h-4"></span>
            </span>
            <input
              v-model="form.email"
              type="email"
              class="w-full bg-background/50 border border-border rounded-lg pl-10 pr-4 py-3 text-foreground focus:border-primary focus:ring-1 focus:ring-primary transition-all outline-none"
              :placeholder="t('auth.email_placeholder')"
              required
            />
          </div>
        </div>

        <div class="space-y-2">
          <label class="text-sm font-medium text-muted-foreground">{{ t('auth.verification_code') }}</label>
          <div class="flex gap-2">
            <input
              v-model="form.code"
              type="text"
              class="flex-1 bg-background/50 border border-border rounded-lg px-4 py-3 text-foreground focus:border-primary focus:ring-1 focus:ring-primary transition-all outline-none text-center font-mono tracking-widest"
              placeholder="XXXXXX"
              maxlength="6"
              required
            />
            <button
              type="button"
              @click="sendCode"
              :disabled="codeLoading || countdown > 0 || !form.email"
              class="px-4 py-2 min-w-[120px] bg-secondary text-secondary-foreground font-bold rounded-lg hover:bg-secondary/80 transition-all disabled:opacity-50 text-sm"
            >
              {{ countdown > 0 ? `${countdown}s` : (codeLoading ? t('auth.sending') : t('auth.send_code')) }}
            </button>
          </div>
        </div>

        <div class="space-y-2">
          <label class="text-sm font-medium text-muted-foreground">{{ t('auth.new_password') }}</label>
          <div class="relative">
            <span class="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground">
               <span class="i-lucide-lock w-4 h-4"></span>
            </span>
            <input
              v-model="form.password"
              type="password"
              class="w-full bg-background/50 border border-border rounded-lg pl-10 pr-4 py-3 text-foreground focus:border-primary focus:ring-1 focus:ring-primary transition-all outline-none"
              :placeholder="t('auth.password_placeholder')"
              minlength="6"
              maxlength="20"
              required
            />
          </div>
        </div>

        <div class="space-y-2">
          <label class="text-sm font-medium text-muted-foreground">{{ t('auth.confirm_password') }}</label>
          <div class="relative">
            <span class="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground">
               <span class="i-lucide-shield-check w-4 h-4"></span>
            </span>
            <input
              v-model="form.confirmPassword"
              type="password"
              class="w-full bg-background/50 border border-border rounded-lg pl-10 pr-4 py-3 text-foreground focus:border-primary focus:ring-1 focus:ring-primary transition-all outline-none"
              :placeholder="t('auth.confirm_placeholder')"
              minlength="6"
              maxlength="20"
              required
            />
          </div>
        </div>

        <button
          type="submit"
          :disabled="loading"
          class="w-full py-3 bg-primary text-primary-foreground font-bold rounded-lg hover:bg-primary/90 transition-all disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2 mt-2"
        >
          <span v-if="loading" class="w-4 h-4 border-2 border-current border-t-transparent rounded-full animate-spin"></span>
          {{ loading ? t('auth.resetting') : t('auth.reset_btn') }}
        </button>
      </form>

      <div class="mt-6 text-center text-sm text-muted-foreground">
        {{ t('auth.remember') }}
        <router-link to="/login" class="text-primary hover:underline font-bold">{{ t('auth.sign_in') }}</router-link>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { resetPassword, sendResetVerifyCode } from '@/api/auth'
import { useToast } from 'vue-toastification'
import { useI18n } from 'vue-i18n'
import BrandLogo from '@/components/common/BrandLogo.vue'

const { t } = useI18n()
const router = useRouter()
const toast = useToast()
const loading = ref(false)
const codeLoading = ref(false)
const countdown = ref(0)
const form = ref({
  email: '',
  code: '',
  password: '',
  confirmPassword: ''
})

const startCountdown = () => {
    countdown.value = 60
    const timer = setInterval(() => {
        countdown.value--
        if (countdown.value <= 0) {
            clearInterval(timer)
        }
    }, 1000)
}

const sendCode = async () => {
    if (!form.value.email) return
    codeLoading.value = true
    try {
        await sendResetVerifyCode(form.value.email)
        toast.success(t('auth.code_sent'))
        startCountdown()
    } catch (e: any) {
        console.error(e)
        toast.error(e?.response?.data?.message || e.message || t('auth.reset_code_failed'))
    } finally {
        codeLoading.value = false
    }
}

const handleReset = async () => {
  if (form.value.password !== form.value.confirmPassword) {
    toast.error(t('auth.no_match'))
    return
  }
  if (form.value.password.length < 6 || form.value.password.length > 20) {
    toast.error(t('auth.length_err'))
    return
  }

  loading.value = true
  try {
    const res: any = await resetPassword({
        mode: 1,
        account: form.value.email,
        code: form.value.code,
        password: form.value.password,
    })

    if (res.code === 200 || res.code === 0) {
        toast.success(t('auth.reset_success'))
        router.push('/login')
    } else {
        toast.error(res.message || t('auth.reset_failed'))
    }
  } catch (error: any) {
    console.error(error)
    toast.error(error?.response?.data?.message || error.message || t('auth.reset_error'))
  } finally {
    loading.value = false
  }
}
</script>
