<template>
  <div class="min-h-screen flex items-center justify-center bg-background relative overflow-hidden">
    <!-- Background Effects -->
    <div class="absolute inset-0 pointer-events-none">
      <div class="absolute top-[-10%] right-[-10%] w-[40%] h-[40%] bg-neon-green/10 rounded-full blur-[100px]"></div>
      <div class="absolute bottom-[-10%] left-[-10%] w-[40%] h-[40%] bg-primary/10 rounded-full blur-[100px]"></div>
    </div>

    <div class="w-full max-w-md p-8 bg-card/60 backdrop-blur-xl border border-border rounded-xl shadow-2xl relative z-10">
      <div class="flex justify-center mb-6">
          <BrandLogo container-class="flex flex-col items-center" image-class="w-28 h-12 object-contain drop-shadow-neon" />
      </div>
      <div class="text-center mb-8">
        <h1 class="text-3xl font-black tracking-tighter text-glow mb-2">{{ t('auth.register_title') }}</h1>
        <p class="text-muted-foreground">{{ t('auth.register_desc') }}</p>
      </div>

      <form @submit.prevent="handleRegister" class="space-y-5">
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
          <label class="text-sm font-medium text-muted-foreground">{{ t('auth.register_code_label') }}</label>
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
              {{ countdown > 0 ? t('auth.register_countdown', { seconds: countdown }) : (codeLoading ? t('auth.sending') : t('auth.send_code')) }}
            </button>
          </div>
        </div>

        <div class="space-y-2">
          <label class="text-sm font-medium text-muted-foreground">{{ t('auth.register_password') }}</label>
          <div class="relative">
            <span class="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground">
               <span class="i-lucide-lock w-4 h-4"></span>
            </span>
            <input
              v-model="form.password"
              type="password"
              class="w-full bg-background/50 border border-border rounded-lg pl-10 pr-4 py-3 text-foreground focus:border-primary focus:ring-1 focus:ring-primary transition-all outline-none"
              :placeholder="t('auth.password_placeholder')"
              required
            />
          </div>
        </div>

        <div class="space-y-2">
          <label class="text-sm font-medium text-muted-foreground">{{ t('auth.register_country') }}</label>
          <div ref="countrySelectRef" class="relative">
            <button
              type="button"
              :disabled="countryDropdownDisabled"
              class="w-full bg-background/50 border border-border rounded-lg px-4 py-3 text-left text-foreground focus:border-primary focus:ring-1 focus:ring-primary transition-all outline-none disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-between gap-3"
              :aria-expanded="countrySelectOpen"
              aria-haspopup="listbox"
              @click="toggleCountrySelect"
            >
              <span class="min-w-0 truncate">
                {{ countriesLoading ? t('auth.register_loading_countries') : (selectedCountry ? countryLabel(selectedCountry) : t('auth.register_select_country')) }}
              </span>
              <span :class="['i-lucide-chevron-down h-4 w-4 shrink-0 text-muted-foreground transition-transform', countrySelectOpen ? 'rotate-180' : '']"></span>
            </button>

            <div
              v-if="countrySelectOpen"
              class="absolute left-0 right-0 top-full z-30 mt-2 overflow-hidden rounded-xl border border-border bg-card shadow-2xl"
            >
              <div class="border-b border-border p-3">
                <input
                  v-model="countrySearch"
                  type="search"
                  class="w-full bg-background/60 border border-border rounded-lg px-3 py-2 text-sm text-foreground focus:border-primary focus:ring-1 focus:ring-primary transition-all outline-none"
                  :placeholder="t('auth.register_search_country')"
                  @keydown.escape.prevent="closeCountrySelect"
                />
              </div>
              <div class="max-h-64 overflow-y-auto py-1" role="listbox">
                <button
                  v-for="country in filteredCountryOptions"
                  :key="country.code"
                  type="button"
                  role="option"
                  :aria-selected="country.code === form.countryCode"
                  class="w-full px-4 py-3 text-left hover:bg-muted/60 transition-colors flex items-center justify-between gap-3"
                  :class="country.code === form.countryCode ? 'bg-primary/10 text-primary' : 'text-foreground'"
                  @click="selectCountry(country.code)"
                >
                  <span class="min-w-0 truncate font-medium">{{ country.name }}</span>
                  <span class="shrink-0 rounded-md bg-background/70 px-2 py-0.5 text-xs font-bold text-muted-foreground">{{ country.code }}</span>
                </button>
                <div v-if="filteredCountryOptions.length === 0" class="px-4 py-6 text-center text-sm text-muted-foreground">
                  {{ t('auth.register_no_country_matches') }}
                </div>
              </div>
            </div>
          </div>
          <p v-if="!countriesLoading && countryOptions.length === 0" class="text-xs text-destructive">
            {{ t('auth.register_no_countries') }}
          </p>
        </div>

        <div class="space-y-2">
          <label class="text-sm font-medium text-muted-foreground">{{ inviteLabel }}</label>
          <div class="relative">
            <span class="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground">
               <span class="i-lucide-tag w-4 h-4"></span>
            </span>
            <input
              v-model="form.promotion"
              type="text"
              class="w-full bg-background/50 border border-border rounded-lg pl-10 pr-4 py-3 text-foreground focus:border-primary focus:ring-1 focus:ring-primary transition-all outline-none"
              :placeholder="t('auth.register_promotion_placeholder')"
              :required="inviteCodeRequired"
            />
          </div>
        </div>

        <div class="text-xs text-muted-foreground">
            {{ t('auth.register_agreement_prefix') }}
            <a href="#" class="text-primary hover:underline">{{ t('auth.register_terms') }}</a>
            {{ t('auth.register_agreement_middle') }}
            <a href="#" class="text-primary hover:underline">{{ t('auth.register_privacy') }}</a>.
        </div>

        <button
          type="submit"
          :disabled="loading || countriesLoading || configLoading || !form.countryCode"
          class="w-full py-3 bg-primary text-primary-foreground font-bold rounded-lg hover:bg-primary/90 transition-all disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
        >
          <span v-if="loading" class="w-4 h-4 border-2 border-current border-t-transparent rounded-full animate-spin"></span>
          {{ loading ? t('auth.register_creating') : t('auth.register_button') }}
        </button>
      </form>

      <div class="mt-6 text-center text-sm text-muted-foreground">
        {{ t('auth.register_have_account') }}
        <router-link to="/login" class="text-primary hover:underline font-bold">{{ t('auth.sign_in') }}</router-link>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { getRegisterConfig, register, sendVerifyCode } from '@/api/auth'
