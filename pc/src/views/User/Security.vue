<template>
  <div class="space-y-8">
    <h2 class="text-2xl font-bold mb-6">{{ t('security.title') }}</h2>

    <div v-if="initLoading" class="flex justify-center py-10">
      <Icon icon="mdi:loading" class="animate-spin text-4xl text-primary" />
    </div>

    <template v-else>
      <!-- Password Management -->
      <div class="space-y-4">
        <h3 class="text-lg font-bold flex items-center gap-2">
          <Icon icon="mdi:lock-outline" class="text-primary" />
          {{ t('security.pwd_manage') }}
        </h3>
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div class="p-4 bg-muted/30 rounded border border-border flex justify-between items-center">
            <div>
              <div class="font-bold">{{ t('security.login_pwd') }}</div>
              <div class="text-xs text-muted-foreground">{{ t('security.login_desc') }}</div>
            </div>
            <button @click="openPasswordModal('login')" class="text-primary text-sm font-bold hover:underline">{{ t('security.change') }}</button>
          </div>
          <div class="p-4 bg-muted/30 rounded border border-border flex justify-between items-center">
            <div>
              <div class="font-bold">{{ t('security.trade_pwd') }}</div>
              <div class="text-xs text-muted-foreground">{{ t('security.trade_desc') }}</div>
            </div>
            <div v-if="!isTradePasswordSet">
              <button @click="openPasswordModal('trade-set')" class="text-primary text-sm font-bold hover:underline">{{ t('security.set') }}</button>
            </div>
            <div v-else class="flex gap-3">
              <button @click="openPasswordModal('trade-update')" class="text-primary text-sm font-bold hover:underline">{{ t('security.change') }}</button>
              <button @click="openPasswordModal('trade-reset')" class="text-muted-foreground text-sm font-bold hover:underline">{{ t('security.reset') }}</button>
            </div>
          </div>
        </div>
      </div>

      <!-- Account Binding -->
      <div class="space-y-4">
        <h3 class="text-lg font-bold flex items-center gap-2">
          <Icon icon="mdi:link-variant" class="text-primary" />
          {{ t('security.account_bind') }}
        </h3>
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div class="p-4 bg-muted/30 rounded border border-border flex justify-between items-center">
            <div class="flex items-center gap-3">
              <Icon icon="mdi:email-outline" class="text-2xl text-muted-foreground" />
              <div>
                <div class="font-bold">{{ t('security.email') }}</div>
                <div class="text-xs text-muted-foreground">{{ securitySetting?.email || t('security.not_bound') }}</div>
              </div>
            </div>
            <span v-if="securitySetting?.email" class="text-up text-xs font-bold border border-up/30 px-2 py-1 rounded bg-up/10">{{ t('security.verified') }}</span>
            <button v-else class="text-primary text-sm font-bold hover:underline">{{ t('security.bind') }}</button>
          </div>

          <div class="p-4 bg-muted/30 rounded border border-border flex justify-between items-center">
            <div class="flex items-center gap-3">
              <Icon icon="simple-icons:coinbase" class="text-2xl text-[#0052FF]" />
              <div>
                <div class="font-bold">{{ t('security.wallet') }}</div>
                <div class="text-xs text-muted-foreground">{{ isWalletConnected ? t('security.connected') : t('security.not_connected') }}</div>
              </div>
            </div>
            <button
                v-if="!isWalletConnected"
                @click="handleConnectWallet"
                class="px-3 py-1.5 bg-[#0052FF] text-white text-xs font-bold rounded hover:opacity-90 transition-all flex items-center gap-1"
                :disabled="walletLoading"
            >
              <Icon v-if="walletLoading" icon="mdi:loading" class="animate-spin" />
              <Icon v-else icon="mdi:plus" />
              {{ t('security.connect') }}
            </button>
            <span v-else class="text-up text-xs font-bold border border-up/30 px-2 py-1 rounded bg-up/10">{{ t('security.linked') }}</span>
          </div>
        </div>
      </div>
    </template>
  </div>

  <!-- Dynamic Modal for Password Management -->
  <div v-if="showModal" class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm">
      <div class="bg-card border border-border p-6 rounded-lg w-full max-w-md shadow-neon animate-in fade-in zoom-in duration-200">
          <h3 class="text-xl font-bold mb-4">{{ modalTitle }}</h3>

          <form @submit.prevent="handleSave" class="space-y-4">
              <!-- Old Password (Only for update) -->
              <div v-if="modalType === 'trade-update'">
                  <label class="text-xs font-bold text-muted-foreground mb-1 block">{{ t('security.current_trade') }}</label>
                  <input v-model="form.oldPassword" type="password" required class="w-full bg-muted/50 border border-border rounded p-2 text-sm focus:border-primary focus:outline-none transition-colors" />
              </div>

              <!-- Login Password (Required by backend when setting trade password) -->
              <div v-if="modalType === 'trade-set'">
                  <label class="text-xs font-bold text-muted-foreground mb-1 block">{{ t('security.current_pwd') }}</label>
                  <input v-model="form.loginPassword" type="password" required :placeholder="t('auth.password_placeholder')" class="w-full bg-muted/50 border border-border rounded p-2 text-sm focus:border-primary focus:outline-none transition-colors" />
              </div>

              <!-- New Password (set, update, reset) -->
              <div v-if="['trade-set', 'trade-update', 'trade-reset'].includes(modalType)">
                  <label class="text-xs font-bold text-muted-foreground mb-1 block">{{ t('security.new_trade') }}</label>
                  <input v-model="form.newPassword" type="password" required :placeholder="t('auth.password_placeholder')" minlength="6" maxlength="20" class="w-full bg-muted/50 border border-border rounded p-2 text-sm focus:border-primary focus:outline-none transition-colors" />
              </div>

              <!-- Confirm New Password -->
              <div v-if="['trade-set', 'trade-update', 'trade-reset'].includes(modalType)">
                  <label class="text-xs font-bold text-muted-foreground mb-1 block">{{ t('security.confirm_trade') }}</label>
                  <input v-model="form.confirmPassword" type="password" required :placeholder="t('auth.confirm_placeholder')" minlength="6" maxlength="20" class="w-full bg-muted/50 border border-border rounded p-2 text-sm focus:border-primary focus:outline-none transition-colors" />
              </div>

              <!-- Verification Code (Only for reset) -->
              <div v-if="modalType === 'trade-reset'">
                  <label class="text-xs font-bold text-muted-foreground mb-1 block">{{ t('security.code') }}</label>
                  <div class="flex gap-2">
                      <input v-model="form.code" type="text" required class="flex-1 bg-muted/50 border border-border rounded p-2 text-sm focus:border-primary focus:outline-none transition-colors" placeholder="XXXXXX" />
                      <button type="button" @click="sendCode" :disabled="codeLoading || countdown > 0 || !securitySetting?.email" class="px-3 py-2 bg-secondary text-secondary-foreground text-xs font-bold rounded hover:bg-secondary/80 disabled:opacity-50 min-w-[100px]">
                          {{ countdown > 0 ? `${countdown}s` : (codeLoading ? '...' : t('security.send_code')) }}
                      </button>
                  </div>
              </div>

              <div v-if="modalType === 'login'">
                  <div>
                      <label class="text-xs font-bold text-muted-foreground mb-1 block">{{ t('security.current_pwd') }}</label>
                      <input v-model="form.oldPassword" type="password" required class="w-full bg-muted/50 border border-border rounded p-2 text-sm focus:border-primary focus:outline-none transition-colors" />
                  </div>
                  <div class="mt-4">
                      <label class="text-xs font-bold text-muted-foreground mb-1 block">{{ t('auth.new_password') }}</label>
                      <input v-model="form.newPassword" type="password" required minlength="6" maxlength="20" class="w-full bg-muted/50 border border-border rounded p-2 text-sm focus:border-primary focus:outline-none transition-colors" />
                  </div>
                  <div class="mt-4">
                      <label class="text-xs font-bold text-muted-foreground mb-1 block">{{ t('security.confirm_trade') }}</label>
                      <input v-model="form.confirmPassword" type="password" required minlength="6" maxlength="20" class="w-full bg-muted/50 border border-border rounded p-2 text-sm focus:border-primary focus:outline-none transition-colors" />
                  </div>
              </div>

              <div class="flex justify-end gap-2 mt-6">
                  <button type="button" @click="showModal = false" :disabled="submitLoading" class="px-4 py-2 text-sm font-bold text-muted-foreground hover:text-foreground">{{ t('security.cancel') }}</button>
                  <button type="submit" :disabled="submitLoading" class="px-4 py-2 text-sm font-bold bg-primary text-primary-foreground rounded hover:bg-primary/90 flex items-center gap-2">
                    <Icon v-if="submitLoading" icon="mdi:loading" class="animate-spin" />
                    {{ t('security.save') }}
                  </button>
              </div>
          </form>
      </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { Icon } from '@iconify/vue'
