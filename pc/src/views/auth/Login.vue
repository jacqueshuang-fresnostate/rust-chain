<template>
  <div class="min-h-screen flex items-center justify-center bg-background relative overflow-hidden">
    <!-- Background Effects -->
    <div class="absolute inset-0 pointer-events-none">
      <div class="absolute top-[-10%] left-[-10%] w-[40%] h-[40%] bg-primary/10 rounded-full blur-[100px]"></div>
      <div class="absolute bottom-[-10%] right-[-10%] w-[40%] h-[40%] bg-neon-pink/10 rounded-full blur-[100px]"></div>
    </div>

    <div class="w-full max-w-md p-8 bg-card/60 backdrop-blur-xl border border-border rounded-xl shadow-2xl relative z-10">
      <div class="flex justify-center mb-6">
          <img src="@/assets/logo/logo.png" alt="Hippo Exchange" class="w-28 h-12 object-contain drop-shadow-neon" />
      </div>
      <div class="text-center mb-8">
        <h1 class="text-3xl font-black tracking-tighter text-glow mb-2">WELCOME BACK</h1>
        <p class="text-muted-foreground">Sign in to your trading account</p>
      </div>

      <form @submit.prevent="handleLogin" class="space-y-6">
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
          <div class="flex justify-between">
            <label class="text-sm font-medium text-muted-foreground">Password</label>
            <router-link to="/forgot-password" class="text-xs text-primary hover:underline">Forgot password?</router-link>
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
            <label for="remember" class="text-sm text-muted-foreground select-none cursor-pointer">Remember me</label>
        </div>

        <button
          type="submit"
          :disabled="loading"
          class="w-full py-3 bg-primary text-primary-foreground font-bold rounded-lg hover:bg-primary/90 transition-all disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
        >
          <span v-if="loading" class="w-4 h-4 border-2 border-current border-t-transparent rounded-full animate-spin"></span>
          {{ loading ? 'Signing In...' : 'Sign In' }}
        </button>
      </form>

      <div class="mt-6 text-center text-sm text-muted-foreground">
        Don't have an account?
        <router-link to="/register" class="text-primary hover:underline font-bold">Create Account</router-link>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { login } from '@/api/auth'
import { useUserStore } from '@/stores/user'
import { useToast } from 'vue-toastification'

const router = useRouter()
const userStore = useUserStore()
const toast = useToast()
const loading = ref(false)
const rememberMe = ref(false)
const form = ref({
  email: '',
  password: ''
})

onMounted(() => {
  const savedEmail = localStorage.getItem('remember_email')
  const savedPassword = localStorage.getItem('remember_password')
  if (savedEmail && savedPassword) {
    form.value.email = savedEmail
    form.value.password = savedPassword
    rememberMe.value = true
  }
})

const handleLogin = async () => {
  loading.value = true
  try {
    const res: any = await login({
        email: form.value.email,
        password: form.value.password,
        type: 'password'
    })

    if (res.code === 0 || res.code === 200) {
        // Handle Remember Me
        if (rememberMe.value) {
            localStorage.setItem('remember_email', form.value.email)
            localStorage.setItem('remember_password', form.value.password)
        } else {
            localStorage.removeItem('remember_email')
            localStorage.removeItem('remember_password')
        }

        // 1. Store backend session
        userStore.setAuthSession({
            token: res.data.token,
            refreshToken: res.data.refreshToken,
            user: res.data,
        })
        await userStore.loadProfile().catch((error) => console.error('Failed to load profile:', error))
        await userStore.loadWalletAccounts().catch((error) => console.error('Failed to load wallet accounts:', error))

        // 2. Global Success Message
        toast.success('Login Success')

        // 3. Redirect to Home
        router.push('/')
    } else {
        toast.error(res.message || 'Login failed')
    }
  } catch (error: any) {
    console.error(error)
     // Toast error is handled by request interceptor for 500s, but we might want to show generic error if not handled
    if (!error.response) { // Network errors etc
         toast.error(error.message || 'Login Error')
    }
  } finally {
    loading.value = false
  }
}
</script>