import { fetchPublicCountries } from '@/api/countries'
import type { PcCountryOption } from '@/api/backendAdapters'
import { useUserStore } from '@/stores/user'
import { useToast } from 'vue-toastification'
import { useI18n } from 'vue-i18n'
import BrandLogo from '@/components/common/BrandLogo.vue'

const router = useRouter()
const userStore = useUserStore()
const toast = useToast()
const { t } = useI18n()
const loading = ref(false)
const codeLoading = ref(false)
const countriesLoading = ref(false)
const configLoading = ref(false)
const countdown = ref(0)
const countryOptions = ref<PcCountryOption[]>([])
const emailCodeRequired = ref(true)
const inviteCodeRequired = ref(false)
const countrySelectOpen = ref(false)
const countrySearch = ref('')
const countrySelectRef = ref<HTMLElement | null>(null)
const form = ref({
  email: '',
  code: '',
  password: '',
  countryCode: '',
  promotion: ''
})

const countryDropdownDisabled = computed(() => countriesLoading.value || countryOptions.value.length === 0)
const selectedCountry = computed(() => countryOptions.value.find((country) => country.code === form.value.countryCode))
const inviteLabel = computed(() => inviteCodeRequired.value ? t('auth.register_promotion_required') : t('auth.register_promotion_optional'))
const filteredCountryOptions = computed(() => {
    const keyword = countrySearch.value.trim().toLowerCase()
    if (!keyword) return countryOptions.value
    return countryOptions.value.filter((country) => {
        return country.name.toLowerCase().includes(keyword) || country.code.toLowerCase().includes(keyword)
    })
})