import { useToast } from 'vue-toastification'
import { changeLoginPassword, getSecuritySetting, setTransactionPassword, updateTransactionPassword, resetTransactionPassword, type MemberSecurity } from '@/api/user'
import { sendResetVerifyCode } from '@/api/auth'
import { useI18n } from 'vue-i18n'

const { t } = useI18n()
const toast = useToast()
const walletLoading = ref(false)
const isWalletConnected = ref(false)
const showModal = ref(false)
const modalTitle = ref('')
const submitLoading = ref(false)
const codeLoading = ref(false)
const countdown = ref(0)
const initLoading = ref(true)

const securitySetting = ref<MemberSecurity | null>(null)
const modalType = ref<'trade-set' | 'trade-update' | 'trade-reset' | 'login'>('trade-set')

const form = ref({
    oldPassword: '',
    loginPassword: '',
    newPassword: '',
    confirmPassword: '',
    code: ''
})

// Determining if trading password is set using fundsVerified (assumed)
const isTradePasswordSet = computed(() => {
    return securitySetting.value?.fundsVerified === 1 || securitySetting.value?.transactionStatus === 1
})

const fetchSecuritySetting = async () => {
    try {
        const res: any = await getSecuritySetting()
        if (res.code === 0 || res.code === 200) {
            securitySetting.value = res.data
        }
    } catch (e) {
        console.error('Failed to load security settings', e)
    } finally {
        initLoading.value = false
    }
}

