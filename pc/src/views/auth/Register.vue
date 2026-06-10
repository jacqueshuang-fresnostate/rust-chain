<template>
  <div class="min-h-screen flex items-center justify-center bg-background relative overflow-hidden">
    <!-- Background Effects -->
    <div class="absolute inset-0 pointer-events-none">
      <div class="absolute top-[-10%] right-[-10%] w-[40%] h-[40%] bg-neon-green/10 rounded-full blur-[100px]"></div>
      <div class="absolute bottom-[-10%] left-[-10%] w-[40%] h-[40%] bg-primary/10 rounded-full blur-[100px]"></div>
    </div>

    <div class="w-full max-w-md p-8 bg-card/60 backdrop-blur-xl border border-border rounded-xl shadow-2xl relative z-10">
      <div class="flex justify-center mb-6">
          <img src="@/assets/logo/logo.png" alt="Hippo Exchange" class="w-28 h-12 object-contain drop-shadow-neon" />
      </div>
      <div class="text-center mb-8">
        <h1 class="text-3xl font-black tracking-tighter text-glow mb-2">CREATE ACCOUNT</h1>
        <p class="text-muted-foreground">Join the future of decentralized trading</p>
      </div>

      <form @submit.prevent="handleRegister" class="space-y-5">
        <div class="space-y-2">
          <label class="text-sm font-medium text-muted-foreground">Email</label>
          <div class="relative">
            <span class="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground">
               <span class="i-lucide-mail w-4 h-4"></span>
            </span>
            <input
              v-model="form.email"
              type="email"
              class="w-full bg-background/50 border border-border rounded-lg pl-10 pr-4 py-3 text-foreground focus:border-primary focus:ring-1 focus:ring-primary transition-all outline-none"
              placeholder="name@example.com"
              required
            />
          </div>
        </div>

        <div class="space-y-2">
          <label class="text-sm font-medium text-muted-foreground">Verification Code (Not required by current backend)</label>
          <div class="flex gap-2">
            <input
              v-model="form.code"
              type="text"
              class="flex-1 bg-background/50 border border-border rounded-lg px-4 py-3 text-foreground focus:border-primary focus:ring-1 focus:ring-primary transition-all outline-none text-center font-mono tracking-widest"
              placeholder="XXXXXX"
              maxlength="6"
            />
            <button
              type="button"
              @click="sendCode"
              :disabled="codeLoading || countdown > 0 || !form.email"
              class="px-4 py-2 min-w-[120px] bg-secondary text-secondary-foreground font-bold rounded-lg hover:bg-secondary/80 transition-all disabled:opacity-50 text-sm"
            >
              {{ countdown > 0 ? `${countdown}s` : (codeLoading ? 'Sending...' : 'No Code Required') }}
            </button>
          </div>
        </div>

        <div class="space-y-2">
          <label class="text-sm font-medium text-muted-foreground">Password</label>
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

        <div class="space-y-2">
          <label class="text-sm font-medium text-muted-foreground">Promotion Code (Optional)</label>
          <div class="relative">
            <span class="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground">
               <span class="i-lucide-tag w-4 h-4"></span>
            </span>
            <input
              v-model="form.promotion"
              type="text"
              class="w-full bg-background/50 border border-border rounded-lg pl-10 pr-4 py-3 text-foreground focus:border-primary focus:ring-1 focus:ring-primary transition-all outline-none"
              placeholder="Invitation Code"
            />
          </div>
        </div>

        <div class="text-xs text-muted-foreground">
            By registering, you agree to our <a href="#" class="text-primary hover:underline">Terms of Service</a> and <a href="#" class="text-primary hover:underline">Privacy Policy</a>.
        </div>

        <button
          type="submit"
          :disabled="loading"
          class="w-full py-3 bg-primary text-primary-foreground font-bold rounded-lg hover:bg-primary/90 transition-all disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
        >
          <span v-if="loading" class="w-4 h-4 border-2 border-current border-t-transparent rounded-full animate-spin"></span>
          {{ loading ? 'Creating Account...' : 'Register' }}
        </button>
      </form>

      <div class="mt-6 text-center text-sm text-muted-foreground">
        Already have an account?
        <router-link to="/login" class="text-primary hover:underline font-bold">Sign In</router-link>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { register, sendVerifyCode } from '@/api/auth'
import { useUserStore } from '@/stores/user'
import { useToast } from 'vue-toastification'

const router = useRouter()
const userStore = useUserStore()
const toast = useToast()
const loading = ref(false)
const codeLoading = ref(false)
const countdown = ref(0)
const form = ref({
  email: '',
  code: '',
  password: '',
  promotion: ''
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
        await sendVerifyCode(form.value.email)
        toast.info('Current backend does not require a registration code')
        startCountdown()
    } catch (e: any) {
        console.error(e)
        // Global error handling in request interceptor handles 500s, but we can catch specific logic here if needed
    } finally {
        codeLoading.value = false
    }
}

const handleRegister = async () => {
  loading.value = true
  try {
    const res: any = await register({
        email: form.value.email,
        code: form.value.code,
        password: form.value.password,
        promotion: form.value.promotion
    })

    if (res.code === 200 || res.code === 0) {
        toast.success('Registration successful')
        userStore.setAuthSession({
            token: res.data.token,
            refreshToken: res.data.refreshToken,
            user: res.data,
        })
        await userStore.loadProfile().catch((error) => console.error('Failed to load profile:', error))
        await userStore.loadWalletAccounts().catch((error) => console.error('Failed to load wallet accounts:', error))
        router.push('/')
    } else {
        toast.error(res.message || 'Registration failed')
    }
  } catch (error: any) {
    console.error(error)
    // Toast error is handled by request interceptor for 500s, but we might want to show generic error if not handled
    if (!error.response) { // Network errors etc
         toast.error(error.message || 'Registration Error')
    }
  } finally {
    loading.value = false
  }
}
</script>