function countryLabel(country: PcCountryOption) {
    return `${country.name} (${country.code})`
}

function closeCountrySelect() {
    countrySelectOpen.value = false
}

function toggleCountrySelect() {
    if (countryDropdownDisabled.value) return
    countrySelectOpen.value = !countrySelectOpen.value
    if (countrySelectOpen.value) {
        countrySearch.value = ''
    }
}

function selectCountry(code: string) {
    form.value.countryCode = code
    countrySearch.value = ''
    closeCountrySelect()
}

function handleCountryOutsideClick(event: MouseEvent) {
    if (!countrySelectRef.value?.contains(event.target as Node)) {
        closeCountrySelect()
    }
}

const startCountdown = () => {
    countdown.value = 60
    const timer = setInterval(() => {
        countdown.value--
        if (countdown.value <= 0) {
            clearInterval(timer)
        }
    }, 1000)
}

const loadCountries = async () => {
    countriesLoading.value = true
    try {
        const res = await fetchPublicCountries()
        if (res.code === 0) {
            countryOptions.value = res.data
            form.value.countryCode = res.data[0]?.code || ''
        }
    } catch (error: any) {
        console.error(error)
        toast.error(error?.response?.data?.message || t('auth.register_countries_failed'))
    } finally {
        countriesLoading.value = false
    }
}

const loadRegisterConfig = async () => {
    configLoading.value = true
    try {
        const res = await getRegisterConfig()
        if (res.code === 0) {
            emailCodeRequired.value = res.data.emailCodeRequired
            inviteCodeRequired.value = res.data.inviteCodeRequired
        }
    } catch (error: any) {
        console.error(error)
        toast.error(error?.response?.data?.message || t('auth.register_config_failed'))
    } finally {
        configLoading.value = false
    }
}

const sendCode = async () => {
    if (!form.value.email) return
    codeLoading.value = true
    try {
        await sendVerifyCode(form.value.email)
        toast.success(t('auth.register_code_sent'))
        startCountdown()
    } catch (e: any) {
        console.error(e)
        // Global error handling in request interceptor handles 500s, but we can catch specific logic here if needed
    } finally {
        codeLoading.value = false
    }
}

const handleRegister = async () => {
  if (!form.value.countryCode) {
    toast.error(t('auth.register_no_countries'))
    return
  }
  if (emailCodeRequired.value && !form.value.code.trim()) {
    toast.error(t('auth.register_code_required'))
    return
  }
  if (inviteCodeRequired.value && !form.value.promotion.trim()) {
    toast.error(t('auth.register_invite_required'))
    return
  }

  loading.value = true
  try {
    const res: any = await register({
        email: form.value.email,
        code: form.value.code,
        password: form.value.password,
        countryCode: form.value.countryCode,
        inviteCode: form.value.promotion
    })

    if (res.code === 200 || res.code === 0) {
        toast.success(t('auth.register_success'))
        userStore.setAuthSession({
            token: res.data.token,
            refreshToken: res.data.refreshToken,
            user: res.data,
        })
        await userStore.loadProfile().catch((error) => console.error('Failed to load profile:', error))
        await userStore.loadWalletAccounts().catch((error) => console.error('Failed to load wallet accounts:', error))
        router.push('/')
    } else {
        toast.error(res.message || t('auth.register_failed'))
    }
  } catch (error: any) {
    console.error(error)
    // Toast error is handled by request interceptor for 500s, but we might want to show generic error if not handled
    if (!error.response) { // Network errors etc
         toast.error(error.message || t('auth.register_error'))
    }
  } finally {
    loading.value = false
  }
}

onMounted(() => {
    loadCountries()
    loadRegisterConfig()
    window.addEventListener('click', handleCountryOutsideClick)
})

onBeforeUnmount(() => {
    window.removeEventListener('click', handleCountryOutsideClick)
})
</script>