onMounted(() => {
    fetchSecuritySetting()
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
    if (!securitySetting.value?.email) {
        toast.error(t('security.no_email'))
        return
    }
    codeLoading.value = true
    try {
        await sendResetVerifyCode(securitySetting.value.email)
        toast.success(t('security.code_sent') + ' ' + securitySetting.value.email)
        startCountdown()
    } catch (e: any) {
        console.error(e)
    } finally {
        codeLoading.value = false
    }
}

function openPasswordModal(type: 'trade-set' | 'trade-update' | 'trade-reset' | 'login') {
    modalType.value = type
    modalTitle.value = type === 'trade-set' ? t('security.set_trade') :
                       type === 'trade-update' ? t('security.update_trade') :
                       type === 'trade-reset' ? t('security.reset_trade') : t('security.change_login')
    form.value = { oldPassword: '', loginPassword: '', newPassword: '', confirmPassword: '', code: '' }
    showModal.value = true
}

async function handleSave() {
    if (form.value.newPassword !== form.value.confirmPassword) {
        toast.error(t('security.no_match'))
        return
    }

    submitLoading.value = true
    try {
        let res: any

        if (modalType.value === 'login') {
            res = await changeLoginPassword(form.value.oldPassword, form.value.newPassword)
        } else if (modalType.value === 'trade-set') {
            res = await setTransactionPassword(form.value.newPassword, form.value.loginPassword)
        } else if (modalType.value === 'trade-update') {
            res = await updateTransactionPassword(form.value.oldPassword, form.value.newPassword)
        } else if (modalType.value === 'trade-reset') {
            res = await resetTransactionPassword(form.value.newPassword, form.value.code)
        }

        if (res?.code === 0 || res?.code === 200) {
            toast.success(res.message || t('security.success'))
            showModal.value = false
            fetchSecuritySetting() // Refresh settings
        } else {
            toast.error(res?.message || t('security.failed'))
        }
    } catch (e: any) {
        console.error(e)
        if (!e.response) {
            toast.error(e.message || t('security.failed'))
        }
    } finally {
        submitLoading.value = false
    }
}

function handleConnectWallet() {
    walletLoading.value = false
    toast.error('当前后端暂未开放钱包绑定接口')
}
</script>
